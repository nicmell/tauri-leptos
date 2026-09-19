pub mod logging;
pub mod paths;
pub mod server;

/// Bundle identifier, kept in sync with `src-tauri/tauri.conf.json`.
pub const APP_IDENTIFIER: &str = "com.nick.tauri-leptos";

/// Env var that pins every app directory under a single folder.
/// An empty value must act as unset.
pub const APP_DIR_ENV: &str = "TAURI_LEPTOS_APP_DIR";
