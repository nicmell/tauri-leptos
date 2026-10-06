//! The page's end of the pipe.

/// The URL of the script that starts the pipe's web worker.
pub const WORKER_SCRIPT: &str = "/pipe-worker.js";

#[cfg(feature = "hydrate")]
pub use browser::Pipe;
#[cfg(not(feature = "hydrate"))]
pub use stub::Pipe;

#[cfg(feature = "hydrate")]
mod browser {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use futures::StreamExt;
    use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
    use js_sys::{Array, ArrayBuffer, Function, Object, Reflect, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{MessageChannel, MessageEvent, MessagePort, Worker, WorkerOptions, WorkerType};

    use super::WORKER_SCRIPT;
    use crate::tauri_ipc::{self, Channel, describe};
    use crate::{Frame, PipeEvent};

    /// The page's end of the pipe. In the Tauri app, its far end is the app's
    /// core. Elsewhere, it is a web worker that holds a websocket to `/ws`.
    pub struct Pipe {
        transport: Transport,
        /// Cleared on drop, so a late event reaches nobody.
        alive: Rc<Cell<bool>>,
    }

    enum Transport {
        Worker {
            worker: Worker,
            port: MessagePort,
            _on_message: Closure<dyn Fn(MessageEvent)>,
            _on_error: Closure<dyn Fn()>,
        },
        Tauri {
            channel: Rc<RefCell<Channel>>,
            default_onmessage: Function,
            outgoing: UnboundedSender<Outgoing>,
        },
    }

    enum Outgoing {
        Frame(Frame),
        Reconnect,
    }

    impl Pipe {
        /// Opens the pipe, and hands each event of its far end to `on_event`.
        pub fn open(on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
            let alive = Rc::new(Cell::new(true));
            let on_event: Rc<dyn Fn(PipeEvent)> = {
                let alive = alive.clone();
                Rc::new(move |event| {
                    if alive.get() {
                        on_event(event);
                    }
                })
            };
            let transport = if tauri_ipc::available() {
                open_tauri(on_event)?
            } else {
                open_worker(on_event)?
            };
            Ok(Self { transport, alive })
        }

        /// Sends `frame` to the far end. A frame sent while the pipe is
        /// disconnected is dropped.
        pub fn send(&self, frame: Frame) {
            match &self.transport {
                Transport::Worker { port, .. } => {
                    let _ = match frame {
                        Frame::Text(text) => port.post_message(&text.into()),
                        Frame::Binary(bytes) => {
                            let buffer = Uint8Array::from(bytes.as_slice()).buffer();
                            port.post_message_with_transferable(&buffer, &Array::of1(&buffer))
                        }
                    };
                }
                Transport::Tauri { outgoing, .. } => {
                    let _ = outgoing.unbounded_send(Outgoing::Frame(frame));
                }
            }
        }

        /// Connects again, after `Disconnected`.
        pub fn reconnect(&self) {
            match &self.transport {
                Transport::Worker { port, .. } => {
                    let command = Object::new();
                    let _ = Reflect::set(&command, &"command".into(), &"connect".into());
                    let _ = port.post_message(&command);
                }
                Transport::Tauri { outgoing, .. } => {
                    let _ = outgoing.unbounded_send(Outgoing::Reconnect);
                }
            }
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            self.alive.set(false);
            match &self.transport {
                Transport::Worker { worker, port, .. } => {
                    port.set_onmessage(None);
                    port.close();
                    worker.set_onerror(None);
                    worker.terminate();
                }
                Transport::Tauri {
                    channel,
                    default_onmessage,
                    ..
                } => channel.borrow().set_onmessage(default_onmessage),
            }
        }
    }

    fn open_worker(on_event: Rc<dyn Fn(PipeEvent)>) -> Result<Transport, String> {
        let channel = MessageChannel::new().map_err(|error| describe(&error))?;
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker =
            Worker::new_with_options(WORKER_SCRIPT, &options).map_err(|error| describe(&error))?;
        worker
            .post_message_with_transfer(&JsValue::NULL, &Array::of1(&channel.port2()))
            .map_err(|error| describe(&error))?;
        let port = channel.port1();
        let on_message = {
            let on_event = on_event.clone();
            Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
                if let Some(event) = decode(&message.data()) {
                    on_event(event);
                }
            })
        };
        port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        // A worker that fails to load or to run cannot answer.
        let on_error = Closure::<dyn Fn()>::new(move || on_event(PipeEvent::Disconnected));
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        Ok(Transport::Worker {
            worker,
            port,
            _on_message: on_message,
            _on_error: on_error,
        })
    }

    fn open_tauri(on_event: Rc<dyn Fn(PipeEvent)>) -> Result<Transport, String> {
        let channel = Channel::new().map_err(|error| describe(&error))?;
        let default_onmessage = channel.onmessage();
        let on_message = {
            let on_event = on_event.clone();
            Rc::new(Closure::<dyn Fn(JsValue)>::new(move |data: JsValue| {
                if let Some(event) = decode(&data) {
                    on_event(event);
                }
            }))
        };
        channel.set_onmessage(on_message.as_ref().as_ref().unchecked_ref());
        let channel = Rc::new(RefCell::new(channel));
        let (outgoing, queue) = mpsc::unbounded();
        spawn_local(run(Tauri {
            channel: channel.clone(),
            default_onmessage: default_onmessage.clone(),
            on_message,
            on_event,
            queue,
        }));
        Ok(Transport::Tauri {
            channel,
            default_onmessage,
            outgoing,
        })
    }

    /// What the loop of a Tauri pipe holds.
    struct Tauri {
        channel: Rc<RefCell<Channel>>,
        default_onmessage: Function,
        on_message: Rc<Closure<dyn Fn(JsValue)>>,
        on_event: Rc<dyn Fn(PipeEvent)>,
        queue: UnboundedReceiver<Outgoing>,
    }

    /// Opens the pipe in the Tauri app, posts the queued frames, and closes
    /// the pipe when the queue ends.
    async fn run(mut tauri: Tauri) {
        let current = tauri.channel.borrow().clone();
        let mut id = open(&current, tauri.on_event.as_ref()).await;
        // One call at a time: Tauri does not promise the order of concurrent
        // commands.
        while let Some(item) = tauri.queue.next().await {
            match item {
                Outgoing::Frame(frame) => {
                    if let Some(id) = &id {
                        post(id, frame).await;
                    }
                }
                Outgoing::Reconnect => {
                    if let Some(old) = id.take() {
                        close(&old).await;
                    }
                    // A channel ends with its pipe: Tauri drops the callback
                    // behind it, so a new pipe needs a new channel.
                    let Ok(fresh) = Channel::new() else {
                        (tauri.on_event)(PipeEvent::Disconnected);
                        continue;
                    };
                    fresh.set_onmessage(tauri.on_message.as_ref().as_ref().unchecked_ref());
                    let old = tauri.channel.replace(fresh.clone());
                    old.set_onmessage(&tauri.default_onmessage);
                    id = open(&fresh, tauri.on_event.as_ref()).await;
                }
            }
        }
        if let Some(id) = id {
            close(&id).await;
        }
    }

    async fn open(channel: &Channel, on_event: &dyn Fn(PipeEvent)) -> Option<JsValue> {
        let events = JsValue::from(channel.clone());
        match tauri_ipc::invoke("pipe_open", &[("events", &events)]).await {
            Ok(id) => Some(id),
            Err(error) => {
                web_sys::console::error_1(&error.into());
                on_event(PipeEvent::Disconnected);
                None
            }
        }
    }

    async fn post(id: &JsValue, frame: Frame) {
        let object = Object::new();
        let _ = match frame {
            Frame::Text(text) => Reflect::set(&object, &"text".into(), &text.into()),
            Frame::Binary(bytes) => Reflect::set(
                &object,
                &"binary".into(),
                &Array::from(&Uint8Array::from(bytes.as_slice())),
            ),
        };
        if let Err(error) = tauri_ipc::invoke("pipe_post", &[("id", id), ("frame", &object)]).await
        {
            web_sys::console::error_1(&error.into());
        }
    }

    async fn close(id: &JsValue) {
        if let Err(error) = tauri_ipc::invoke("pipe_close", &[("id", id)]).await {
            web_sys::console::error_1(&error.into());
        }
    }

    /// Reads a value from the far end: a string is a text frame, a buffer a
    /// binary one, and an object with `event` a change of the connection.
    fn decode(data: &JsValue) -> Option<PipeEvent> {
        if let Some(text) = data.as_string() {
            return Some(PipeEvent::Frame(Frame::Text(text)));
        }
        if let Some(buffer) = data.dyn_ref::<ArrayBuffer>() {
            return Some(PipeEvent::Frame(Frame::Binary(
                Uint8Array::new(buffer).to_vec(),
            )));
        }
        // Tauri's IPC fallback answers bytes as an array of numbers.
        if Array::is_array(data) {
            return Some(PipeEvent::Frame(Frame::Binary(
                Uint8Array::new(data).to_vec(),
            )));
        }
        if !data.is_object() {
            return None;
        }
        match Reflect::get(data, &"event".into())
            .ok()?
            .as_string()?
            .as_str()
        {
            "connected" => Some(PipeEvent::Connected),
            "disconnected" => Some(PipeEvent::Disconnected),
            _ => None,
        }
    }
}

#[cfg(not(feature = "hydrate"))]
mod stub {
    use crate::{Frame, PipeEvent};

    /// The page's end of the pipe; it opens in the browser only.
    pub struct Pipe;

    impl Pipe {
        /// Fails outside the browser.
        pub fn open(_on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
            Err("the pipe opens in the browser only".to_owned())
        }

        /// Does nothing outside the browser.
        pub fn send(&self, _frame: Frame) {}

        /// Does nothing outside the browser.
        pub fn reconnect(&self) {}
    }
}
