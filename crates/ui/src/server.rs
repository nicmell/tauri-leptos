//! SSR-side routing (feature `ssr`): the leptos pages and server
//! functions, self-contained — asset serving and the api live in
//! app-core, which mounts [`pages`] as the miss handler of its asset
//! layer. Unknown paths are a plain 404 (axum's default fallback).

use std::net::SocketAddr;

use axum::Router;
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};

/// The JS/wasm bundle name, fixed at build time by the cargo-leptos
/// metadata (`name` in `[[workspace.metadata.leptos]]`).
pub const OUTPUT_NAME: &str = "tauri-leptos";

/// Leptos options assembled from our own values — the runtime never
/// depends on cargo-leptos environment variables.
fn leptos_options(addr: SocketAddr) -> LeptosOptions {
    LeptosOptions::builder()
        .output_name(OUTPUT_NAME)
        .site_addr(addr)
        // Must match `reload-port` in the cargo-leptos metadata: the
        // AutoReload script connects to it (dev builds only).
        .reload_port(3002)
        .build()
}

/// The SSR pages + server functions. `api_base` = the origin the
/// client sends api/ws requests to, injected into the page (`None` =
/// same origin).
pub fn pages(addr: SocketAddr, api_base: Option<String>) -> Router {
    let options = leptos_options(addr);
    let routes = generate_route_list(App);
    Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone(), api_base.clone())
        })
        .with_state(options)
}
