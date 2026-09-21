//! The API half of the shared HTTP server: sample JSON endpoint, an
//! in-memory counter, and a WebSocket echo, plus the bind/serve/shutdown
//! machinery. Frontend routes (leptos SSR) are merged on top by the ui
//! crate for the single-origin production server.

use std::future::Future;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

use axum::Json;
use axum::extract::Query;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};

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
/// Dev router (the tauri shell under `cfg(dev)`): the api lives here
/// (stable across frontend rebuilds), pages and assets come from the
/// watch server through the reverse proxy.
pub fn dev_router(config: &crate::config::AppConfig) -> axum::Router {
    use crate::assets::{Assets, ProxyAssets};
    tracing::info!(upstream = %config.dev.upstream, "dev server (api + proxy to the watch)");
    api_router(&config.cors_origins)
        .merge(ProxyAssets(config.dev.upstream.clone()).into_router(axum::Router::new()))
}

/// Api-only router: a remote api server for frontends elsewhere.
pub fn api_only_router(config: &crate::config::AppConfig) -> axum::Router {
    tracing::info!("api-only server");
    api_router(&config.cors_origins).route("/", get(|| async { "tauri-leptos api server" }))
}

/// The stateful API half. `cors_origins` allows remote frontends to
/// call `/api` cross-origin (empty = same-origin only, no layer;
/// `"*"` = any origin). Web sockets are not subject to CORS.
pub fn api_router(cors_origins: &[String]) -> axum::Router {
    let router = axum::Router::new()
        .route("/api/hello", get(hello))
        .route("/api/counter", get(counter))
        .route("/ws", get(ws_upgrade));
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

#[derive(Deserialize)]
struct HelloParams {
    name: Option<String>,
}

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

async fn hello(Query(params): Query<HelloParams>) -> Json<HelloResponse> {
    let name = params.name.unwrap_or_else(|| "world".to_owned());
    Json(HelloResponse {
        message: format!("Hello, {name}! You've been greeted from Rust!"),
    })
}

/// In-memory state marker: proves across dev rebuilds that the API server
/// process was not restarted (the count survives UI changes).
async fn counter() -> Json<serde_json::Value> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let value = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    Json(serde_json::json!({ "count": value }))
}

async fn ws_upgrade(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(echo)
}

async fn echo(mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(_) | Message::Binary(_) => {
                if socket.send(message).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
