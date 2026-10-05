//! The page's end of the pipe.

/// The URL of the script that starts the pipe's web worker.
pub const WORKER_SCRIPT: &str = "/pipe-worker.js";

#[cfg(feature = "hydrate")]
pub use browser::Pipe;
#[cfg(not(feature = "hydrate"))]
pub use stub::Pipe;

#[cfg(feature = "hydrate")]
mod browser {
    use js_sys::Array;
    use tauri_leptos_protocol::{ClientMessage, PipeEvent, decode, encode};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{MessageChannel, MessageEvent, MessagePort, Worker, WorkerOptions, WorkerType};

    use super::WORKER_SCRIPT;

    /// The page's end of the pipe: a `MessagePort` to a web worker, which
    /// holds a websocket to `/ws`.
    pub struct Pipe {
        worker: Worker,
        port: MessagePort,
        _on_message: Closure<dyn Fn(MessageEvent)>,
    }

    impl Pipe {
        /// Opens the pipe, and hands each event of its far end to `on_event`.
        pub fn open(on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
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
                if let Some(Ok(event)) = message.data().as_string().map(|json| decode(&json)) {
                    on_event(event);
                }
            });
            port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            Ok(Self {
                worker,
                port,
                _on_message: on_message,
            })
        }

        /// Sends `message` to the session.
        pub fn send(&self, message: &ClientMessage) {
            let _ = self.port.post_message(&encode(message).into());
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            self.port.set_onmessage(None);
            self.port.close();
            self.worker.terminate();
        }
    }

    fn describe(error: &JsValue) -> String {
        format!("{error:?}")
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
