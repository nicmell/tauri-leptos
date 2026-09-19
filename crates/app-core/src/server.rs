//! The single HTTP server shared by every run mode: serves the embedded
//! Trunk bundle (from disk in debug builds), a sample JSON endpoint, and a
//! WebSocket echo. The Tauri shell points its webview at this server, so
//! everything is same-origin — no CORS, no IPC.

use std::future::Future;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

use axum::Json;
use axum::extract::Query;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};

/// The Trunk output at the repo root. `allow_missing` keeps `cargo check`
/// working on a fresh clone; the server answers 503 until `trunk build` runs.
#[derive(rust_embed::Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../dist"]
#[allow_missing]
struct Dist;

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

    /// Serve until `shutdown` resolves, then finish in-flight requests.
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> io::Result<()> {
        if Dist::get("index.html").is_none() {
            tracing::warn!("no frontend bundle embedded; run `trunk build` and rebuild");
        }
        self.listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(self.listener)?;
        axum::serve(listener, router())
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

pub fn router() -> axum::Router {
    axum::Router::new()
        .route("/api/hello", get(hello))
        .route("/ws", get(ws_upgrade))
        .fallback(static_handler)
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

fn serve_embedded(path: &str) -> Option<Response> {
    let file = Dist::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Some(
        (
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data.into_owned(),
        )
            .into_response(),
    )
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    serve_embedded(path)
        // SPA fallback: unknown paths get the app shell
        .or_else(|| serve_embedded("index.html"))
        .unwrap_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "frontend not built - run `trunk build`",
            )
                .into_response()
        })
}
