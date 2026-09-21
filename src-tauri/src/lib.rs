// Dev runs (cfg(dev), emitted by tauri-build) proxy to the watch and
// need no leptos: `-- --no-default-features` is the fast path. Release
// builds must embed the frontend.
#[cfg(all(not(dev), not(feature = "site")))]
compile_error!("release builds need the `site` feature (default)");

use std::sync::OnceLock;

use tauri_leptos_core::logging::{self, LogGuard};

/// The in-process single-origin server, always on an ephemeral port:
/// `site` builds embed the SSR frontend (bundled resources), dev runs
/// (`cfg(dev)`) keep the api local and reverse-proxy pages/assets from
/// the `cargo leptos watch` server (android: through `adb reverse`).
mod server {
    use tauri_leptos_core::bootstrap::Ctx;

    /// The embedded frontend: SSR + assets from the bundled resources.
    #[cfg(all(feature = "site", not(dev)))]
    fn site_router(
        handle: tauri::AppHandle,
        resource_dir: &std::path::Path,
        ctx: &Ctx,
        addr: std::net::SocketAddr,
    ) -> axum::Router {
        // Relative site_root resolves against resource_dir; absent =
        // the bundled "site" map.
        let resource_site = ctx
            .config
            .site_root_resolved(resource_dir, resource_dir.join("site"));
        tracing::info!(base = %resource_site.display(), "serving bundled resources");
        let assets = tauri_leptos_core::assets::StaticAssets::from_tauri_fs(handle, resource_site);
        let options = tauri_leptos_ui::server::leptos_options(addr);
        tauri_leptos_ui::server::router(options, assets, &ctx.config)
    }

    /// Start the in-process single-origin server on an ephemeral port
    /// and return the bound address — the window is created on it
    /// afterwards, so no fixed port can ever conflict with something
    /// else on the user's machine.
    pub fn start(
        app: &tauri::App,
    ) -> Result<std::net::SocketAddr, tauri_leptos_core::app::BoxError> {
        // Strict: first launch seeds the default config, a broken one
        // keeps the app from starting.
        let ctx = Ctx::from_tauri(app.handle())?;

        let core_app = tauri_leptos_core::app::app(ctx);
        // Dev: api in-process + reverse proxy to the watch (state
        // survives frontend rebuilds); release: the embedded site.
        #[cfg(dev)]
        let core_app =
            core_app.with_router(|ctx, _| Ok(tauri_leptos_core::server::dev_router(&ctx.config)));
        #[cfg(not(dev))]
        let core_app = {
            use tauri::Manager;
            let handle = app.handle().clone();
            let resource_dir = app.path().resource_dir()?;
            core_app.with_router(move |ctx, addr| Ok(site_router(handle, &resource_dir, ctx, addr)))
        };

        let serving = core_app.serve(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))?;
        let addr = serving.addr();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = serving.await {
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
            let url = format!(
                "http://{}",
                server::start(app).map_err(|e| -> Box<dyn std::error::Error> { e })?
            );
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
