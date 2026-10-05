//! The messages between the page, its pipe and the session at the far end,
//! and their JSON encoding.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A message from the page to the session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Asks the session to send `text` back.
    Echo { text: String },
}

/// A message from the session to the page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// The session's tick counter, sent once a second.
    Tick { count: u64 },
    /// The text of a [`ClientMessage::Echo`].
    Echo { text: String },
}

/// What the far end of the pipe reports to the page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipeEvent {
    /// The session is reachable.
    Connected,
    /// The session ended. A worker pipe opens a new one.
    Disconnected,
    /// The session sent `message`.
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
    fn pipe_events() {
        pinned(&PipeEvent::Connected, r#"{"type":"connected"}"#);
        pinned(&PipeEvent::Disconnected, r#"{"type":"disconnected"}"#);
        pinned(
            &PipeEvent::Received {
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
