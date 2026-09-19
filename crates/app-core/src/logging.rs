//! Unified tracing setup, one writer per platform: stderr on desktop and
//! server (journald-friendly under systemd), logcat on Android, plus an
//! opt-in daily-rolling file layer for the headless server.

use std::path::Path;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

/// Keeps the non-blocking file appender alive; hold it for the whole
/// process lifetime or buffered log lines are lost on exit.
pub struct LogGuard {
    _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Initialize the global subscriber. `log_dir` enables the file layer
/// (ignored on Android, where everything goes to logcat).
#[cfg(not(target_os = "android"))]
pub fn init(log_dir: Option<&Path>) -> LogGuard {
    use std::io::IsTerminal;

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal());
    let (file_layer, guard) = match log_dir {
        Some(dir) => {
            let (writer, guard) =
                tracing_appender::non_blocking(tracing_appender::rolling::daily(dir, "app.log"));
            let layer = tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false);
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };
    tracing_subscriber::registry()
        .with(env_filter())
        .with(stderr_layer)
        .with(file_layer)
        .init();
    LogGuard { _guard: guard }
}

#[cfg(target_os = "android")]
pub fn init(_log_dir: Option<&Path>) -> LogGuard {
    tracing_subscriber::registry()
        .with(env_filter())
        .with(paranoid_android::layer("tauri-leptos"))
        .init();
    LogGuard { _guard: None }
}
