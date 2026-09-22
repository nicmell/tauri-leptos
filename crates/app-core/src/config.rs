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
    /// Address the server listens on (SSR + api + ws, one origin).
    /// `--host`/`--port` override it per invocation.
    pub listen: SocketAddr,
    /// Frontend bundle directory for `site` builds. `None` = the
    /// platform default (`AppPaths::default_site_root`).
    pub site_root: Option<PathBuf>,
    /// Origin the client sends api/ws requests to. `None` = same
    /// origin. Set it when the api lives on another host (e.g. the
    /// app on a device, the api on a Pi) — that host then needs the
    /// matching `cors_origins`.
    pub api_base: Option<String>,
    /// Origins allowed to call `/api` cross-origin (remote frontends).
    /// Empty = no CORS layer; `"*"` = any origin.
    pub cors_origins: Vec<String>,
    /// Also write logs to a daily-rolling file in the app log dir.
    pub log_to_file: bool,
    /// Dev settings (the tauri shell's dev runs).
    pub dev: DevConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DevConfig {
    /// The `cargo leptos watch` server the dev proxy forwards to. Must
    /// match `site-addr` in the leptos metadata and `devUrl` in
    /// tauri.conf.json.
    pub upstream: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 3000)),
            site_root: None,
            api_base: None,
            cors_origins: Vec::new(),
            log_to_file: false,
            dev: DevConfig::default(),
        }
    }
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            upstream: "http://127.0.0.1:3001".to_owned(),
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
        let config: Self = toml::from_str(text)?;
        config.validate().map_err(serde::de::Error::custom)?;
        Ok(config)
    }

    /// Semantic checks beyond TOML shape — fail at load time, not at
    /// router construction (the dev proxy panics on a malformed URI).
    fn validate(&self) -> Result<(), String> {
        let upstream = &self.dev.upstream;
        let uri: axum::http::Uri = upstream
            .parse()
            .map_err(|e| format!("dev.upstream `{upstream}`: {e}"))?;
        if uri.scheme_str() != Some("http") {
            return Err(format!(
                "dev.upstream `{upstream}`: must be an http:// URL \
                 (the dev proxy speaks plain http to the watch)"
            ));
        }
        Ok(())
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
                match config.seed(&path) {
                    Ok(()) => {
                        eprintln!("[config] wrote default config to {}", path.display());
                        Ok(config)
                    }
                    // Lost the race: another process seeded it first.
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                        let text = std::fs::read_to_string(&path)
                            .map_err(|e| ConfigError::Io(path.clone(), e))?;
                        Self::parse(&text).map_err(|e| ConfigError::Parse(path, e))
                    }
                    Err(e) => {
                        eprintln!("[config] could not write default {}: {e}", path.display());
                        Ok(config)
                    }
                }
            }
            Err(e) => Err(ConfigError::Io(path, e)),
        }
    }

    /// Resolve `site_root` by convention: absolute paths as-is,
    /// relative paths against `base` (the config file's directory for
    /// the standalone server, `resource_dir` for the tauri shell),
    /// absent -> `default`.
    pub fn site_root_resolved(&self, base: &Path, default: impl Into<PathBuf>) -> PathBuf {
        match &self.site_root {
            Some(p) if p.is_absolute() => p.clone(),
            Some(p) => base.join(p),
            None => default.into(),
        }
    }

    /// Write this config to `path` atomically — fails if the file
    /// already exists (no read-then-write race). Creates parent
    /// directories.
    pub fn seed(&self, path: &Path) -> io::Result<()> {
        use std::io::Write;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(self.to_toml().as_bytes())
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
        let parsed = AppConfig::parse("listen = \"0.0.0.0:8080\"\n").expect("partial parses");
        assert_eq!(parsed.listen, SocketAddr::from(([0, 0, 0, 0], 8080)));
        assert!(!parsed.log_to_file);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(AppConfig::parse("nonsense = true\n").is_err());
    }

    #[test]
    fn malformed_dev_upstream_is_rejected() {
        assert!(AppConfig::parse("[dev]\nupstream = \"not a url\"\n").is_err());
        assert!(AppConfig::parse("[dev]\nupstream = \"https://127.0.0.1:3001\"\n").is_err());
        assert!(AppConfig::parse("[dev]\nupstream = \"http://127.0.0.1:3001\"\n").is_ok());
    }

    #[test]
    fn site_root_convention() {
        let base = Path::new("/etc/app");
        let mut config = AppConfig::default();
        assert_eq!(
            config.site_root_resolved(base, "target/site"),
            PathBuf::from("target/site")
        );
        config.site_root = Some(PathBuf::from("site"));
        assert_eq!(
            config.site_root_resolved(base, "target/site"),
            PathBuf::from("/etc/app/site")
        );
        config.site_root = Some(PathBuf::from("/usr/share/app/site"));
        assert_eq!(
            config.site_root_resolved(base, "target/site"),
            PathBuf::from("/usr/share/app/site")
        );
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
