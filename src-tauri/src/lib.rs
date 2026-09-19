use std::sync::OnceLock;

use tauri::Manager;
use tauri_leptos_core::logging::{self, LogGuard};
use tauri_leptos_core::server::Server;

/// Fixed SSR port: the webview URL, the leptos config, and the headless
/// default all agree on it. Will move to config.toml.
const PORT: u16 = 3000;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Keep the file-appender guard (if any) alive for the whole process.
    static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();
    let _ = LOG_GUARD.set(logging::init(None));

    // Built outside cargo-leptos, so the leptos options are assembled by
    // hand; site_root is where `cargo leptos build` leaves the assets.
    let leptos_options = leptos::config::LeptosOptions::builder()
        .output_name("tauri-leptos")
        .site_root("target/site")
        .site_addr(std::net::SocketAddr::from(([127, 0, 0, 1], PORT)))
        .build();
    let app_router = tauri_leptos_ui::server::router(leptos_options);

    let server = Server::bind(("127.0.0.1", PORT)).expect("bind the local http server");

    tauri::Builder::default()
        .setup(move |app| {
            tauri::async_runtime::spawn(async move {
                // No shutdown signal: the server lives as long as the process.
                if let Err(e) = server.serve(app_router, std::future::pending()).await {
                    tracing::error!("http server exited: {e}");
                }
            });

            let resolver = app.path();
            tracing::info!(
                app_config_dir = ?resolver.app_config_dir(),
                app_log_dir = ?resolver.app_log_dir(),
                "tauri path resolver"
            );

            let mut window_config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or("missing window config")?;
            window_config.url =
                tauri::WebviewUrl::External(format!("http://127.0.0.1:{PORT}").parse()?);
            tauri::WebviewWindowBuilder::from_config(app.handle(), &window_config)?.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
