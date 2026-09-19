//! Dev-only frontend server, launched by `cargo leptos watch`: serves the
//! SSR routes (rebuilt on UI changes) and reverse-proxies `/api` and `/ws`
//! to the state-holding API server, which is never restarted by UI work.
//! Same origin for the browser/webview — no CORS.

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum_reverse_proxy::ReverseProxy;
use tauri_leptos_core::{logging, server};

const LISTEN: ([u8; 4], u16) = ([127, 0, 0, 1], 3000);
const DEFAULT_API_ADDR: &str = "127.0.0.1:3001";
const API_ADDR_ENV: &str = "TAURI_LEPTOS_API_ADDR";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _log_guard = logging::init(None);

    let api_addr = std::env::var(API_ADDR_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_API_ADDR.to_owned());

    let listen = SocketAddr::from(LISTEN);
    let leptos_options = tauri_leptos_ui::server::leptos_options(Path::new("target/site"), listen);
    let app = tauri_leptos_ui::server::leptos_router(leptos_options)
        .merge(Router::from(ReverseProxy::new(
            "/api",
            format!("http://{api_addr}/api"),
        )))
        .merge(Router::from(ReverseProxy::new(
            "/ws",
            format!("http://{api_addr}/ws"),
        )));

    let srv = server::Server::bind(listen)?;
    tracing::info!(addr = %srv.local_addr()?, %api_addr, "dev server up (SSR + api proxy)");
    srv.serve(app, server::shutdown_signal()).await?;
    Ok(())
}
