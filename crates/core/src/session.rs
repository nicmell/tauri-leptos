//! The demo session: a tick every second, and an echo of each `Echo`.

use std::time::Duration;

use tauri_leptos_protocol::{ClientMessage, ServerMessage};
use tokio::sync::mpsc;

/// Answers the page's messages from `incoming` on `outgoing`, and sends a
/// `Tick` every second. It ends when `incoming` closes or `outgoing` fails.
pub async fn session(
    mut incoming: mpsc::Receiver<ClientMessage>,
    outgoing: mpsc::Sender<ServerMessage>,
) {
    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    let mut count = 0;
    loop {
        let reply = tokio::select! {
            _ = ticks.tick() => {
                count += 1;
                ServerMessage::Tick { count }
            }
            message = incoming.recv() => match message {
                Some(ClientMessage::Echo { text }) => ServerMessage::Echo { text },
                None => return,
            },
        };
        if outgoing.send(reply).await.is_err() {
            return;
        }
    }
}
