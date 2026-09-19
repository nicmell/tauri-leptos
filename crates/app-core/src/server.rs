//! The API half of the single shared HTTP server: sample JSON endpoint and
//! WebSocket echo, plus the bind/serve/shutdown machinery. Frontend routes
//! (leptos SSR) are merged on top by the ui crate; every run mode stays
//! same-origin — no CORS, no IPC.

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
    /// Bind synchronously so callers (the Tauri shell) can read the actual
    /// port before entering an async runtime.
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
pub fn api_router() -> axum::Router {
    axum::Router::new()
        .route("/api/hello", get(hello))
        .route("/ws", get(ws_upgrade))
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
