//! The app server as a plain unix daemon (systemd-friendly: stderr
//! logging, *_DIRECTORY env vars honored by the paths module): the
//! single-origin router (SSR frontend + api), a thin wrapper around
//! `core::app`. A remote api server is just this with `cors_origins`
//! set — the site it also serves is harmless. Dev happens in the tauri
//! shell (`cargo tauri dev`), whose ephemeral origin is a plain http
//! server — open it in a browser for browser work.

use std::error::Error;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tauri_leptos_core::bootstrap::Ctx;
use tauri_leptos_core::config::{AppConfig, CONFIG_FILE};
use tauri_leptos_core::logging;
use tauri_leptos_core::paths::{AppPaths, app_dir_from_env};

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
    /// Defaults to `serve` — `cargo leptos watch` runs this binary
    /// with no arguments.
    #[command(subcommand)]
    command: Option<Command>,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();

    let command = cli.command.unwrap_or(Command::Serve {
        host: None,
        port: None,
        log_to_file: false,
    });
    match command {
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

            tauri_leptos_core::app::app(ctx).serve(listen)?.await?;
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
