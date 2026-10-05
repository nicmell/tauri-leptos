use std::time::Duration;

use tauri_leptos_core::Frame;
use tauri_leptos_protocol::{ClientMessage, ServerMessage, decode, encode, session};
use tokio::sync::mpsc;

async fn next(replies: &mut mpsc::Receiver<Frame>) -> Option<Frame> {
    tokio::time::timeout(Duration::from_secs(2), replies.recv())
        .await
        .expect("a frame in time")
}

/// The next frame that is not a tick.
async fn answer(replies: &mut mpsc::Receiver<Frame>) -> Frame {
    loop {
        let frame = next(replies).await.expect("a frame");
        match &frame {
            Frame::Text(json) if matches!(decode(json), Ok(ServerMessage::Tick { .. })) => {}
            _ => return frame,
        }
    }
}

#[tokio::test]
async fn the_session_ticks_echoes_both_kinds_and_ends_with_its_input() {
    let (to_session, incoming) = mpsc::channel(4);
    let (outgoing, mut replies) = mpsc::channel(4);
    let running = tokio::spawn(session()(incoming, outgoing));

    let tick = next(&mut replies).await.expect("a tick");
    assert_eq!(tick, Frame::Text(encode(&ServerMessage::Tick { count: 1 })));

    let echo = ClientMessage::Echo { text: "hi".into() };
    to_session
        .send(Frame::Text(encode(&echo)))
        .await
        .expect("send");
    let reply = ServerMessage::Echo { text: "hi".into() };
    assert_eq!(answer(&mut replies).await, Frame::Text(encode(&reply)));

    let bytes = Frame::Binary(vec![0, 1, 2, 255]);
    to_session.send(bytes.clone()).await.expect("send");
    assert_eq!(answer(&mut replies).await, bytes);

    drop(to_session);
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .expect("the session ends")
        .expect("no panic");
}
