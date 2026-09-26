//! The API half of the shared HTTP server: the bind/serve/shutdown
//! machinery, the CORS and WebSocket-origin policy, and the routes of
//! the [`demo`] module. Frontend routes (leptos SSR) are merged on top
//! by the ui crate for the single-origin production server.

use std::future::Future;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

mod demo;

pub struct Server {
    listener: std::net::TcpListener,
}

impl Server {
    /// Bind synchronously so callers (the Tauri shell) can bind before
    /// entering an async runtime.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = std::net::TcpListener::bind(addr)?;
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Serve `app` until `shutdown` resolves, then finish in-flight requests.
    pub async fn serve(
        self,
        app: axum::Router,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(self.listener)?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}

/// Resolves on ctrl-c or SIGTERM (unix) — the systemd stop path.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install the ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install the SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}

/// The API surface — merged with the SSR routes by the ui crate.
/// The stateful API half. `cors_origins` allows remote frontends to
/// call `/api` cross-origin (empty = same-origin only, no layer;
/// `"*"` = any origin). Web sockets are not subject to CORS, they go
/// through [`origin_allowed`] instead.
///
/// The routes are the demo ones: an app starts from
/// `axum::Router::new()` here and deletes `server/demo.rs`.
pub fn api_router(cors_origins: &[String]) -> axum::Router {
    let router = demo::routes(cors_origins.to_vec());
    if cors_origins.is_empty() {
        return router;
    }
    let origins = if cors_origins.iter().any(|o| o == "*") {
        tower_http::cors::AllowOrigin::any()
    } else {
        tower_http::cors::AllowOrigin::list(cors_origins.iter().filter_map(|o| o.parse().ok()))
    };
    router.layer(
        tower_http::cors::CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
            .allow_headers([axum::http::header::CONTENT_TYPE]),
    )
}

/// Browsers always allow cross-origin `WebSocket`s, so the server must
/// check the `Origin` itself — wrap every ws route in a `route_layer`
/// that calls this, as [`demo`] does. Same-origin (Origin host == Host
/// header): same-origin (Origin host == Host header)
/// or one of `cors_origins` (`"*"` = any) passes; so does a request
/// without an Origin (non-browser clients).
pub fn origin_allowed(headers: &axum::http::HeaderMap, allowed: &[String]) -> bool {
    let Some(origin) = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    else {
        return true;
    };
    if allowed.iter().any(|a| a == "*" || a == origin) {
        return true;
    }
    let origin_host = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
        .unwrap_or(origin);
    headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| host == origin_host)
}
