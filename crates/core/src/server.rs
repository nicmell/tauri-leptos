//! The pipe's routes: the websocket for the web worker, and the scripts that
//! open the pipe.

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tauri_leptos_protocol::{ClientMessage, decode, encode};
use tokio::sync::mpsc;

use crate::session::session;

const PIPE_WORKER: &str = include_str!("scripts/pipe-worker.js");
const WORKER: &str = include_str!("scripts/worker.js");

/// `GET /ws`, `GET /pipe.js` and `GET /pipe/worker.js`. The worker loads the
/// app's wasm from `/{pkg_dir}/{output_name}.js` and `.wasm`.
pub fn router(output_name: &str, pkg_dir: &str) -> Router {
    let worker = WORKER
        .replace("__GLUE__", &format!("/{pkg_dir}/{output_name}.js"))
        .replace("__WASM__", &format!("/{pkg_dir}/{output_name}.wasm"));
    Router::new()
        .route("/ws", get(upgrade))
        .route("/pipe.js", get(|| async { script(PIPE_WORKER.to_owned()) }))
        .route(
            "/pipe/worker.js",
            get(move || {
                let worker = worker.clone();
                async move { script(worker) }
            }),
        )
}

fn script(body: String) -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn upgrade(socket: WebSocketUpgrade, headers: HeaderMap) -> Response {
    if !same_origin(&headers) {
        log::warn!(
            "websocket refused for origin {:?}",
            headers.get(header::ORIGIN)
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    socket.on_upgrade(serve)
}

/// Whether `Origin` names the `Host` that the request went to.
fn same_origin(headers: &HeaderMap) -> bool {
    let value_of = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let (Some(origin), Some(host)) = (value_of(header::ORIGIN), value_of(header::HOST)) else {
        return false;
    };
    origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        == Some(host)
}

/// Runs a session for `socket`: its text frames in, the session's messages
/// out.
async fn serve(mut socket: WebSocket) {
    let (to_session, incoming) = mpsc::channel(16);
    let (outgoing, mut from_session) = mpsc::channel(16);
    tokio::spawn(session(incoming, outgoing));
    loop {
        tokio::select! {
            frame = socket.recv() => match frame {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(message) = decode::<ClientMessage>(text.as_str())
                        && to_session.send(message).await.is_err()
                    {
                        return;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return,
            },
            reply = from_session.recv() => {
                let Some(reply) = reply else { return };
                if socket.send(Message::Text(encode(&reply).into())).await.is_err() {
                    return;
                }
            }
        }
    }
}
