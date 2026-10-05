//! The pipe's route in a browser: the websocket of the web worker.

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::{Frame, Session};

/// The room in each direction between a socket and its session.
const QUEUE: usize = 16;

/// `GET /ws`: a websocket to a new run of `session`. A page of another origin
/// gets a 403.
pub fn router(session: Session) -> Router {
    Router::new().route(
        "/ws",
        get(move |socket: WebSocketUpgrade, headers: HeaderMap| {
            let session = session.clone();
            async move { upgrade(socket, &headers, session) }
        }),
    )
}

fn upgrade(socket: WebSocketUpgrade, headers: &HeaderMap, session: Session) -> Response {
    if !origin_allowed(headers) {
        log::warn!(
            "websocket refused for origin {:?}",
            headers.get(header::ORIGIN)
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    socket.on_upgrade(move |socket| serve(socket, session))
}

/// Whether `Origin` is missing, unreadable, or names the `Host` that the
/// request went to.
fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return true;
    };
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    host.is_some_and(|host| {
        origin
            .strip_prefix("http://")
            .or_else(|| origin.strip_prefix("https://"))
            == Some(host)
    })
}

/// Runs `session` for `socket`: the socket's frames in, the session's out.
async fn serve(socket: WebSocket, session: Session) {
    let (to_session, incoming) = mpsc::channel(QUEUE);
    let (outgoing, mut from_session) = mpsc::channel(QUEUE);
    let running = tokio::spawn(session(incoming, outgoing));
    let (mut sink, mut stream) = socket.split();
    // Its own task: a reader that waits for room in the session's queue must
    // not stop the session's frames from going out.
    let writer = tokio::spawn(async move {
        while let Some(frame) = from_session.recv().await {
            let message = match frame {
                Frame::Text(text) => Message::Text(text.into()),
                Frame::Binary(bytes) => Message::Binary(bytes.into()),
            };
            if sink.send(message).await.is_err() {
                return;
            }
        }
        let _ = sink.close().await;
    });
    let reader = async move {
        while let Some(Ok(message)) = stream.next().await {
            let frame = match message {
                Message::Text(text) => Frame::Text(text.as_str().to_owned()),
                Message::Binary(bytes) => Frame::Binary(bytes.to_vec()),
                Message::Close(_) => return,
                Message::Ping(_) | Message::Pong(_) => continue,
            };
            if to_session.send(frame).await.is_err() {
                return;
            }
        }
    };
    tokio::select! {
        () = reader => {}
        _ = writer => {}
    }
    // The reader is gone, so the session's input is closed and it ends; its
    // output closing ends the writer.
    let _ = running.await;
}
