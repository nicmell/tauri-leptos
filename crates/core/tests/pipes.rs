use std::time::Duration;

use tauri_leptos_core::pipes::Pipes;
use tauri_leptos_protocol::{ClientMessage, PipeEvent, ServerMessage, decode, encode};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

async fn next(events: &mut mpsc::UnboundedReceiver<String>) -> Option<PipeEvent> {
    tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("an event in time")
        .map(|json| decode(&json).expect("a pipe event"))
}

#[tokio::test]
async fn a_pipe_runs_a_session_until_its_webview_closes() {
    let pipes = Pipes::new(Handle::current());
    let (emitted, mut events) = mpsc::unbounded_channel();
    let id = pipes.open("main", move |json| {
        let _ = emitted.send(json);
    });

    assert_eq!(next(&mut events).await, Some(PipeEvent::Connected));
    let tick = PipeEvent::Received {
        message: ServerMessage::Tick { count: 1 },
    };
    assert_eq!(next(&mut events).await, Some(tick));

    let echo = ClientMessage::Echo { text: "hi".into() };
    pipes.post(id, &encode(&echo)).expect("post");
    loop {
        match next(&mut events).await {
            Some(PipeEvent::Received {
                message: ServerMessage::Echo { text },
            }) => break assert_eq!(text, "hi"),
            Some(PipeEvent::Received { .. }) => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    pipes.close_webview("other");
    assert!(pipes.post(id, &encode(&echo)).is_ok());
    pipes.close_webview("main");
    assert!(pipes.post(id, &encode(&echo)).is_err());
    closed(&mut events).await;
}

#[tokio::test]
async fn a_pipe_closes_alone() {
    let pipes = Pipes::new(Handle::current());
    let (emitted, mut events) = mpsc::unbounded_channel();
    let id = pipes.open("main", move |json| {
        let _ = emitted.send(json);
    });
    let other = pipes.open("main", |_| {});
    assert_eq!(next(&mut events).await, Some(PipeEvent::Connected));

    pipes.close(id);
    let echo = encode(&ClientMessage::Echo { text: "hi".into() });
    assert!(pipes.post(id, &echo).is_err());
    assert!(pipes.post(other, &echo).is_ok());
    closed(&mut events).await;
}

/// Waits for the end of `events`, which can still hold what the session sent
/// before the close.
async fn closed(events: &mut mpsc::UnboundedReceiver<String>) {
    while let Some(event) = next(events).await {
        assert!(
            matches!(event, PipeEvent::Received { .. }),
            "{event:?} after close"
        );
    }
}
