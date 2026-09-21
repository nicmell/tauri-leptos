//! Site-asset backends: one interface, one implementation per way the
//! frontend bundle can be reached. Each backend turns into an axum
//! [`Router`] mounted as the server's fallback; requests that are not
//! assets flow into `on_miss` (the SSR shell renderer in full builds).

use std::path::PathBuf;

use axum::Router;

/// A way to serve the site bundle. `on_miss` handles requests that are
/// not assets (typically: render the SSR shell so the client router
/// can take over).
pub trait Assets {
    fn into_router(self, on_miss: Router) -> Router;
}

/// Assets from a plain directory (cli, tests): `tower_http::ServeDir`
/// with its own traversal guard, `ETag` and range support.
pub struct DirAssets(pub PathBuf);

impl Assets for DirAssets {
    fn into_router(self, on_miss: Router) -> Router {
        let serve = tower_http::services::ServeDir::new(self.0)
            .append_index_html_on_directories(false)
            .fallback(on_miss);
        Router::new().fallback_service(serve)
    }
}
