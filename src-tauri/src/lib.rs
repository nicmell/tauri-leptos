#[cfg(all(feature = "site", feature = "dev"))]
compile_error!("features `site` and `dev` are mutually exclusive");
#[cfg(not(any(feature = "site", feature = "dev")))]
compile_error!("enable `site` (default) or `dev`");
#[cfg(all(feature = "dev", not(dev)))]
compile_error!("feature `dev` is for `tauri dev` runs only");

use std::sync::OnceLock;

use tauri_leptos_core::logging::{self, LogGuard};

/// The in-process single-origin server, always on an ephemeral port:
/// `site` builds embed the SSR frontend (bundled resources), `dev`
/// builds keep the api local and reverse-proxy pages/assets from the
/// `cargo leptos watch` server (android: through `adb reverse`).
mod server {
    #[cfg(feature = "site")]
    use std::io;
    #[cfg(feature = "site")]
    use std::path::PathBuf;

    #[cfg(feature = "site")]
    use axum::Router;
    #[cfg(feature = "site")]
    use axum::body::Body;
    #[cfg(feature = "site")]
    use axum::extract::Request;
    #[cfg(feature = "site")]
    use axum::response::{IntoResponse, Response};
    #[cfg(feature = "site")]
    use tauri::Manager;
    #[cfg(feature = "site")]
    use tauri_leptos_core::assets::Assets;

    #[cfg(feature = "site")]
    /// Site assets from the Tauri resource store, one code path for the
    /// desktop bundle and the Android APK: the fs plugin's Rust API opens
    /// plain files on desktop and APK assets (via file descriptor) on
    /// Android, always yielding a real `std::fs::File`. The only backend
    /// that can serve straight from an APK — `ServeDir` needs real paths.
    struct TauriAssets {
        app: tauri::AppHandle,
        base: PathBuf,
    }

    #[cfg(feature = "site")]
    impl TauriAssets {
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

    #[cfg(feature = "site")]
    fn serve_file(rel: &str, file: std::fs::File) -> Response {
        let mime = mime_guess::from_path(rel).first_or_octet_stream();
        let stream = tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file));
        (
            [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
            Body::from_stream(stream),
        )
            .into_response()
    }

    #[cfg(feature = "site")]
    impl Assets for TauriAssets {
        fn into_router(self, on_miss: Router) -> Router {
            Router::new().fallback(move |req: Request| {
                let assets = TauriAssets {
                    app: self.app.clone(),
                    base: self.base.clone(),
                };
                let on_miss = on_miss.clone();
                async move {
                    use tower::util::ServiceExt;
                    let rel = req.uri().path().trim_start_matches('/').to_owned();
                    match assets.open(&rel) {
                        Ok(file) => serve_file(&rel, file),
                        Err(_) => on_miss.oneshot(req).await.unwrap_or_else(|e| match e {}),
                    }
                }
            })
        }
    }

    /// The embedded frontend: SSR + assets from the bundled resources.
    #[cfg(feature = "site")]
    fn app_router(
        app: &tauri::App,
        addr: std::net::SocketAddr,
        config: &tauri_leptos_core::config::AppConfig,
    ) -> tauri::Result<axum::Router> {
        let resource_site = app.path().resource_dir()?.join("site");
        tracing::info!(base = %resource_site.display(), "serving bundled resources");
        let assets = TauriAssets {
            app: app.handle().clone(),
            base: resource_site,
        };
        let options = tauri_leptos_ui::server::leptos_options(addr);
        Ok(tauri_leptos_ui::server::router(options, assets, config))
    }

    /// Dev shell: api in-process, pages/assets proxied from the watch.
    #[cfg(feature = "dev")]
    fn app_router(
        _app: &tauri::App,
        _addr: std::net::SocketAddr,
        config: &tauri_leptos_core::config::AppConfig,
    ) -> tauri::Result<axum::Router> {
        use tauri_leptos_core::assets::{Assets, ProxyAssets};

        let upstream = config.dev.upstream.clone();
        tracing::info!(%upstream, "dev shell (api + proxy to the watch)");
        Ok(tauri_leptos_core::server::api_router(&config.cors_origins)
            .merge(ProxyAssets(upstream).into_router(axum::Router::new())))
    }

    /// Start the in-process single-origin server on an ephemeral port
    /// and return the bound address — the window is created on it
    /// afterwards, so no fixed port can ever conflict with something
    /// else on the user's machine.
    pub fn start(app: &tauri::App) -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
        use std::net::SocketAddr;

        use tauri_leptos_core::server::Server;

        // Best effort: the shell falls back to the default config —
        // standalone path resolution mirrors the tauri path API.
        let config = tauri_leptos_core::paths::AppPaths::resolve_standalone(None)
            .ok()
            .and_then(|paths| tauri_leptos_core::config::AppConfig::load(&paths).ok())
            .unwrap_or_default();

        let server = Server::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let addr = server.local_addr()?;
        let router = app_router(app, addr, &config)?;
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
            // The window is created here, not in the config: the
            // in-process URL is only known after the ephemeral bind.
            let url = format!("http://{}", server::start(app)?);
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
