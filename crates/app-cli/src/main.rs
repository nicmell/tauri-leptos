//! Headless entry point: the same core server the Tauri shell embeds,
//! run as a plain unix daemon (systemd-friendly: stderr logging,
//! *_DIRECTORY env vars honored by the paths module).

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
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
        #[arg(long, default_value = "127.0.0.1:3000")]
        listen: SocketAddr,
        /// Also write logs to a daily-rolling file in the app log dir
        #[arg(long)]
        log_to_file: bool,
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
            log_to_file,
        } => {
            let _log_guard = logging::init(log_to_file.then_some(paths.app_log_dir.as_path()));
            paths.ensure_dirs()?;
            let srv = server::Server::bind(listen)?;
            tracing::info!(addr = %srv.local_addr()?, "serving");
            tracing::debug!(?paths, "resolved app paths");
            srv.serve(server::shutdown_signal()).await?;
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
