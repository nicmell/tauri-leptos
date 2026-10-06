//! What crosses the pipe.

use serde::{Deserialize, Serialize};

/// One message through the pipe, in either direction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frame {
    Text(String),
    Binary(Vec<u8>),
}

/// What the page hears from its pipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipeEvent {
    /// The far end is reachable, and frames can go out.
    Connected,
    /// The far end is gone. The page decides whether to reconnect.
    Disconnected,
    Frame(Frame),
}
