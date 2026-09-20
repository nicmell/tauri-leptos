//! Dev frontend-server, run by `cargo leptos watch`: SSR pages with hot
//! reload on 127.0.0.1:3000. The API lives in its own process (the cli)
//! and its address is injected into the page as the `api-base` meta, so
//! UI rebuilds never touch API state.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    use tauri_leptos_core::config::AppConfig;
    use tauri_leptos_core::paths::{AppPaths, app_dir_from_env};
    use tauri_leptos_core::{logging, server};

    let _log_guard = logging::init(None);

    // Best effort: a dev tool falls back to defaults instead of refusing.
    let api_addr = AppPaths::resolve_standalone(app_dir_from_env().as_deref())
        .ok()
        .and_then(|paths| AppConfig::load(&paths).ok())
        .unwrap_or_default()
        .api_addr;
    let listen = SocketAddr::from(([127, 0, 0, 1], 3000));
    let options = tauri_leptos_ui::server::leptos_options(listen);
    let app = tauri_leptos_ui::server::leptos_router(
        options,
        Some(format!("http://{api_addr}")),
        std::sync::Arc::new(tauri_leptos_ui::server::DirAssets("target/site".into())),
    );

    let srv = server::Server::bind(listen)?;
    tracing::info!(addr = %srv.local_addr()?, api = %api_addr, "frontend server up");
    srv.serve(app, server::shutdown_signal()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
