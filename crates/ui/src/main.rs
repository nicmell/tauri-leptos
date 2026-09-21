//! The watch server, run by `cargo leptos watch` on 127.0.0.1:3001:
//! SSR pages + hydration assets with hot reload. Internal — the dev
//! entry point is the cli (or the tauri shell), which proxies here and
//! holds the api. Open :3000, not this.

// The bin only builds with `required-features = ["ssr"]`.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    use tauri_leptos_core::{logging, server};

    let _log_guard = logging::init(None);

    let listen = SocketAddr::from(([127, 0, 0, 1], 3001));
    let options = tauri_leptos_ui::server::leptos_options(listen);
    let app = tauri_leptos_ui::server::leptos_router(
        options,
        tauri_leptos_core::assets::DirAssets("target/site".into()),
    );

    let srv = server::Server::bind(listen)?;
    tracing::info!(addr = %srv.local_addr()?, "watch server up (ssr only, entry is :3000)");
    srv.serve(app, server::shutdown_signal()).await?;
    Ok(())
}
