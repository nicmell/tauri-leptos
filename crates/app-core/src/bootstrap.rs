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

/// What the router's fallback serves — the only thing that varies
/// between hosts, built once at bootstrap (site-root resolution,
/// warnings and `resource_dir` errors all happen at startup, not per
/// request or at router build).
#[derive(Clone)]
enum Frontend {
    /// The static site bundle (`ServeDir`; Tokio backend standalone,
    /// the tauri fs backend in the shell).
    Site(axum::Router),
    /// The shell's dev runs: everything but the api reverse-proxied
    /// from the watch server ("empty" assets).
    #[cfg(feature = "tauri")]
    Proxy(axum::Router),
}

/// Resolved paths + loaded config + frontend: what every entrypoint
/// starts from.
#[derive(Clone)]
pub struct Ctx {
    pub paths: AppPaths,
    pub config: AppConfig,
    frontend: Frontend,
    /// Shell binds ephemeral (the window is created on the bound
    /// address); standalone binds the configured listen.
    ephemeral_bind: bool,
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

        // Relative site_root resolves against the config dir; absent =
        // the cargo-leptos output (dev workspace runs). Pin the root:
        // symlinked/relative roots resolve once, here.
        let site_root = config.site_root_resolved(&paths.app_config_dir, "target/site");
        let site_root = site_root.canonicalize().unwrap_or(site_root);
        if !site_root.join("pkg").exists() {
            tracing::warn!(
                site_root = %site_root.display(),
                "site root looks empty - run `cargo leptos build --release` \
                 and install/point `site_root` in the config at it"
            );
        }
        tracing::info!(site_root = %site_root.display(), "single-origin server (ssr + api)");
        let assets =
            tower_http::services::ServeDir::new(site_root).append_index_html_on_directories(false);
        let frontend = Frontend::Site(axum::Router::new().fallback_service(assets));

        Ok(Self {
            paths,
            config,
            frontend,
            ephemeral_bind: false,
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
        use tauri::Manager;

        let handle = app.handle().clone();
        let paths = match app_dir_from_env().filter(|_| dev) {
            Some(dir) => AppPaths::from_root(dir),
            None => AppPaths::from_tauri(&handle).map_err(BootstrapError::Tauri)?,
        };
        let config = AppConfig::load(&paths).map_err(BootstrapError::Config)?;

        let frontend = if dev {
            // Api local (state survives frontend rebuilds), everything
            // else reverse-proxied from the watch server.
            tracing::info!(upstream = %config.dev.upstream, "dev server (api + proxy to the watch)");
            Frontend::Proxy(axum_reverse_proxy::ReverseProxy::new("/", &config.dev.upstream).into())
        } else {
            // Relative site_root resolves against resource_dir; absent
            // = the bundled "site" map.
            let resource_dir = handle
                .path()
                .resource_dir()
                .map_err(BootstrapError::Tauri)?;
            let resource_site = config.site_root_resolved(&resource_dir, resource_dir.join("site"));
            tracing::info!(base = %resource_site.display(), "serving bundled resources");
            let assets = tower_http::services::ServeDir::with_backend(
                resource_site,
                crate::assets::TauriBackend::new(handle),
            )
            .append_index_html_on_directories(false);
            Frontend::Site(axum::Router::new().fallback_service(assets))
        };

        Ok(Self {
            paths,
            config,
            frontend,
            ephemeral_bind: true,
        })
    }

    /// The address to bind, host-inferred: the configured listen
    /// address standalone, an ephemeral port in the tauri shell (the
    /// window is created on the bound address afterwards, so no fixed
    /// port can ever conflict on the user's machine).
    pub(crate) fn listen(&self) -> SocketAddr {
        if self.ephemeral_bind {
            SocketAddr::from(([127, 0, 0, 1], 0))
        } else {
            self.config.listen
        }
    }

    /// The one router: the leptos pages + api in front, the
    /// bootstrap-built frontend (site assets or the dev proxy) as the
    /// fallback — unknown paths are `ServeDir`s plain 404.
    pub fn router(&self, addr: SocketAddr) -> axum::Router {
        let api = crate::server::api_router(&self.config.cors_origins);
        match &self.frontend {
            Frontend::Site(assets) => {
                tauri_leptos_ui::server::router(addr, self.config.api_base.clone())
                    .merge(api)
                    .fallback_service(assets.clone())
            }
            #[cfg(feature = "tauri")]
            Frontend::Proxy(proxy) => api.merge(proxy.clone()),
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
