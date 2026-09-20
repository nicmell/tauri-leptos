//! Dev frontend-server, run by `cargo leptos watch`: SSR pages with hot
//! reload on 127.0.0.1:3000. The API lives in its own process (the cli)
//! and its address is injected into the page as the `api-base` meta, so
//! UI rebuilds never touch API state.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;
    use std::path::Path;

    use tauri_leptos_core::{logging, server};

    let _log_guard = logging::init(None);

    let api_addr = "127.0.0.1:3001";
    let listen = SocketAddr::from(([127, 0, 0, 1], 3000));
    let options = tauri_leptos_ui::server::leptos_options(Path::new("target/site"), listen);
    let app = tauri_leptos_ui::server::leptos_router(options, Some(format!("http://{api_addr}")));

    let srv = server::Server::bind(listen)?;
    tracing::info!(addr = %srv.local_addr()?, api = %api_addr, "frontend server up");
    srv.serve(app, server::shutdown_signal()).await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
fn main() {}
