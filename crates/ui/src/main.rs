//! Dev server, run by `cargo leptos watch`: the full single-origin
//! router (SSR + api + ws) with hot reload on 127.0.0.1:3000. Edits to
//! Rust logic restart this process — in-memory api state resets, same
//! as a redeploy.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    use tauri_leptos_core::{logging, server};

    let _log_guard = logging::init(None);

    let listen = SocketAddr::from(([127, 0, 0, 1], 3000));
    let options = tauri_leptos_ui::server::leptos_options(listen);
    let app = tauri_leptos_ui::server::router(
        options,
        std::sync::Arc::new(tauri_leptos_ui::server::DirAssets("target/site".into())),
    );

    let srv = server::Server::bind(listen)?;
    tracing::info!(addr = %srv.local_addr()?, "dev server up (ssr + api)");
    srv.serve(app, server::shutdown_signal()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
