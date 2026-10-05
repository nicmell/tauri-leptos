//! A session that ends before its page. Its own test binary: the panic hook
//! is process-wide.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use tauri_leptos_core::server::router;
use tauri_leptos_core::{Frame, session};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

static PANICS: AtomicUsize = AtomicUsize::new(0);

/// Says goodbye and ends, as a session that closes its pipe does.
async fn goodbye(_incoming: mpsc::Receiver<Frame>, outgoing: mpsc::Sender<Frame>) {
    let _ = outgoing.send(Frame::Text("bye".to_owned())).await;
}

#[tokio::test]
async fn the_socket_closes_after_the_last_frame_and_nothing_panics() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        PANICS.fetch_add(1, Ordering::SeqCst);
        report(info);
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local address");
    tokio::spawn(async move { axum::serve(listener, router(session(goodbye))).await });
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .expect("connect");

    let last = socket.next().await.expect("open").expect("a frame");
    assert_eq!(last, Message::text("bye"));
    let close = socket.next().await.expect("open").expect("a frame");
    assert!(matches!(close, Message::Close(_)), "{close:?}");
    // A panic in the server's task does not reach this one.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(PANICS.load(Ordering::SeqCst), 0);
}
