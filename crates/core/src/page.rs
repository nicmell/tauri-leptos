//! The page's end of the pipe.

/// The URL of the script that starts the pipe's web worker.
pub const WORKER_SCRIPT: &str = "/pipe-worker.js";

#[cfg(feature = "hydrate")]
pub use browser::Pipe;
#[cfg(not(feature = "hydrate"))]
pub use stub::Pipe;

#[cfg(feature = "hydrate")]
mod browser {
    use std::cell::Cell;
    use std::rc::Rc;

    use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{MessageChannel, MessageEvent, MessagePort, Worker, WorkerOptions, WorkerType};

    use super::WORKER_SCRIPT;
    use crate::socket::Socket;
    use crate::{Frame, PipeEvent};

    /// The page's end of the pipe: a web worker, which relays to a websocket
    /// to `/ws` on the page's main thread. In the Tauri app,
    /// tauri-plugin-leptos-ssr carries that websocket over IPC.
    pub struct Pipe {
        worker: Worker,
        port: MessagePort,
        _socket: Rc<Socket>,
        /// Cleared on drop, so a late event reaches nobody.
        alive: Rc<Cell<bool>>,
        _on_message: Closure<dyn Fn(MessageEvent)>,
        _on_error: Closure<dyn Fn()>,
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
            let page = MessageChannel::new().map_err(|error| describe(&error))?;
            let socket = MessageChannel::new().map_err(|error| describe(&error))?;
            let options = WorkerOptions::new();
            options.set_type(WorkerType::Module);
            let worker = Worker::new_with_options(WORKER_SCRIPT, &options)
                .map_err(|error| describe(&error))?;
            worker
                .post_message_with_transfer(
                    &JsValue::NULL,
                    &Array::of2(&page.port2(), &socket.port2()),
                )
                .map_err(|error| describe(&error))?;
            let socket = Socket::open(socket.port1());
            let port = page.port1();
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
            Ok(Self {
                worker,
                port,
                _socket: socket,
                alive,
                _on_message: on_message,
                _on_error: on_error,
            })
        }

        /// Sends `frame` to the far end. A frame sent while the pipe is
        /// disconnected is dropped.
        pub fn send(&self, frame: Frame) {
            let _ = match frame {
                Frame::Text(text) => self.port.post_message(&text.into()),
                Frame::Binary(bytes) => {
                    let buffer = Uint8Array::from(bytes.as_slice()).buffer();
                    self.port
                        .post_message_with_transferable(&buffer, &Array::of1(&buffer))
                }
            };
        }

        /// Connects again, after `Disconnected`.
        pub fn reconnect(&self) {
            let command = Object::new();
            let _ = Reflect::set(&command, &"command".into(), &"connect".into());
            let _ = self.port.post_message(&command);
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            self.alive.set(false);
            self.port.set_onmessage(None);
            self.port.close();
            self.worker.set_onerror(None);
            self.worker.terminate();
        }
    }

    /// Reads a value from the worker: a string is a text frame, a buffer a
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

    fn describe(error: &JsValue) -> String {
        format!("{error:?}")
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
