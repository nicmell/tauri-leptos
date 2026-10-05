//! The page's end of the pipe.

#[cfg(feature = "hydrate")]
pub use browser::Pipe;
#[cfg(not(feature = "hydrate"))]
pub use stub::Pipe;

#[cfg(feature = "hydrate")]
mod browser {
    use js_sys::{Function, Promise, Reflect};
    use tauri_leptos_protocol::{ClientMessage, PipeEvent, decode, encode};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{MessageEvent, MessagePort};

    /// The page's end of the pipe: a `MessagePort` whose far end is a web
    /// worker in a browser, or the core of the Tauri app.
    pub struct Pipe {
        port: MessagePort,
        _on_event: Closure<dyn Fn(MessageEvent)>,
    }

    impl Pipe {
        /// Opens the pipe that the server serves this page at `/pipe.js`, and
        /// hands each event of its far end to `on_event`.
        pub async fn open(on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
            let import = Function::new_no_args("return import('/pipe.js')");
            let module: Promise = import
                .call0(&JsValue::NULL)
                .map_err(|error| describe(&error))?
                .unchecked_into();
            let module = JsFuture::from(module)
                .await
                .map_err(|error| describe(&error))?;
            let open: Function = Reflect::get(&module, &"open".into())
                .map_err(|error| describe(&error))?
                .dyn_into()
                .map_err(|error| describe(&error))?;
            let port: MessagePort = open
                .call0(&JsValue::NULL)
                .map_err(|error| describe(&error))?
                .dyn_into()
                .map_err(|error| describe(&error))?;
            let on_event = Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
                if let Some(Ok(event)) = message.data().as_string().map(|json| decode(&json)) {
                    on_event(event);
                }
            });
            port.set_onmessage(Some(on_event.as_ref().unchecked_ref()));
            Ok(Self {
                port,
                _on_event: on_event,
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
        // async like the browser version, which its callers await
        #[allow(clippy::unused_async)]
        pub async fn open(_on_event: impl Fn(PipeEvent) + 'static) -> Result<Self, String> {
            Err("the pipe opens in the browser only".to_owned())
        }

        /// Does nothing outside the browser.
        pub fn send(&self, _message: &ClientMessage) {}
    }
}
