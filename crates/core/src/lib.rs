//! The pipe between the page and a session at its far end: frames of text or
//! bytes, carried by a web worker and a websocket in a browser, and by Tauri
//! channels in the Tauri app. The app brings the session.

mod frame;
pub mod page;
#[cfg(feature = "ssr")]
pub mod pipes;
#[cfg(feature = "ssr")]
pub mod server;
#[cfg(feature = "ssr")]
mod session;
#[cfg(feature = "hydrate")]
pub mod tauri_ipc;
#[cfg(feature = "hydrate")]
pub mod worker;

pub use frame::{Frame, PipeEvent};
#[cfg(feature = "ssr")]
pub use session::{Session, session};
