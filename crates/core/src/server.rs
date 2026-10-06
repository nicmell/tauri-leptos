//! The pipe's route: the websocket of the web worker.

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tauri_leptos_protocol::{ClientMessage, decode, encode};
use tokio::sync::mpsc;

use crate::session::session;

/// `GET /ws`: a websocket to a new session, for pages of the same origin.
pub fn router() -> Router {
    Router::new().route("/ws", get(upgrade))
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
