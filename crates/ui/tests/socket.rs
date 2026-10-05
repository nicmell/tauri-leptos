use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use tauri_leptos_protocol::{ClientMessage, ServerMessage, decode, encode};
use tauri_leptos_ui::socket::{OriginPolicy, router};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start(policy: OriginPolicy) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move { axum::serve(listener, router(policy)).await });
    addr
}

/// Opens `/ws` with `origin`, and with `host` in place of the real `Host`.
async fn connect(addr: SocketAddr, origin: &str, host: Option<&str>) -> Result<Socket, Error> {
    let mut request = format!("ws://{addr}/ws")
        .into_client_request()
        .expect("request");
    let headers = request.headers_mut();
    headers.insert("origin", HeaderValue::from_str(origin).expect("origin"));
    if let Some(host) = host {
        headers.insert("host", HeaderValue::from_str(host).expect("host"));
    }
    tokio_tungstenite::connect_async(request)
        .await
        .map(|(socket, _)| socket)
}

fn refused(result: Result<Socket, Error>) -> bool {
    matches!(result, Err(Error::Http(response)) if response.status() == StatusCode::FORBIDDEN)
}

async fn receive(socket: &mut Socket) -> ServerMessage {
    loop {
        if let Message::Text(text) = socket.next().await.expect("open").expect("frame") {
            return decode(text.as_str()).expect("a server message");
        }
    }
}

#[tokio::test]
async fn same_origin_accepts_its_own_origin_only() {
    let addr = start(OriginPolicy::SameOrigin).await;
    assert!(connect(addr, &format!("http://{addr}"), None).await.is_ok());
    assert!(refused(connect(addr, "http://evil.example", None).await));
}

/// A page whose DNS name switched to the server's address sends an `Origin`
/// that matches its `Host`.
#[tokio::test]
async fn only_refuses_a_rebinding_page_that_same_origin_lets_in() {
    let (origin, host) = ("http://rebind.example:3002", Some("rebind.example:3002"));
    let same_origin = start(OriginPolicy::SameOrigin).await;
    assert!(connect(same_origin, origin, host).await.is_ok());

    let only = start(OriginPolicy::Only(vec!["leptos://localhost".into()])).await;
    assert!(connect(only, "leptos://localhost", None).await.is_ok());
    assert!(refused(connect(only, origin, host).await));
}

#[tokio::test]
async fn ticks_and_echoes() {
    let addr = start(OriginPolicy::Only(vec!["leptos://localhost".into()])).await;
    let mut socket = connect(addr, "leptos://localhost", None)
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
