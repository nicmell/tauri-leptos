//! The pipe between the page and a session at its far end: frames of text or
//! bytes over a websocket to `/ws`, with a web worker between the page and
//! the socket. In the Tauri app, tauri-plugin-leptos-ssr carries the socket
//! over IPC. The app brings the session.

mod frame;
pub mod page;
#[cfg(feature = "ssr")]
pub mod server;
#[cfg(feature = "ssr")]
mod session;
#[cfg(feature = "hydrate")]
mod socket;
#[cfg(feature = "hydrate")]
pub mod worker;

pub use frame::{Frame, PipeEvent};
#[cfg(feature = "ssr")]
pub use session::{Session, session};
