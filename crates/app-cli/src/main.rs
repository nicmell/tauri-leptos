//! The API server as a plain unix daemon (systemd-friendly: stderr
//! logging, *_DIRECTORY env vars honored by the paths module). In dev it
//! is the stateful half of the split: `cargo leptos watch` rebuilds the
//! frontend while this process — and its state — stays up.

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::http::{HeaderValue, Method, header};
use clap::{Parser, Subcommand};
use tauri_leptos_core::config::{AppConfig, CONFIG_FILE};
use tauri_leptos_core::paths::{AppPaths, app_dir_from_env};
use tauri_leptos_core::{logging, server};

#[derive(Parser)]
#[command(name = "tauri-leptos-cli", version, about = "tauri-leptos api server")]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "Put config/data/cache/logs under this single directory \
                (overrides platform paths; env: TAURI_LEPTOS_APP_DIR)"
    )]
    app_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server: SSR frontend + API on one origin (default), or
    /// API only with --headless (the dev split)
    Serve {
        /// Listen address (overrides the configured api address)
        #[arg(long)]
        listen: Option<SocketAddr>,
        /// API only, no frontend: dev mode alongside `cargo leptos watch`
        #[arg(long)]
        headless: bool,
        /// Frontend bundle directory (full mode; default: the platform
        /// share dir on Linux, target/site elsewhere)
        #[arg(long)]
        site_root: Option<PathBuf>,
        /// Also write logs to a daily-rolling file in the app log dir
        #[arg(long)]
        log_to_file: bool,
    },
    /// Inspect or create the configuration file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Write the default config.toml (refuses to overwrite)
    Write {
        /// Target path (default: <app config dir>/config.toml)
        path: Option<PathBuf>,
    },
    /// Check that a config file parses
    Validate {
        /// Path to check (default: <app config dir>/config.toml)
        path: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let app_dir = cli.app_dir.or_else(app_dir_from_env);
    let paths = AppPaths::resolve_standalone(app_dir.as_deref())?;

    match cli.command {
        Command::Serve {
            listen,
            headless,
            site_root,
            log_to_file,
        } => {
            // Invalid config is a hard error: systemd must see the failure.
            let config = AppConfig::load(&paths)?;
            // In full mode this is the address of the whole app.
            let listen = listen.unwrap_or(config.api_addr);
            let log_to_file = log_to_file || config.log_to_file;

            let _log_guard = logging::init(log_to_file.then_some(paths.app_log_dir.as_path()));
            paths.ensure_dirs()?;

            let app = if headless {
                // Dev split: the page comes from another origin (the
                // frontend server on :3000, or the tauri webview).
                let cors = tower_http::cors::CorsLayer::new()
                    .allow_origin([
                        HeaderValue::from_static("http://127.0.0.1:3000"),
                        HeaderValue::from_static("tauri://localhost"),
                        HeaderValue::from_static("http://tauri.localhost"),
                    ])
                    .allow_methods([Method::GET, Method::POST])
                    .allow_headers([header::CONTENT_TYPE]);
                tracing::info!("headless: api only, frontend expected on :3000");
                server::api_router()
                    .route(
                        "/",
                        axum::routing::get(|| async {
                            "tauri-leptos api server - the frontend is served on :3000"
                        }),
                    )
                    .layer(cors)
            } else {
                // Full single-origin server: SSR pages + API, the systemd
                // deployment mode. The site must be a release build
                // (`cargo leptos build --release`).
                let site_root = site_root.unwrap_or_else(AppPaths::default_site_root);
                if !site_root.join("pkg").exists() {
                    tracing::warn!(
                        site_root = %site_root.display(),
                        "site root looks empty - run `cargo leptos build --release` \
                         and install/point --site-root at it"
                    );
                }
                tracing::info!(site_root = %site_root.display(), "full server (ssr + api)");
                let options = tauri_leptos_ui::server::leptos_options(listen);
                tauri_leptos_ui::server::router(
                    options,
                    std::sync::Arc::new(tauri_leptos_ui::server::DirAssets(site_root)),
                )
            };

            let srv = server::Server::bind(listen)?;
            tracing::info!(addr = %srv.local_addr()?, "server up");
            tracing::debug!(?paths, "resolved app paths");
            srv.serve(app, server::shutdown_signal()).await?;
        }
        Command::Config { action } => {
            let default_path = || paths.app_config_dir.join(CONFIG_FILE);
            match action {
                ConfigAction::Write { path } => {
                    let path = path.unwrap_or_else(default_path);
                    if path.exists() {
                        return Err(format!("{} already exists", path.display()).into());
                    }
                    AppConfig::default().seed(&path)?;
                    println!("wrote {}", path.display());
                }
                ConfigAction::Validate { path } => {
                    let path = path.unwrap_or_else(default_path);
                    let text = std::fs::read_to_string(&path)?;
                    AppConfig::parse(&text)?;
                    println!("{} is valid", path.display());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_consistent() {
        super::Cli::command().debug_assert();
    }
}
