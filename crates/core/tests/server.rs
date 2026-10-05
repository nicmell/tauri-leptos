use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, header};
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
use tower::ServiceExt;

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
    tokio::spawn(async move { axum::serve(listener, router("app", "pkg")).await });
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

async fn script(path: &str, from_webview: bool) -> (String, String) {
    let mut request = Request::get(path);
    if from_webview {
        request = request.header("leptos-ssr-origin", "leptos://localhost");
    }
    let request = request.body(Body::empty()).expect("request");
    let Ok(response) = router("app", "pkg").oneshot(request).await;
    if path == "/pipe.js" {
        assert_eq!(response.headers()[header::VARY], "leptos-ssr-origin");
    }
    let content_type = response.headers()[header::CONTENT_TYPE]
        .to_str()
        .expect("ascii")
        .to_owned();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (
        content_type,
        String::from_utf8(body.to_vec()).expect("utf-8"),
    )
}

#[tokio::test]
async fn the_scripts_are_javascript() {
    let (content_type, worker) = script("/pipe/worker.js", false).await;
    assert_eq!(content_type, "text/javascript; charset=utf-8");
    assert!(worker.contains("import('/pkg/app.js')"), "{worker}");
    assert!(worker.contains("'/pkg/app.wasm'"), "{worker}");

    let (content_type, pipe) = script("/pipe.js", false).await;
    assert_eq!(content_type, "text/javascript; charset=utf-8");
    assert!(pipe.contains("new Worker('/pipe/worker.js'"), "{pipe}");
}

#[tokio::test]
async fn the_tauri_webview_gets_the_channel_pipe() {
    let (content_type, pipe) = script("/pipe.js", true).await;
    assert_eq!(content_type, "text/javascript; charset=utf-8");
    assert!(pipe.contains("invoke('pipe_open'"), "{pipe}");
}
