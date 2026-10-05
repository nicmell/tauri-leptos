//! The demo session: a tick every second, and an echo of each `Echo` and of
//! each binary frame.

use std::time::Duration;

use tauri_leptos_core::{Frame, Session};
use tokio::sync::mpsc;

use crate::{ClientMessage, ServerMessage, decode, encode};

/// The session behind every pipe.
pub fn session() -> Session {
    tauri_leptos_core::session(run)
}

/// Answers the page's frames from `incoming` on `outgoing`, and sends a
/// `Tick` every second. It ends when `incoming` closes or `outgoing` fails.
async fn run(mut incoming: mpsc::Receiver<Frame>, outgoing: mpsc::Sender<Frame>) {
    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    let mut count = 0;
    loop {
        let reply = tokio::select! {
            _ = ticks.tick() => {
                count += 1;
                text(&ServerMessage::Tick { count })
            }
            frame = incoming.recv() => match frame {
                Some(Frame::Text(json)) => match decode(&json) {
                    Ok(ClientMessage::Echo { text: echoed }) => text(&ServerMessage::Echo { text: echoed }),
                    Err(_) => continue,
                },
                Some(binary @ Frame::Binary(_)) => binary,
                None => return,
            },
        };
        if outgoing.send(reply).await.is_err() {
            return;
        }
    }
}

fn text(message: &ServerMessage) -> Frame {
    Frame::Text(encode(message))
}
