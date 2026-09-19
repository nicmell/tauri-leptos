use std::sync::OnceLock;

use tauri::Manager;
use tauri_leptos_core::config::AppConfig;
use tauri_leptos_core::logging::{self, LogGuard};
use tauri_leptos_core::paths::AppPaths;

/// Where the frontend bundle lives for the in-process server.
///
/// Desktop/iOS: the bundled resources are real files — `resource_dir()/site`
/// (falls back to config/cargo-leptos output when running unbundled).
/// Android: APK assets are not filesystem files; `site.tar` (bundled as a
/// single resource, since asset directories cannot be enumerated) is
/// unpacked once per app version into local data.
#[cfg(any(feature = "ssr", target_os = "android"))]
fn resolve_site_root(
    app: &tauri::App,
    config: &AppConfig,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    #[cfg(target_os = "android")]
    {
        use std::io::Read;
        use tauri_plugin_fs::FsExt;

        let _ = config;
        let site_dir = app.path().app_local_data_dir()?.join("site");
        let version = app.package_info().version.to_string();
        let marker = site_dir.join(".version");
        if std::fs::read_to_string(&marker).ok().as_deref() != Some(version.as_str()) {
            tracing::info!(dir = %site_dir.display(), "unpacking site.tar (version {version})");
            let _ = std::fs::remove_dir_all(&site_dir);
            std::fs::create_dir_all(&site_dir)?;
            let archive_path = app
                .path()
                .resolve("site.tar", tauri::path::BaseDirectory::Resource)?;
            let mut open_options = tauri_plugin_fs::OpenOptions::new();
            open_options.read(true);
            let mut file = app.fs().open(archive_path, open_options)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            tar::Archive::new(bytes.as_slice()).unpack(&site_dir)?;
            std::fs::write(&marker, &version)?;
        }
        Ok(site_dir)
    }
    #[cfg(not(target_os = "android"))]
    {
        let bundled = app.path().resource_dir()?.join("site");
        if bundled.join("index.html").exists() || bundled.join("pkg").exists() {
            return Ok(bundled);
        }
        // Unbundled run (e.g. cargo tauri dev -f ssr): fall back to config
        // or the cargo-leptos output in the workspace.
        Ok(config
            .site_root
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("target/site")))
    }
}

/// The in-process SSR server ships in production builds and on mobile;
/// desktop dev attaches to `cargo leptos watch` instead.
#[cfg(any(feature = "ssr", target_os = "android"))]
fn start_server(app: &tauri::App, config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_leptos_core::server::Server;

    let site_root = resolve_site_root(app, config)?;
    tracing::info!(site_root = %site_root.display(), "in-process server");
    let leptos_options = tauri_leptos_ui::server::leptos_options(&site_root, config.listen);
    let app_router = tauri_leptos_ui::server::router(leptos_options);
    let server = Server::bind(config.listen)?;
    tauri::async_runtime::spawn(async move {
        // No shutdown signal: the server lives as long as the process.
        if let Err(e) = server.serve(app_router, std::future::pending()).await {
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

    // Same config as the headless server; the shell degrades to defaults
    // instead of refusing to open a window.
    let config = AppPaths::resolve_standalone(None)
        .map_err(|e| eprintln!("[config] paths unavailable: {e}"))
        .and_then(|paths| {
            AppConfig::load(&paths).map_err(|e| eprintln!("[config] ignoring config: {e}"))
        })
        .unwrap_or_default();
    let port = config.listen.port();

    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_fs::init());

    builder
        .setup(move |app| {
            #[cfg(any(feature = "ssr", target_os = "android"))]
            start_server(app, &config)?;
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
