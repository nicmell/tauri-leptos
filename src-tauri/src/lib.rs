use std::sync::OnceLock;

use tauri::Manager;
use tauri_leptos_core::logging::{self, LogGuard};
use tauri_leptos_core::server::Server;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Keep the file-appender guard (if any) alive for the whole process.
    static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();
    let _ = LOG_GUARD.set(logging::init(None));

    // Desktop dev (`cargo tauri dev`): the webview loads devUrl (trunk on
    // :1420, hot reload) which proxies /api and /ws here — so the port must
    // be the fixed proxy target. Everywhere else the webview loads straight
    // from this server on an ephemeral port. Android always takes the
    // embedded route: devUrl would resolve to the device itself.
    let dev_desktop = tauri::is_dev() && !cfg!(target_os = "android");
    let addr = if dev_desktop {
        ("127.0.0.1", 3000)
    } else {
        ("127.0.0.1", 0)
    };
    let server = Server::bind(addr).expect("bind the local http server");
    let port = server.local_addr().expect("read the bound address").port();

    tauri::Builder::default()
        .setup(move |app| {
            tauri::async_runtime::spawn(async move {
                // No shutdown signal: the server lives as long as the process.
                if let Err(e) = server
                    .serve(
                        tauri_leptos_core::server::api_router(),
                        std::future::pending(),
                    )
                    .await
                {
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
            if !dev_desktop {
                // In dev the default App url resolves to devUrl on its own.
                window_config.url =
                    tauri::WebviewUrl::External(format!("http://127.0.0.1:{port}").parse()?);
            }
            tauri::WebviewWindowBuilder::from_config(app.handle(), &window_config)?.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
