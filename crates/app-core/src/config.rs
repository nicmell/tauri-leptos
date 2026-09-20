//! `config.toml` in the app config dir. Loading happens before the logger
//! (the config decides logging), so problems are reported on stderr.

use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;

pub const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AppConfig {
    /// Address of the API/WS server. The frontend URL is never
    /// configurable (fixed 127.0.0.1:3000).
    pub api_addr: SocketAddr,
    /// Also write logs to a daily-rolling file in the app log dir.
    pub log_to_file: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_addr: SocketAddr::from(([127, 0, 0, 1], 3001)),
            log_to_file: false,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, toml::de::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Self::Parse(path, e) => write!(f, "{}: {e}", path.display()),
        }
    }
}

impl std::error::Error for ConfigError {}

impl AppConfig {
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config serializes")
    }

    /// Load from `<app_config_dir>/config.toml`. A missing file is seeded
    /// with the defaults (best effort); an unreadable or invalid file is
    /// an error — serve mode must fail fast so systemd sees it.
    pub fn load(paths: &AppPaths) -> Result<Self, ConfigError> {
        let path = paths.app_config_dir.join(CONFIG_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).map_err(|e| ConfigError::Parse(path, e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let config = Self::default();
                if let Err(e) = config.seed(&path) {
                    eprintln!("[config] could not write default {}: {e}", path.display());
                } else {
                    eprintln!("[config] wrote default config to {}", path.display());
                }
                Ok(config)
            }
            Err(e) => Err(ConfigError::Io(path, e)),
        }
    }

    /// Write this config to `path`; creates parent directories.
    pub fn seed(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_toml())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_roundtrip_through_toml() {
        let config = AppConfig::default();
        let parsed = AppConfig::parse(&config.to_toml()).expect("roundtrip parses");
        assert_eq!(parsed, config);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let parsed = AppConfig::parse("api_addr = \"0.0.0.0:8080\"\n").expect("partial parses");
        assert_eq!(parsed.api_addr, SocketAddr::from(([0, 0, 0, 0], 8080)));
        assert!(!parsed.log_to_file);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(AppConfig::parse("nonsense = true\n").is_err());
    }

    #[test]
    fn load_seeds_missing_file() {
        let dir = std::env::temp_dir().join(format!("tl-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = AppPaths::from_root(&dir);
        let config = AppConfig::load(&paths).expect("seeds and returns defaults");
        assert_eq!(config, AppConfig::default());
        assert!(paths.app_config_dir.join(CONFIG_FILE).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
