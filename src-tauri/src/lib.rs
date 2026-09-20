use std::sync::OnceLock;

use tauri_leptos_core::logging::{self, LogGuard};

/// The window URL is fixed in tauri.conf.json (`http://127.0.0.1:3000`):
/// in dev that is `cargo leptos watch`, in production the in-process
/// single-origin server started here.
#[cfg(feature = "ssr")]
fn start_server(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;
    use std::path::PathBuf;

    use tauri::Manager;
    use tauri_leptos_core::server::Server;

    // Bundled resources when present, the cargo-leptos output otherwise
    // (unbundled runs from the workspace).
    let bundled = app.path().resource_dir()?.join("site");
    let site_root = if bundled.join("pkg").exists() {
        bundled
    } else {
        PathBuf::from("target/site")
    };

    let listen = SocketAddr::from(([127, 0, 0, 1], 3000));
    let options = tauri_leptos_ui::server::leptos_options(&site_root, listen);
    let router = tauri_leptos_ui::server::router(options);
    let server = Server::bind(listen)?;
    tracing::info!(site_root = %site_root.display(), "in-process server on {listen}");
    tauri::async_runtime::spawn(async move {
        // No shutdown signal: the server lives as long as the process.
        if let Err(e) = server.serve(router, std::future::pending()).await {
            tracing::error!("http server exited: {e}");
        }
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Keep the file-appender guard (if any) alive for the whole process.
    static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();
    let _ = LOG_GUARD.set(logging::init(None));

    tauri::Builder::default()
        .setup(|app| {
            #[cfg(feature = "ssr")]
            start_server(app)?;
            #[cfg(not(feature = "ssr"))]
            {
                let _ = app;
                tracing::info!("no in-process server; window attaches to cargo leptos watch");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
