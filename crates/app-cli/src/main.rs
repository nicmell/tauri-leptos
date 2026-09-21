//! The app server as a plain unix daemon (systemd-friendly: stderr
//! logging, *_DIRECTORY env vars honored by the paths module). A thin
//! wrapper around the single-origin router; what it serves depends on
//! the build:
//!
//! - `site` (default): embedded SSR frontend + api.
//! - `dev`: api here, pages/assets reverse-proxied from the
//!   `cargo leptos watch` server — api state survives frontend rebuilds.
//! - neither: api only (a remote api server for frontends elsewhere).

#[cfg(all(feature = "site", feature = "dev"))]
compile_error!("features `site` and `dev` are mutually exclusive");

use std::error::Error;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tauri_leptos_core::config::{AppConfig, CONFIG_FILE};
use tauri_leptos_core::paths::{AppPaths, app_dir_from_env};
use tauri_leptos_core::{logging, server};

#[derive(Parser)]
#[command(name = "tauri-leptos-cli", version, about = "tauri-leptos server")]
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
    /// Run the server on one origin (what it serves depends on the
    /// build features; see the crate docs)
    Serve {
        /// Bind address (overrides the configured host)
        #[arg(long)]
        host: Option<IpAddr>,
        /// Bind port (overrides the configured port)
        #[arg(long)]
        port: Option<u16>,
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

/// Full build: embedded SSR frontend + api. The site must be a release
/// build (`cargo leptos build --release`).
#[cfg(feature = "site")]
fn app_router(config: &AppConfig, listen: SocketAddr) -> axum::Router {
    let site_root = config
        .site_root
        .clone()
        .unwrap_or_else(AppPaths::default_site_root);
    if !site_root.join("pkg").exists() {
        tracing::warn!(
            site_root = %site_root.display(),
            "site root looks empty - run `cargo leptos build --release` \
             and install/point `site_root` in the config at it"
        );
    }
    tracing::info!(site_root = %site_root.display(), "single-origin server (ssr + api)");
    let options = tauri_leptos_ui::server::leptos_options(listen);
    tauri_leptos_ui::server::router(options, tauri_leptos_core::assets::DirAssets(site_root))
}

/// Dev build: api lives here (stable across frontend rebuilds), pages
/// and assets come from the watch server through the reverse proxy.
#[cfg(all(feature = "dev", not(feature = "site")))]
fn app_router(config: &AppConfig, _listen: SocketAddr) -> axum::Router {
    use tauri_leptos_core::assets::{Assets, ProxyAssets};
    tracing::info!(upstream = %config.dev.upstream, "dev server (api + proxy to the watch)");
    server::api_router()
        .merge(ProxyAssets(config.dev.upstream.clone()).into_router(axum::Router::new()))
}

/// Api-only build: a remote api server for frontends running elsewhere.
#[cfg(not(any(feature = "site", feature = "dev")))]
fn app_router(_config: &AppConfig, _listen: SocketAddr) -> axum::Router {
    tracing::info!("api-only server");
    server::api_router().route(
        "/",
        axum::routing::get(|| async { "tauri-leptos api server" }),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let app_dir = cli.app_dir.or_else(app_dir_from_env);
    let paths = AppPaths::resolve_standalone(app_dir.as_deref())?;

    match cli.command {
        Command::Serve {
            host,
            port,
            log_to_file,
        } => {
            // Invalid config is a hard error: systemd must see the failure.
            let config = AppConfig::load(&paths)?;
            let listen = SocketAddr::new(
                host.unwrap_or_else(|| config.listen.ip()),
                port.unwrap_or_else(|| config.listen.port()),
            );
            let log_to_file = log_to_file || config.log_to_file;

            let _log_guard = logging::init(log_to_file.then_some(paths.app_log_dir.as_path()));
            paths.ensure_dirs()?;

            let app = app_router(&config, listen);

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
