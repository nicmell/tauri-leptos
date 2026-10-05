use std::net::SocketAddr;

/// The app's settings, available as context to server rendering and to
/// server functions.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Where the Tauri app's websocket server listens, and where the page's
    /// worker connects inside the Tauri app.
    pub ws_addr: SocketAddr,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ws_addr: SocketAddr::from(([127, 0, 0, 1], 3002)),
        }
    }
}
