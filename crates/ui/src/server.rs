//! SSR-side routing (feature `ssr`): leptos pages + hydration assets,
//! optionally merged with the core API for the single-origin production
//! server.

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};

/// The JS/wasm bundle name, fixed at build time by the cargo-leptos
/// metadata (`name` in `[[workspace.metadata.leptos]]`).
pub const OUTPUT_NAME: &str = "tauri-leptos";

/// Leptos options assembled from our own values — the runtime never
/// depends on cargo-leptos environment variables.
pub fn leptos_options(site_root: &Path, addr: SocketAddr) -> LeptosOptions {
    LeptosOptions::builder()
        .output_name(OUTPUT_NAME)
        .site_root(site_root.to_string_lossy().into_owned())
        .site_addr(addr)
        .build()
}

/// SSR routes + hydration assets. `api_base` = the API origin injected
/// into the page (dev frontend-server); `None` = same origin (prod).
pub fn leptos_router(options: LeptosOptions, api_base: Option<String>) -> Router {
    let routes = generate_route_list(App);
    Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            let api_base = api_base.clone();
            move || shell(options.clone(), api_base.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(move |options| {
            shell(options, api_base.clone())
        }))
        .with_state(options)
}

/// The complete production router: SSR + the core API on one origin,
/// so the client uses relative URLs.
pub fn router(options: LeptosOptions) -> Router {
    leptos_router(options, None).merge(tauri_leptos_core::server::api_router())
}
