// Dev runs (cfg(dev), emitted by tauri-build) proxy to the watch and
// need no leptos: `-- --no-default-features` is the fast path. Release
// builds must embed the frontend.
#[cfg(all(not(dev), not(feature = "site")))]
compile_error!("release builds need the `site` feature (default)");

use std::sync::OnceLock;

use tauri_leptos_core::logging::{self, LogGuard};

/// The in-process single-origin server, always on an ephemeral port:
/// `site` builds embed the SSR frontend (bundled resources), `dev`
/// builds keep the api local and reverse-proxy pages/assets from the
/// `cargo leptos watch` server (android: through `adb reverse`).
mod server {

    /// The embedded frontend: SSR + assets from the bundled resources.
    #[cfg(all(feature = "site", not(dev)))]
    fn site_router(
        app: &tauri::App,
        addr: std::net::SocketAddr,
        config: &tauri_leptos_core::config::AppConfig,
    ) -> tauri::Result<axum::Router> {
        use tauri::Manager;
        let resource_dir = app.path().resource_dir()?;
        // Relative site_root resolves against resource_dir; absent =
        // the bundled "site" map.
        let resource_site = config.site_root_resolved(&resource_dir, resource_dir.join("site"));
        tracing::info!(base = %resource_site.display(), "serving bundled resources");
        let assets = tauri_leptos_core::assets::StaticAssets::from_tauri_fs(
            app.handle().clone(),
            resource_site,
        );
        let options = tauri_leptos_ui::server::leptos_options(addr);
        Ok(tauri_leptos_ui::server::router(options, assets, config))
    }

    /// Start the in-process single-origin server on an ephemeral port
    /// and return the bound address — the window is created on it
    /// afterwards, so no fixed port can ever conflict with something
    /// else on the user's machine.
    pub fn start(app: &tauri::App) -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
        use std::net::SocketAddr;

        use tauri_leptos_core::server::Server;

        // Strict: first launch seeds the default config, a broken one
        // keeps the app from starting.
        let ctx = tauri_leptos_core::bootstrap::Ctx::from_tauri(app.handle())?;

        let server = Server::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let addr = server.local_addr()?;
        // Dev: api in-process + reverse proxy to the watch (state
        // survives frontend rebuilds); release: the embedded site.
        #[cfg(dev)]
        let router = tauri_leptos_core::server::dev_router(&ctx.config);
        #[cfg(not(dev))]
        let router = site_router(app, addr, &ctx.config)?;
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
