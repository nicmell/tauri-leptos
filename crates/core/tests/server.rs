use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use tauri_leptos_core::server::router;
use tauri_leptos_core::{Frame, session};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Sends every frame back.
async fn echo(mut incoming: mpsc::Receiver<Frame>, outgoing: mpsc::Sender<Frame>) {
    while let Some(frame) = incoming.recv().await {
        if outgoing.send(frame).await.is_err() {
            return;
        }
    }
}

async fn start() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move { axum::serve(listener, router(session(echo))).await });
    addr
}

async fn connect(addr: SocketAddr, origin: Option<&[u8]>) -> Result<Socket, Error> {
    let mut request = format!("ws://{addr}/ws")
        .into_client_request()
        .expect("request");
    if let Some(origin) = origin {
        let origin = HeaderValue::from_bytes(origin).expect("a header value");
        request.headers_mut().insert("origin", origin);
    }
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}

#[tokio::test]
async fn text_and_binary_frames_cross_both_ways() {
    let addr = start().await;
    let mut socket = connect(addr, Some(format!("http://{addr}").as_bytes()))
        .await
        .expect("connect");

    socket.send(Message::text("hi")).await.expect("send");
    let bytes = vec![0_u8, 1, 2, 255];
    socket
        .send(Message::binary(bytes.clone()))
        .await
        .expect("send");

    let reply = socket.next().await.expect("open").expect("a frame");
    assert_eq!(reply, Message::text("hi"));
    let reply = socket.next().await.expect("open").expect("a frame");
    assert_eq!(reply, Message::binary(bytes));
}

#[tokio::test]
async fn a_page_of_another_origin_is_refused() {
    let addr = start().await;
    let refused = connect(addr, Some(b"http://evil.example")).await;
    assert!(
        matches!(refused, Err(Error::Http(response)) if response.status() == StatusCode::FORBIDDEN)
    );
}

#[tokio::test]
async fn a_client_without_a_readable_origin_passes() {
    let addr = start().await;
    assert!(connect(addr, None).await.is_ok());
    assert!(connect(addr, Some(b"\xff")).await.is_ok());
}
