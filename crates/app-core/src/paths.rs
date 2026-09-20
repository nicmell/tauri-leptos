//! Application directories, named after the Tauri v2 path API.
//!
//! In tauri mode the shell fills [`AppPaths`] from Tauri's own `PathResolver`;
//! this module provides the standalone resolution used by the headless server,
//! reproducing Tauri's per-platform mapping (see `tauri/src/path/desktop.rs`).

use std::env;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::{APP_DIR_ENV, APP_IDENTIFIER};

/// Read the app-dir env var; an empty value must act as unset, which is
/// why this is a manual read and not clap's `env` attribute.
pub fn app_dir_from_env() -> Option<PathBuf> {
    env::var(APP_DIR_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
}

/// The app's directories, one field per Tauri path-API name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub app_config_dir: PathBuf,
    pub app_data_dir: PathBuf,
    pub app_local_data_dir: PathBuf,
    pub app_cache_dir: PathBuf,
    pub app_log_dir: PathBuf,
}

#[derive(Debug)]
pub enum PathsError {
    /// The platform home/base directories could not be determined.
    UnknownBaseDirs,
}

impl fmt::Display for PathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBaseDirs => write!(f, "platform base directories are unavailable"),
        }
    }
}

impl std::error::Error for PathsError {}

impl AppPaths {
    /// Every directory under one root — the reproducible dev/test layout
    /// selected by `--app-dir` or the app-dir env var.
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            app_config_dir: root.join("config"),
            app_data_dir: root.join("data"),
            app_local_data_dir: root.join("data"),
            app_cache_dir: root.join("cache"),
            app_log_dir: root.join("logs"),
        }
    }

    /// Resolution for processes running without Tauri (the headless server).
    ///
    /// Precedence: explicit `app_dir` (CLI flag or env, resolved by the
    /// caller) > systemd directory env vars > platform defaults.
    pub fn resolve_standalone(app_dir: Option<&Path>) -> Result<Self, PathsError> {
        if let Some(root) = app_dir {
            return Ok(Self::from_root(root));
        }
        Ok(Self::platform_defaults()?.with_systemd_overrides(|var| env::var(var).ok()))
    }

    /// Tauri's per-platform mapping, via the same `dirs` calls it uses.
    fn platform_defaults() -> Result<Self, PathsError> {
        let config = dirs::config_dir().ok_or(PathsError::UnknownBaseDirs)?;
        let data = dirs::data_dir().ok_or(PathsError::UnknownBaseDirs)?;
        let local_data = dirs::data_local_dir().ok_or(PathsError::UnknownBaseDirs)?;
        let cache = dirs::cache_dir().ok_or(PathsError::UnknownBaseDirs)?;
        let log = if cfg!(target_os = "macos") {
            dirs::home_dir()
                .ok_or(PathsError::UnknownBaseDirs)?
                .join("Library/Logs")
                .join(APP_IDENTIFIER)
        } else {
            local_data.join(APP_IDENTIFIER).join("logs")
        };
        Ok(Self {
            app_config_dir: config.join(APP_IDENTIFIER),
            app_data_dir: data.join(APP_IDENTIFIER),
            app_local_data_dir: local_data.join(APP_IDENTIFIER),
            app_cache_dir: cache.join(APP_IDENTIFIER),
            app_log_dir: log,
        })
    }

    /// Apply systemd's `*_DIRECTORY` env vars (`ConfigurationDirectory=`,
    /// `StateDirectory=`, ...) when present. Each may hold a colon-separated
    /// list; the first entry wins. Empty values act as unset.
    fn with_systemd_overrides(mut self, lookup: impl Fn(&str) -> Option<String>) -> Self {
        let first = |var: &str| {
            lookup(var)
                .and_then(|v| v.split(':').next().map(str::to_owned))
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from)
        };
        if let Some(dir) = first("CONFIGURATION_DIRECTORY") {
            self.app_config_dir = dir;
        }
        if let Some(dir) = first("STATE_DIRECTORY") {
            self.app_data_dir.clone_from(&dir);
            self.app_local_data_dir = dir;
        }
        if let Some(dir) = first("CACHE_DIRECTORY") {
            self.app_cache_dir = dir;
        }
        if let Some(dir) = first("LOGS_DIRECTORY") {
            self.app_log_dir = dir;
        }
        self
    }

    /// Default frontend bundle location for the standalone full server:
    /// the FHS share dir for manual installs on Linux (the deb ships its
    /// own path in the unit), the cargo-leptos output elsewhere (dev).
    pub fn default_site_root() -> PathBuf {
        if cfg!(target_os = "linux") {
            PathBuf::from("/usr/local/share/tauri-leptos/site")
        } else {
            PathBuf::from("target/site")
        }
    }

    /// Create every directory that does not exist yet.
    pub fn ensure_dirs(&self) -> io::Result<()> {
        for dir in [
            &self.app_config_dir,
            &self.app_data_dir,
            &self.app_local_data_dir,
            &self.app_cache_dir,
            &self.app_log_dir,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_root_puts_everything_under_one_folder() {
        let paths = AppPaths::from_root("/tmp/app");
        assert_eq!(paths.app_config_dir, Path::new("/tmp/app/config"));
        assert_eq!(paths.app_data_dir, Path::new("/tmp/app/data"));
        assert_eq!(paths.app_local_data_dir, Path::new("/tmp/app/data"));
        assert_eq!(paths.app_cache_dir, Path::new("/tmp/app/cache"));
        assert_eq!(paths.app_log_dir, Path::new("/tmp/app/logs"));
    }

    #[test]
    fn systemd_overrides_replace_matching_dirs() {
        let paths = AppPaths::from_root("/base").with_systemd_overrides(|var| match var {
            "CONFIGURATION_DIRECTORY" => Some("/etc/app".into()),
            "STATE_DIRECTORY" => Some("/var/lib/app".into()),
            "LOGS_DIRECTORY" => Some("/var/log/app".into()),
            _ => None,
        });
        assert_eq!(paths.app_config_dir, Path::new("/etc/app"));
        assert_eq!(paths.app_data_dir, Path::new("/var/lib/app"));
        assert_eq!(paths.app_local_data_dir, Path::new("/var/lib/app"));
        assert_eq!(paths.app_cache_dir, Path::new("/base/cache"));
        assert_eq!(paths.app_log_dir, Path::new("/var/log/app"));
    }

    #[test]
    fn systemd_override_takes_first_entry_and_ignores_empty() {
        let paths = AppPaths::from_root("/base").with_systemd_overrides(|var| match var {
            "STATE_DIRECTORY" => Some("/var/lib/a:/var/lib/b".into()),
            "CACHE_DIRECTORY" => Some(String::new()),
            _ => None,
        });
        assert_eq!(paths.app_data_dir, Path::new("/var/lib/a"));
        assert_eq!(paths.app_cache_dir, Path::new("/base/cache"));
    }

    #[test]
    fn explicit_app_dir_wins_over_everything() {
        let paths = AppPaths::resolve_standalone(Some(Path::new("/tmp/pinned")))
            .expect("from_root cannot fail");
        assert_eq!(paths, AppPaths::from_root("/tmp/pinned"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_log_dir_lands_in_library_logs() {
        let paths = AppPaths::platform_defaults().expect("home dirs exist");
        let home = dirs::home_dir().expect("home dir exists");
        assert_eq!(
            paths.app_log_dir,
            home.join("Library/Logs").join(APP_IDENTIFIER)
        );
    }
}
