use std::time::Duration;

use tauri_leptos_core::pipes::Pipes;
use tauri_leptos_core::{Frame, PipeEvent, session};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

/// Sends every frame back.
async fn echo(mut incoming: mpsc::Receiver<Frame>, outgoing: mpsc::Sender<Frame>) {
    while let Some(frame) = incoming.recv().await {
        if outgoing.send(frame).await.is_err() {
            return;
        }
    }
}

/// Ends at once.
async fn quit(_incoming: mpsc::Receiver<Frame>, _outgoing: mpsc::Sender<Frame>) {}

async fn next(events: &mut mpsc::UnboundedReceiver<PipeEvent>) -> Option<PipeEvent> {
    tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("an event in time")
}

fn open(pipes: &Pipes, webview: &str) -> (u32, mpsc::UnboundedReceiver<PipeEvent>) {
    let (emitted, events) = mpsc::unbounded_channel();
    let id = pipes.open(webview, move |event| {
        let _ = emitted.send(event);
    });
    (id, events)
}

#[tokio::test]
async fn a_pipe_carries_both_kinds_until_its_webview_closes() {
    let pipes = Pipes::new(Handle::current(), session(echo));
    let (id, mut events) = open(&pipes, "main");
    assert_eq!(next(&mut events).await, Some(PipeEvent::Connected));

    for frame in [Frame::Text("hi".into()), Frame::Binary(vec![0, 1, 255])] {
        pipes.post(id, frame.clone()).await.expect("post");
        assert_eq!(next(&mut events).await, Some(PipeEvent::Frame(frame)));
    }

    pipes.close_webview("other");
    assert!(pipes.post(id, Frame::Text("hi".into())).await.is_ok());
    pipes.close_webview("main");
    assert!(pipes.post(id, Frame::Text("hi".into())).await.is_err());
    closed(&mut events).await;
}

#[tokio::test]
async fn a_pipe_closes_alone() {
    let pipes = Pipes::new(Handle::current(), session(echo));
    let (id, mut events) = open(&pipes, "main");
    let (other, _) = open(&pipes, "main");
    assert_eq!(next(&mut events).await, Some(PipeEvent::Connected));

    pipes.close(id);
    assert!(pipes.post(id, Frame::Text("hi".into())).await.is_err());
    assert!(pipes.post(other, Frame::Text("hi".into())).await.is_ok());
    closed(&mut events).await;
}

#[tokio::test]
async fn a_session_that_ends_disconnects_its_page() {
    let pipes = Pipes::new(Handle::current(), session(quit));
    let (_, mut events) = open(&pipes, "main");
    assert_eq!(next(&mut events).await, Some(PipeEvent::Connected));
    assert_eq!(next(&mut events).await, Some(PipeEvent::Disconnected));
    assert_eq!(next(&mut events).await, None);
}

/// Waits for the end of `events`, which can still hold what the session sent
/// before the close.
async fn closed(events: &mut mpsc::UnboundedReceiver<PipeEvent>) {
    while let Some(event) = next(events).await {
        assert!(
            matches!(event, PipeEvent::Frame(_)),
            "{event:?} after close"
        );
    }
}
