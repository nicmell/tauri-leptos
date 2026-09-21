//! The shared bootstrap: resolved paths, loaded config and the host —
//! everything [`Ctx::router`] needs to build the one router. One
//! strict path everywhere: a missing config is seeded with defaults on
//! first run ([`AppConfig::load`]), an invalid one refuses to start.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

#[cfg(feature = "tauri")]
use crate::assets::ProxyAssets;
use crate::assets::{Assets, StaticAssets};
use crate::config::{AppConfig, ConfigError};
use crate::paths::{AppPaths, PathsError, app_dir_from_env};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Where the app runs — the only thing that varies between the
/// standalone server and the tauri shell, fixed at bootstrap and
/// consumed by [`Ctx::router`] at runtime (never a compile branch in
/// the entrypoints).
#[derive(Debug, Clone)]
enum Host {
    Standalone,
    #[cfg(feature = "tauri")]
    Tauri {
        app: tauri::AppHandle,
        dev: bool,
    },
}

/// Resolved paths + loaded config + host: what every entrypoint
/// starts from.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub paths: AppPaths,
    pub config: AppConfig,
    host: Host,
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
    /// Standalone bootstrap (cli, watch server). `app_dir` = the CLI
    /// flag; the app-dir env var is the fallback.
    pub fn resolve(app_dir: Option<PathBuf>) -> Result<Self, BootstrapError> {
        let app_dir = app_dir.or_else(app_dir_from_env);
        let paths =
            AppPaths::resolve_standalone(app_dir.as_deref()).map_err(BootstrapError::Paths)?;
        let config = AppConfig::load(&paths).map_err(BootstrapError::Config)?;
        Ok(Self {
            paths,
            config,
            host: Host::Standalone,
        })
    }

    /// Bootstrap inside a Tauri app, from the real path API. `dev` is
    /// the shell's `cfg!(dev)`. First launch seeds the default config;
    /// an invalid one is a hard error — the app must not start on a
    /// broken config.
    #[cfg(feature = "tauri")]
    pub fn from_tauri(app: &tauri::App, dev: bool) -> Result<Self, BootstrapError> {
        let handle = app.handle().clone();
        let paths = AppPaths::from_tauri(&handle).map_err(BootstrapError::Tauri)?;
        let config = AppConfig::load(&paths).map_err(BootstrapError::Config)?;
        Ok(Self {
            paths,
            config,
            host: Host::Tauri { app: handle, dev },
        })
    }

    /// The one router, host-inferred: the SSR site behind the matching
    /// asset backend (std fs standalone, tauri fs in the shell), or —
    /// in the shell's dev runs — the api with everything else
    /// reverse-proxied from the watch server.
    pub fn router(&self, addr: SocketAddr) -> Result<axum::Router, BoxError> {
        match &self.host {
            Host::Standalone => {
                // Relative site_root resolves against the config dir;
                // absent = the cargo-leptos output (dev workspace runs).
                let site_root = self
                    .config
                    .site_root_resolved(&self.paths.app_config_dir, "target/site");
                if !site_root.join("pkg").exists() {
                    tracing::warn!(
                        site_root = %site_root.display(),
                        "site root looks empty - run `cargo leptos build --release` \
                         and install/point `site_root` in the config at it"
                    );
                }
                tracing::info!(site_root = %site_root.display(), "single-origin server (ssr + api)");
                Ok(self.site(addr, StaticAssets::from_site_root(site_root)))
            }
            #[cfg(feature = "tauri")]
            Host::Tauri { dev: true, .. } => {
                tracing::info!(
                    upstream = %self.config.dev.upstream,
                    "dev server (api + proxy to the watch)"
                );
                Ok(self.compose(
                    ProxyAssets(self.config.dev.upstream.clone()),
                    axum::Router::new(),
                ))
            }
            #[cfg(feature = "tauri")]
            Host::Tauri { app, .. } => {
                use tauri::Manager;
                // Relative site_root resolves against resource_dir;
                // absent = the bundled "site" map.
                let resource_dir = app.path().resource_dir()?;
                let resource_site = self
                    .config
                    .site_root_resolved(&resource_dir, resource_dir.join("site"));
                tracing::info!(base = %resource_site.display(), "serving bundled resources");
                Ok(self.site(
                    addr,
                    StaticAssets::from_tauri_fs(app.clone(), resource_site),
                ))
            }
        }
    }

    fn site(&self, addr: SocketAddr, assets: impl Assets) -> axum::Router {
        self.compose(
            assets,
            tauri_leptos_ui::server::router(addr, self.config.api_base.clone()),
        )
    }

    fn compose(&self, assets: impl Assets, pages: axum::Router) -> axum::Router {
        assets
            .into_router(pages)
            .merge(crate::server::api_router(&self.config.cors_origins))
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
        assert!(Ctx::resolve(Some(dir.clone())).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_run_seeds_and_starts() {
        let dir = std::env::temp_dir().join(format!("tl-boot-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::resolve(Some(dir.clone())).expect("seeds and starts");
        assert_eq!(ctx.config, AppConfig::default());
        assert!(ctx.paths.app_config_dir.join(CONFIG_FILE).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
