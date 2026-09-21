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
use tauri_leptos_core::bootstrap::Ctx;
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
/// build (`cargo leptos build --release`). Non-`site` builds get their
/// router from [`Ctx::router`] (core owns that branching).
#[cfg(feature = "site")]
fn site_router(ctx: &Ctx, listen: SocketAddr) -> axum::Router {
    // Relative site_root resolves against the config dir; absent =
    // the cargo-leptos output (dev runs from the workspace).
    let site_root = ctx
        .config
        .site_root_resolved(&ctx.paths.app_config_dir, "target/site");
    if !site_root.join("pkg").exists() {
        tracing::warn!(
            site_root = %site_root.display(),
            "site root looks empty - run `cargo leptos build --release` \
             and install/point `site_root` in the config at it"
        );
    }
    tracing::info!(site_root = %site_root.display(), "single-origin server (ssr + api)");
    let options = tauri_leptos_ui::server::leptos_options(listen);
    tauri_leptos_ui::server::router(
        options,
        tauri_leptos_core::assets::StaticAssets::from_site_root(site_root),
        &ctx.config,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            host,
            port,
            log_to_file,
        } => {
            // Invalid config is a hard error: systemd must see the failure.
            let ctx = Ctx::resolve(cli.app_dir)?;
            let listen = SocketAddr::new(
                host.unwrap_or_else(|| ctx.config.listen.ip()),
                port.unwrap_or_else(|| ctx.config.listen.port()),
            );
            let log_to_file = log_to_file || ctx.config.log_to_file;

            let _log_guard = logging::init(log_to_file.then_some(ctx.paths.app_log_dir.as_path()));
            ctx.paths.ensure_dirs()?;

            #[cfg(feature = "site")]
            let app = site_router(&ctx, listen);
            #[cfg(not(feature = "site"))]
            let app = ctx.router();

            let srv = server::Server::bind(listen)?;
            tracing::info!(addr = %srv.local_addr()?, "server up");
            tracing::debug!(paths = ?ctx.paths, "resolved app paths");
            srv.serve(app, server::shutdown_signal()).await?;
        }
        Command::Config { action } => {
            let paths =
                AppPaths::resolve_standalone(cli.app_dir.or_else(app_dir_from_env).as_deref())?;
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
