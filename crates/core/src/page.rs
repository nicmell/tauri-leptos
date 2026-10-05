//! The page's end of the pipe.

/// The URL of the script that starts the pipe's web worker.
pub const WORKER_SCRIPT: &str = "/pipe-worker.js";

#[cfg(feature = "hydrate")]
pub use browser::Pipe;
#[cfg(not(feature = "hydrate"))]
pub use stub::Pipe;

#[cfg(feature = "hydrate")]
mod browser {
    use futures::StreamExt;
    use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
    use js_sys::{Array, Function};
    use tauri_leptos_protocol::{ClientMessage, PipeEvent, decode, encode};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{MessageChannel, MessageEvent, MessagePort, Worker, WorkerOptions, WorkerType};

    use super::WORKER_SCRIPT;
    use crate::tauri_ipc::{self, Channel, describe};

    /// The page's end of the pipe. In the Tauri app, its far end is the app's
    /// core. Elsewhere, it is a web worker that holds a websocket to `/ws`.
    pub struct Pipe(Transport);

    enum Transport {
        Worker {
            worker: Worker,
            port: MessagePort,
            _on_message: Closure<dyn Fn(MessageEvent)>,
        },
        Tauri {
            channel: Channel,
            default_onmessage: Function,
            outgoing: UnboundedSender<ClientMessage>,
            _on_message: Closure<dyn Fn(JsValue)>,
        },
    }

    impl Pipe {
        /// Opens the pipe, and hands each event of its far end to `on_event`.
        pub fn open(on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
            let on_data = move |data: JsValue| {
                if let Some(Ok(event)) = data.as_string().map(|json| decode(&json)) {
                    on_event(event);
                }
            };
            if tauri_ipc::available() {
                Self::open_tauri(on_data)
            } else {
                Self::open_worker(on_data)
            }
        }

        fn open_worker(on_data: impl Fn(JsValue) + 'static) -> Result<Self, String> {
            let channel = MessageChannel::new().map_err(|error| describe(&error))?;
            let options = WorkerOptions::new();
            options.set_type(WorkerType::Module);
            let worker = Worker::new_with_options(WORKER_SCRIPT, &options)
                .map_err(|error| describe(&error))?;
            worker
                .post_message_with_transfer(&JsValue::NULL, &Array::of1(&channel.port2()))
                .map_err(|error| describe(&error))?;
            let port = channel.port1();
            let on_message = Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
                on_data(message.data());
            });
            port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            Ok(Self(Transport::Worker {
                worker,
                port,
                _on_message: on_message,
            }))
        }

        fn open_tauri(on_data: impl Fn(JsValue) + 'static) -> Result<Self, String> {
            let channel = Channel::new().map_err(|error| describe(&error))?;
            let default_onmessage = channel.onmessage();
            let on_message = Closure::<dyn Fn(JsValue)>::new(on_data);
            channel.set_onmessage(on_message.as_ref().unchecked_ref());
            let (outgoing, queue) = mpsc::unbounded();
            spawn_local(post_all(channel.clone(), queue));
            Ok(Self(Transport::Tauri {
                channel,
                default_onmessage,
                outgoing,
                _on_message: on_message,
            }))
        }

        /// Sends `message` to the session.
        pub fn send(&self, message: &ClientMessage) {
            match &self.0 {
                Transport::Worker { port, .. } => {
                    let _ = port.post_message(&encode(message).into());
                }
                Transport::Tauri { outgoing, .. } => {
                    let _ = outgoing.unbounded_send(message.clone());
                }
            }
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            match &self.0 {
                Transport::Worker { worker, port, .. } => {
                    port.set_onmessage(None);
                    port.close();
                    worker.terminate();
                }
                Transport::Tauri {
                    channel,
                    default_onmessage,
                    ..
                } => channel.set_onmessage(default_onmessage),
            }
        }
    }

    /// Opens a pipe in the Tauri app with `events`, posts the messages of
    /// `queue`, and closes the pipe when `queue` ends.
    async fn post_all(events: Channel, mut queue: UnboundedReceiver<ClientMessage>) {
        let events = JsValue::from(events);
        let id = match tauri_ipc::invoke("pipe_open", &[("events", &events)]).await {
            Ok(id) => id,
            Err(error) => {
                web_sys::console::error_1(&error.into());
                return;
            }
        };
        // One call at a time: Tauri does not promise the order of concurrent
        // commands.
        while let Some(message) = queue.next().await {
            let data = JsValue::from(encode(&message));
            if let Err(error) =
                tauri_ipc::invoke("pipe_post", &[("id", &id), ("data", &data)]).await
            {
                web_sys::console::error_1(&error.into());
            }
        }
        if let Err(error) = tauri_ipc::invoke("pipe_close", &[("id", &id)]).await {
            web_sys::console::error_1(&error.into());
        }
    }
}

#[cfg(not(feature = "hydrate"))]
mod stub {
    use tauri_leptos_protocol::{ClientMessage, PipeEvent};

    /// The page's end of the pipe; it opens in the browser only.
    pub struct Pipe;

    impl Pipe {
        /// Fails outside the browser.
        pub fn open(_on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
            Err("the pipe opens in the browser only".to_owned())
        }

        /// Does nothing outside the browser.
        pub fn send(&self, _message: &ClientMessage) {}
    }
}
