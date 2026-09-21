//! The shared bootstrap: resolved paths + loaded config, and the
//! router for non-`site` builds. One strict path everywhere — a
//! missing config is seeded with defaults on first run
//! ([`AppConfig::load`]), an invalid one refuses to start.

use std::fmt;
use std::path::PathBuf;

use crate::config::{AppConfig, ConfigError};
use crate::paths::{AppPaths, PathsError, app_dir_from_env};

/// Resolved paths + loaded config: what every entrypoint starts from.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub paths: AppPaths,
    pub config: AppConfig,
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
        Ok(Self { paths, config })
    }

    /// Bootstrap inside a Tauri app, from the real path API. First
    /// launch seeds the default config; an invalid one is a hard
    /// error — the app must not start on a broken config.
    #[cfg(feature = "tauri")]
    pub fn from_tauri<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
    ) -> Result<Self, BootstrapError> {
        let paths = AppPaths::from_tauri(app).map_err(BootstrapError::Tauri)?;
        let config = AppConfig::load(&paths).map_err(BootstrapError::Config)?;
        Ok(Self { paths, config })
    }

    /// The router for non-`site` builds: the api-only server. `site`
    /// builds construct the leptos router themselves (core cannot
    /// depend on the ui crate); the tauri dev shell picks
    /// [`crate::server::dev_router`] under `cfg(dev)`.
    pub fn router(&self) -> axum::Router {
        crate::server::api_only_router(&self.config)
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
