//! SSR-side router: leptos routes + hydration asset serving, merged with
//! the core API surface (/api, /ws).

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};

/// The JS/wasm bundle name, fixed at build time by the cargo-leptos
/// metadata (`name` in `[[workspace.metadata.leptos]]`).
pub const OUTPUT_NAME: &str = "tauri-leptos";

/// Leptos options assembled from our own configuration — the runtime never
/// depends on cargo-leptos environment variables.
pub fn leptos_options(site_root: &Path, addr: SocketAddr) -> LeptosOptions {
    LeptosOptions::builder()
        .output_name(OUTPUT_NAME)
        .site_root(site_root.to_string_lossy().into_owned())
        .site_addr(addr)
        .build()
}

/// SSR routes + hydration assets only — the dev server mounts this and
/// proxies the API elsewhere.
pub fn leptos_router(options: LeptosOptions) -> Router {
    let routes = generate_route_list(App);
    Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(options)
}

/// The complete application router (production): SSR + the core API.
pub fn router(options: LeptosOptions) -> Router {
    leptos_router(options).merge(tauri_leptos_core::server::api_router())
}
