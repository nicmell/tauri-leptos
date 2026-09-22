/// The in-process single-origin server, always on an ephemeral port:
/// the embedded SSR frontend (bundled resources), or — in dev runs —
/// the api with pages/assets reverse-proxied from the `cargo leptos
/// watch` server (android: through `adb reverse`). The mode is data:
/// `cfg!(dev)` flows into the [`Ctx`] host, the router is inferred in
/// core.
mod server {
    use tauri_leptos_core::bootstrap::Ctx;

    /// Start the server on an ephemeral port and return the bound
    /// address — the window is created on it afterwards, so no fixed
    /// port can ever conflict with something else on the user's
    /// machine.
    pub fn start(
        app: &tauri::App,
    ) -> Result<std::net::SocketAddr, tauri_leptos_core::app::BoxError> {
        use std::sync::OnceLock;

        use tauri_leptos_core::logging::{self, LogGuard};

        // Keep the file-appender guard (if any) alive for the process.
        static LOG_GUARD: OnceLock<LogGuard> = OnceLock::new();

        // Strict: first launch seeds the default config, a broken one
        // keeps the app from starting.
        let ctx = Ctx::from_tauri(app, cfg!(dev))?;

        // Logging starts here, once the config says whether to add the
        // rolling file.
        let _ = LOG_GUARD.set(logging::init(
            ctx.config
                .log_to_file
                .then(|| ctx.paths.app_log_dir.clone())
                .as_deref(),
        ));
        let core_app = tauri_leptos_core::app::app(ctx)?;
        let addr = core_app.addr();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = core_app.start().await {
                tracing::error!("http server exited: {e}");
            }
        });
        Ok(addr)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
