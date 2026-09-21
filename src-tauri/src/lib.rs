use std::sync::OnceLock;

use tauri_leptos_core::logging::{self, LogGuard};

/// The server ships in production desktop builds (feature `ssr`) and in
/// Android builds. `tauri android dev` (cfg(dev)) attaches to the host's
/// `cargo leptos watch` through `adb reverse` instead, like desktop dev.
#[cfg(any(feature = "ssr", all(target_os = "android", not(dev))))]
mod server {
    use std::io;
    use std::path::PathBuf;
    use std::sync::Arc;

    use tauri::Manager;
    use tauri_leptos_ui::server::SiteAssets;

    /// Site assets from the Tauri resource store, one code path for the
    /// desktop bundle and the Android APK: the fs plugin's Rust API opens
    /// plain files on desktop and APK assets (via file descriptor) on
    /// Android, always yielding a real `std::fs::File`.
    struct TauriAssets {
        app: tauri::AppHandle,
        base: PathBuf,
    }

    impl SiteAssets for TauriAssets {
        fn open(&self, rel: &str) -> io::Result<std::fs::File> {
            use tauri_plugin_fs::FsExt;
            let path = std::path::Path::new(rel);
            if path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid asset path",
                ));
            }
            let mut open_options = tauri_plugin_fs::OpenOptions::new();
            open_options.read(true);
            self.app
                .fs()
                .open(self.base.join(path), open_options)
                .map_err(io::Error::other)
        }
    }

    /// Start the in-process single-origin server (SSR + API) on an
    /// ephemeral port and return the bound address — the window is
    /// created on it afterwards, so no fixed port can ever conflict
    /// with something else on the user's machine.
    pub fn start(app: &tauri::App) -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
        use std::net::SocketAddr;

        use tauri_leptos_core::server::Server;

        let resource_site = app.path().resource_dir()?.join("site");
        // Desktop dev with -f ssr runs unbundled from the workspace; on
        // Android the resource path is an asset:// URI where exists()
        // cannot probe, so the resource store is always used there.
        let assets: Arc<dyn SiteAssets> =
            if cfg!(target_os = "android") || resource_site.join("pkg").exists() {
                tracing::info!(base = %resource_site.display(), "serving bundled resources");
                Arc::new(TauriAssets {
                    app: app.handle().clone(),
                    base: resource_site,
                })
            } else {
                tracing::info!("serving workspace target/site (unbundled run)");
                Arc::new(tauri_leptos_ui::server::DirAssets("target/site".into()))
            };

        let server = Server::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let addr = server.local_addr()?;
        let options = tauri_leptos_ui::server::leptos_options(addr);
        let router = tauri_leptos_ui::server::router(options, assets);
        tracing::info!(%addr, "in-process server");
        tauri::async_runtime::spawn(async move {
            // No shutdown signal: the server lives as long as the process.
            if let Err(e) = server.serve(router, std::future::pending()).await {
                tracing::error!("http server exited: {e}");
            }
        });
        Ok(addr)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Keep the file-appender guard (if any) alive for the whole process.
    static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();
    let _ = LOG_GUARD.set(logging::init(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // The window is created here, not in the config: with the
            // in-process server the URL is only known after the bind.
            #[cfg(any(feature = "ssr", all(target_os = "android", not(dev))))]
            let url = format!("http://{}", server::start(app)?);
            #[cfg(not(any(feature = "ssr", all(target_os = "android", not(dev)))))]
            let url = {
                tracing::info!(
                    "no in-process server; window attaches to cargo leptos watch \
                     (android dev: via adb reverse)"
                );
                "http://127.0.0.1:3000".to_owned()
            };
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(url.parse()?),
            )
            .title("tauri-leptos")
            .inner_size(800.0, 600.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
