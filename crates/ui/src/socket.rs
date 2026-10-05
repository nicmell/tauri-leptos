//! The websocket of the page's worker: the server half (feature `ssr`) and
//! the page half (feature `hydrate`).

#[cfg(feature = "hydrate")]
pub use client::{SocketWorker, url};
#[cfg(feature = "ssr")]
pub use server::{OriginPolicy, router, serve};

#[cfg(feature = "ssr")]
mod server {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::Router;
    use axum::extract::{State, WebSocketUpgrade};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::{IntoResponse, Response};
    use axum::routing::get;

    /// The pages that may open the websocket, by their `Origin` header.
    #[derive(Clone, Debug)]
    pub enum OriginPolicy {
        /// Pages of the server's own origin: `Origin` names the `Host` the
        /// request went to.
        SameOrigin,
        /// Pages of these origins only.
        Only(Vec<String>),
    }

    impl OriginPolicy {
        fn allows(&self, origin: &str, host: Option<&str>) -> bool {
            match self {
                Self::SameOrigin => host.is_some_and(|host| {
                    origin
                        .strip_prefix("http://")
                        .or_else(|| origin.strip_prefix("https://"))
                        == Some(host)
                }),
                Self::Only(origins) => origins.iter().any(|allowed| allowed == origin),
            }
        }
    }

    /// `GET /ws`: the websocket, for the pages that `policy` allows.
    pub fn router(policy: OriginPolicy) -> Router {
        Router::new()
            .route("/ws", get(upgrade))
            .with_state(Arc::new(policy))
    }

    /// Serves [`router`] on `addr`.
    pub async fn serve(addr: SocketAddr, policy: OriginPolicy) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        log::info!("websocket on ws://{}/ws", listener.local_addr()?);
        axum::serve(listener, router(policy)).await
    }

    async fn upgrade(
        ws: WebSocketUpgrade,
        State(policy): State<Arc<OriginPolicy>>,
        headers: HeaderMap,
    ) -> Response {
        let value_of = |name| headers.get(name).and_then(|value| value.to_str().ok());
        let origin = value_of(header::ORIGIN).unwrap_or_default();
        if !policy.allows(origin, value_of(header::HOST)) {
            log::warn!("websocket refused for origin {origin:?}");
            return StatusCode::FORBIDDEN.into_response();
        }
        log::info!("websocket for origin {origin:?}");
        ws.on_upgrade(crate::demo::session)
    }
}

#[cfg(feature = "hydrate")]
mod client {
    use tauri_leptos_protocol::{ClientMessage, WorkerCommand, WorkerEvent, decode, encode};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};

    /// The websocket URL for this page: `ws_addr` inside the Tauri app, the
    /// page's own origin in a browser.
    pub fn url(ws_addr: &str) -> String {
        let window = leptos::prelude::window();
        let in_tauri =
            js_sys::Reflect::has(&window, &"__TAURI_INTERNALS__".into()).unwrap_or(false);
        if in_tauri {
            return format!("ws://{ws_addr}/ws");
        }
        let location = window.location();
        let scheme = if location
            .protocol()
            .is_ok_and(|protocol| protocol == "https:")
        {
            "wss"
        } else {
            "ws"
        };
        format!("{scheme}://{}/ws", location.host().unwrap_or_default())
    }

    /// The page's web worker, which holds the websocket.
    pub struct SocketWorker {
        worker: Worker,
        _on_event: Closure<dyn Fn(MessageEvent)>,
    }

    impl SocketWorker {
        /// Starts the worker, connects it to `url`, and hands each event it
        /// reports to `on_event`.
        pub fn spawn(
            url: String,
            on_event: impl Fn(WorkerEvent) + 'static,
        ) -> Result<Self, String> {
            let options = WorkerOptions::new();
            options.set_type(WorkerType::Module);
            let worker = Worker::new_with_options("/worker.js", &options)
                .map_err(|error| format!("{error:?}"))?;
            let on_event = {
                let worker = worker.clone();
                Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
                    let Some(Ok(event)) = message.data().as_string().map(|json| decode(&json))
                    else {
                        return;
                    };
                    if event == WorkerEvent::Ready {
                        let connect = WorkerCommand::Connect { url: url.clone() };
                        let _ = worker.post_message(&encode(&connect).into());
                    }
                    on_event(event);
                })
            };
            worker.set_onmessage(Some(on_event.as_ref().unchecked_ref()));
            Ok(Self {
                worker,
                _on_event: on_event,
            })
        }

        /// Sends `message` to the server while the websocket is open.
        pub fn send(&self, message: ClientMessage) {
            let command = WorkerCommand::Send { message };
            let _ = self.worker.post_message(&encode(&command).into());
        }
    }

    impl Drop for SocketWorker {
        fn drop(&mut self) {
            self.worker.set_onmessage(None);
            self.worker.terminate();
        }
    }
}

/// The page's web worker; it runs in the browser only.
#[cfg(not(feature = "hydrate"))]
pub struct SocketWorker;

#[cfg(not(feature = "hydrate"))]
impl SocketWorker {
    /// Fails outside the browser.
    pub fn spawn(
        _url: String,
        _on_event: impl Fn(tauri_leptos_protocol::WorkerEvent) + 'static,
    ) -> Result<Self, String> {
        Err("the worker runs in the browser".to_owned())
    }

    /// Does nothing outside the browser.
    pub fn send(&self, _message: tauri_leptos_protocol::ClientMessage) {}
}

/// The websocket URL for this page; outside the browser, `ws_addr`.
#[cfg(not(feature = "hydrate"))]
pub fn url(ws_addr: &str) -> String {
    format!("ws://{ws_addr}/ws")
}
