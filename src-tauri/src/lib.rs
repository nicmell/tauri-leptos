use std::sync::OnceLock;

use tauri::Manager;
use tauri_leptos_core::config::AppConfig;
use tauri_leptos_core::logging::{self, LogGuard};
use tauri_leptos_core::paths::AppPaths;

/// The in-process SSR server ships in production builds and on mobile;
/// desktop dev attaches to `cargo leptos watch` instead.
#[cfg(any(feature = "ssr", target_os = "android"))]
fn start_server(config: &AppConfig) {
    use std::path::PathBuf;
    use tauri_leptos_core::server::Server;

    let site_root = config
        .site_root
        .clone()
        .unwrap_or_else(|| PathBuf::from("target/site"));
    let leptos_options = tauri_leptos_ui::server::leptos_options(&site_root, config.listen);
    let app_router = tauri_leptos_ui::server::router(leptos_options);
    let server = Server::bind(config.listen).expect("bind the local http server");
    tauri::async_runtime::spawn(async move {
        // No shutdown signal: the server lives as long as the process.
        if let Err(e) = server.serve(app_router, std::future::pending()).await {
            tracing::error!("http server exited: {e}");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Keep the file-appender guard (if any) alive for the whole process.
    static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();
    let _ = LOG_GUARD.set(logging::init(None));

    // Same config as the headless server; the shell degrades to defaults
    // instead of refusing to open a window.
    let config = AppPaths::resolve_standalone(None)
        .map_err(|e| eprintln!("[config] paths unavailable: {e}"))
        .and_then(|paths| {
            AppConfig::load(&paths).map_err(|e| eprintln!("[config] ignoring config: {e}"))
        })
        .unwrap_or_default();
    let port = config.listen.port();

    tauri::Builder::default()
        .setup(move |app| {
            #[cfg(any(feature = "ssr", target_os = "android"))]
            start_server(&config);
            #[cfg(not(any(feature = "ssr", target_os = "android")))]
            tracing::info!(
                port,
                "no in-process server in this build; attaching to cargo leptos watch"
            );

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
                tauri::WebviewUrl::External(format!("http://127.0.0.1:{port}").parse()?);
            tauri::WebviewWindowBuilder::from_config(app.handle(), &window_config)?.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
