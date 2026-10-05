//! The messages between the page, its web worker and the websocket server,
//! and their JSON encoding.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A message from the page to the server, through the worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Asks the server to send `text` back.
    Echo { text: String },
}

/// A message from the server to the page, through the worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// The connection's tick counter, sent once a second.
    Tick { count: u64 },
    /// The text of a [`ClientMessage::Echo`].
    Echo { text: String },
}

/// A command from the page to its worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerCommand {
    /// Opens the websocket to `url`, and opens it again after each close.
    Connect { url: String },
    /// Sends `message` while the websocket is open.
    Send { message: ClientMessage },
}

/// An event from the worker to its page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerEvent {
    /// The worker runs and takes commands.
    Ready,
    /// The websocket opened.
    Connected,
    /// The websocket closed, and the worker opens it again.
    Disconnected,
    /// The server sent `message`.
    Received { message: ServerMessage },
}

/// `value` as JSON.
pub fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("protocol types serialize to JSON")
}

/// The value that `json` encodes.
pub fn decode<T: DeserializeOwned>(json: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::*;

    fn pinned<T>(value: &T, json: &str)
    where
        T: Serialize + DeserializeOwned + PartialEq + Debug,
    {
        assert_eq!(encode(value), json);
        assert_eq!(&decode::<T>(json).expect("decode"), value);
    }

    #[test]
    fn client_messages() {
        pinned(
            &ClientMessage::Echo { text: "hi".into() },
            r#"{"type":"echo","text":"hi"}"#,
        );
    }

    #[test]
    fn server_messages() {
        pinned(
            &ServerMessage::Tick { count: 7 },
            r#"{"type":"tick","count":7}"#,
        );
        pinned(
            &ServerMessage::Echo { text: "hi".into() },
            r#"{"type":"echo","text":"hi"}"#,
        );
    }

    #[test]
    fn worker_commands() {
        pinned(
            &WorkerCommand::Connect {
                url: "ws://127.0.0.1:3002/ws".into(),
            },
            r#"{"type":"connect","url":"ws://127.0.0.1:3002/ws"}"#,
        );
        pinned(
            &WorkerCommand::Send {
                message: ClientMessage::Echo { text: "hi".into() },
            },
            r#"{"type":"send","message":{"type":"echo","text":"hi"}}"#,
        );
    }

    #[test]
    fn worker_events() {
        pinned(&WorkerEvent::Ready, r#"{"type":"ready"}"#);
        pinned(&WorkerEvent::Connected, r#"{"type":"connected"}"#);
        pinned(&WorkerEvent::Disconnected, r#"{"type":"disconnected"}"#);
        pinned(
            &WorkerEvent::Received {
                message: ServerMessage::Tick { count: 1 },
            },
            r#"{"type":"received","message":{"type":"tick","count":1}}"#,
        );
    }

    #[test]
    fn unknown_types_are_errors() {
        assert!(decode::<ServerMessage>(r#"{"type":"nope"}"#).is_err());
    }
}
