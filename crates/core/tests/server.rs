use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tauri_leptos_core::server::router;
use tauri_leptos_core::session::session;
use tauri_leptos_protocol::{ClientMessage, ServerMessage, decode, encode};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn the_session_ticks_echoes_and_ends_with_its_input() {
    let (to_session, incoming) = mpsc::channel(4);
    let (outgoing, mut replies) = mpsc::channel(4);
    let running = tokio::spawn(session(incoming, outgoing));

    assert_eq!(replies.recv().await, Some(ServerMessage::Tick { count: 1 }));
    let echo = ClientMessage::Echo { text: "hi".into() };
    to_session.send(echo).await.expect("send");
    assert_eq!(
        replies.recv().await,
        Some(ServerMessage::Echo { text: "hi".into() })
    );

    drop(to_session);
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .expect("the session ends")
        .expect("no panic");
}

async fn start() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move { axum::serve(listener, router()).await });
    addr
}

async fn connect(addr: SocketAddr, origin: &str) -> Result<Socket, Error> {
    let mut request = format!("ws://{addr}/ws")
        .into_client_request()
        .expect("request");
    let origin = HeaderValue::from_str(origin).expect("origin");
    request.headers_mut().insert("origin", origin);
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}

async fn receive(socket: &mut Socket) -> ServerMessage {
    loop {
        if let Message::Text(text) = socket.next().await.expect("open").expect("frame") {
            return decode(text.as_str()).expect("a server message");
        }
    }
}

#[tokio::test]
async fn the_socket_takes_its_own_origin_only() {
    let addr = start().await;
    let refused = connect(addr, "http://evil.example").await;
    assert!(
        matches!(refused, Err(Error::Http(response)) if response.status() == StatusCode::FORBIDDEN)
    );

    let mut socket = connect(addr, &format!("http://{addr}"))
        .await
        .expect("connect");
    assert_eq!(receive(&mut socket).await, ServerMessage::Tick { count: 1 });
    let echo = ClientMessage::Echo { text: "hi".into() };
    socket
        .send(Message::text(encode(&echo)))
        .await
        .expect("send");
    loop {
        match receive(&mut socket).await {
            ServerMessage::Echo { text } => break assert_eq!(text, "hi"),
            ServerMessage::Tick { .. } => {}
        }
    }
}
