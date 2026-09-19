//! Headless entry point: the same core server the Tauri shell embeds,
//! run as a plain unix daemon (systemd-friendly: stderr logging,
//! *_DIRECTORY env vars honored by the paths module).

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tauri_leptos_core::config::{AppConfig, CONFIG_FILE};
use tauri_leptos_core::paths::AppPaths;
use tauri_leptos_core::{APP_DIR_ENV, logging, server};

#[derive(Parser)]
#[command(
    name = "tauri-leptos-cli",
    version,
    about = "tauri-leptos headless server"
)]
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
    /// Run the HTTP server
    Serve {
        /// Listen address (overrides config.toml)
        #[arg(long)]
        listen: Option<SocketAddr>,
        /// Frontend bundle directory (overrides config.toml)
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

/// Read the app-dir env var; an empty value must act as unset, which is
/// why this is a manual read and not clap's `env` attribute.
fn app_dir_from_env() -> Option<PathBuf> {
    std::env::var(APP_DIR_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let app_dir = cli.app_dir.or_else(app_dir_from_env);
    let paths = AppPaths::resolve_standalone(app_dir.as_deref())?;

    match cli.command {
        Command::Serve {
            listen,
            site_root,
            log_to_file,
        } => {
            // Invalid config is a hard error: systemd must see the failure.
            let config = AppConfig::load(&paths)?;
            let listen = listen.unwrap_or(config.listen);
            let site_root = site_root
                .or_else(|| config.site_root.clone())
                .unwrap_or_else(|| PathBuf::from("target/site"));
            let log_to_file = log_to_file || config.log_to_file;

            let _log_guard = logging::init(log_to_file.then_some(paths.app_log_dir.as_path()));
            paths.ensure_dirs()?;
            let leptos_options = tauri_leptos_ui::server::leptos_options(&site_root, listen);
            let app = tauri_leptos_ui::server::router(leptos_options);
            let srv = server::Server::bind(listen)?;
            tracing::info!(addr = %srv.local_addr()?, site_root = %site_root.display(), "serving");
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
