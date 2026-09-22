//! The shared bootstrap: resolved paths, loaded config and the host —
//! everything [`Ctx::router`] needs to build the one router. One
//! strict path everywhere: a missing config is seeded with defaults on
//! first run ([`AppConfig::load`]), an invalid one refuses to start.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::config::{AppConfig, ConfigError};
use crate::paths::{AppPaths, PathsError, app_dir_from_env};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Where the app runs — fixed at bootstrap, consumed by
/// [`Ctx::router`] and [`Ctx::listen`] at runtime (never a compile
/// branch in the entrypoints).
#[derive(Clone)]
enum Host {
    Standalone,
    #[cfg(feature = "tauri")]
    Tauri {
        dev: bool,
    },
}

/// Resolved paths + loaded config + host + the resource router: what
/// every entrypoint starts from.
#[derive(Clone)]
pub struct Ctx {
    pub paths: AppPaths,
    pub config: AppConfig,
    host: Host,
    /// The host's resource router from [`crate::resources`]: the site
    /// from disk, from the Tauri resource store, or proxied from the
    /// watch.
    resources: axum::Router,
}

#[derive(Debug)]
pub enum BootstrapError {
    Paths(PathsError),
    Config(ConfigError),
    #[cfg(feature = "tauri")]
    Tauri(tauri::Error),
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paths(e) => e.fmt(f),
            Self::Config(e) => e.fmt(f),
            #[cfg(feature = "tauri")]
            Self::Tauri(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for BootstrapError {}

impl Ctx {
    /// Standalone bootstrap from the command-line parameters (the
    /// watch server runs the bare binary: everything `None`/`false`).
    /// `app_dir` falls back to the app-dir env var; `host`/`port`
    /// override the configured bind, `log_to_file` ORs into the
    /// config — the flags land in the [`AppConfig`], no setters at
    /// the call site.
    pub fn from_cli(
        app_dir: Option<PathBuf>,
        host: Option<std::net::IpAddr>,
        port: Option<u16>,
        log_to_file: bool,
    ) -> Result<Self, BootstrapError> {
        let app_dir = app_dir.or_else(app_dir_from_env);
        let paths =
            AppPaths::resolve_standalone(app_dir.as_deref()).map_err(BootstrapError::Paths)?;
        let mut config = AppConfig::load(&paths).map_err(BootstrapError::Config)?;
        if let Some(host) = host {
            config.listen.set_ip(host);
        }
        if let Some(port) = port {
            config.listen.set_port(port);
        }
        config.log_to_file |= log_to_file;

        let resources = crate::resources::standalone(&config, &paths);
        Ok(Self {
            paths,
            config,
            host: Host::Standalone,
            resources,
        })
    }

    /// Bootstrap inside a Tauri app, from the real path API. `dev` is
    /// the shell's `cfg!(dev)`. First launch seeds the default config;
    /// an invalid one is a hard error — the app must not start on a
    /// broken config.
    ///
    /// Dev runs honor the app-dir env var (exported by the dev run
    /// configs), so the shell and the watch share ONE config store —
    /// `appdir/` — instead of dev edits landing in the platform config.
    /// (Android dev cannot see the host env and keeps the device's
    /// platform paths; the defaults there are already correct.)
    #[cfg(feature = "tauri")]
    pub fn from_tauri(app: &tauri::App, dev: bool) -> Result<Self, BootstrapError> {
        let handle = app.handle().clone();
        let paths = match app_dir_from_env().filter(|_| dev) {
            Some(dir) => AppPaths::from_root(dir),
            None => AppPaths::from_tauri(&handle).map_err(BootstrapError::Tauri)?,
        };
        let config = AppConfig::load(&paths).map_err(BootstrapError::Config)?;

        let resources = if dev {
            crate::resources::proxy(&config.dev.upstream)
        } else {
            crate::resources::tauri(handle, &config).map_err(BootstrapError::Tauri)?
        };

        Ok(Self {
            paths,
            config,
            host: Host::Tauri { dev },
            resources,
        })
    }

    /// The address to bind, host-inferred: the configured listen
    /// address standalone, an ephemeral port in the tauri shell (the
    /// window is created on the bound address afterwards, so no fixed
    /// port can ever conflict on the user's machine).
    pub(crate) fn listen(&self) -> SocketAddr {
        match &self.host {
            Host::Standalone => self.config.listen,
            #[cfg(feature = "tauri")]
            Host::Tauri { .. } => SocketAddr::from(([127, 0, 0, 1], 0)),
        }
    }

    /// The host's resource router built at bootstrap, for reuse
    /// anywhere in the app.
    pub fn resources(&self) -> axum::Router {
        self.resources.clone()
    }

    /// The one router, host-inferred: the leptos pages + api in front
    /// and the bootstrap-built resources as the fallback (a plain 404
    /// on unknown paths) — or, in the shell's dev runs, the api with
    /// everything else reverse-proxied from the watch.
    pub fn router(&self, addr: SocketAddr) -> axum::Router {
        let api = crate::server::api_router(&self.config.cors_origins);
        match &self.host {
            #[cfg(feature = "tauri")]
            Host::Tauri { dev: true } => {
                tracing::info!(upstream = %self.config.dev.upstream, "dev server (api + proxy to the watch)");
                api.merge(self.resources())
            }
            _ => tauri_leptos_ui::server::router(addr, self.config.api_base.clone())
                .merge(api)
                .fallback_service(self.resources()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG_FILE;

    #[test]
    fn strict_bootstrap_rejects_invalid_config() {
        let dir = std::env::temp_dir().join(format!("tl-boot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = AppPaths::from_root(&dir);
        std::fs::create_dir_all(&paths.app_config_dir).expect("mkdir");
        std::fs::write(paths.app_config_dir.join(CONFIG_FILE), "nonsense = true\n").expect("write");
        assert!(Ctx::from_cli(Some(dir.clone()), None, None, false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_run_seeds_and_starts() {
        let dir = std::env::temp_dir().join(format!("tl-boot-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::from_cli(Some(dir.clone()), None, None, false).expect("seeds and starts");
        assert_eq!(ctx.config, AppConfig::default());
        assert!(ctx.paths.app_config_dir.join(CONFIG_FILE).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
