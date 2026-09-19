use std::sync::OnceLock;

use tauri::Manager;
use tauri_leptos_core::logging::{self, LogGuard};
use tauri_leptos_core::server::Server;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Keep the file-appender guard (if any) alive for the whole process.
    static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();
    let _ = LOG_GUARD.set(logging::init(None));

    // Bind before the runtime starts so the window URL can carry the
    // actual ephemeral port.
    let server = Server::bind(("127.0.0.1", 0)).expect("bind the local http server");
    let port = server.local_addr().expect("read the bound address").port();

    tauri::Builder::default()
        .setup(move |app| {
            tauri::async_runtime::spawn(async move {
                // No shutdown signal: the server lives as long as the process.
                if let Err(e) = server.serve(std::future::pending()).await {
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
                tauri::WebviewUrl::External(format!("http://127.0.0.1:{port}").parse()?);
            tauri::WebviewWindowBuilder::from_config(app.handle(), &window_config)?.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
