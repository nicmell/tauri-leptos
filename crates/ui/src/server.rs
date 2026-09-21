//! SSR-side routing (feature `ssr`): leptos pages + hydration assets,
//! merged with the core API into the single-origin server used
//! everywhere (dev watch, cli, tauri shell).
//!
//! Asset serving goes through [`tauri_leptos_core::assets::Assets`]:
//! the caller picks the backend (directory, tauri resources, dev
//! proxy) and this router wires the SSR shell as its miss handler.

use std::net::SocketAddr;

use axum::Router;
use axum::extract::Request;
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};
use tauri_leptos_core::assets::Assets;

use crate::app::{App, shell};

/// The JS/wasm bundle name, fixed at build time by the cargo-leptos
/// metadata (`name` in `[[workspace.metadata.leptos]]`).
pub const OUTPUT_NAME: &str = "tauri-leptos";

/// Leptos options assembled from our own values — the runtime never
/// depends on cargo-leptos environment variables.
pub fn leptos_options(addr: SocketAddr) -> LeptosOptions {
    LeptosOptions::builder()
        .output_name(OUTPUT_NAME)
        .site_addr(addr)
        // Must match `reload-port` in the cargo-leptos metadata: the
        // AutoReload script connects to it (dev builds only).
        .reload_port(3002)
        .build()
}

/// SSR routes + asset serving through the given backend. `api_base` =
/// the origin the client sends api/ws requests to, injected into the
/// page (`None` = same origin).
pub fn leptos_router(
    options: LeptosOptions,
    api_base: Option<String>,
    assets: impl Assets,
) -> Router {
    let routes = generate_route_list(App);
    // Non-asset misses render the app shell (its router shows the
    // fallback route), same behavior as leptos's own file handler.
    let render_shell = leptos_axum::render_app_to_stream({
        let options = options.clone();
        let api_base = api_base.clone();
        move || shell(options.clone(), api_base.clone())
    });
    let on_miss = Router::new().fallback(move |req: Request| {
        let render_shell = render_shell.clone();
        async move { render_shell(req).await }
    });

    Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone(), api_base.clone())
        })
        .fallback_service(assets.into_router(on_miss))
        .with_state(options)
}

/// The complete `site`-build router: SSR + the core API on one origin,
/// wired from the app config (`api_base`, `cors_origins`).
pub fn router(
    options: LeptosOptions,
    assets: impl Assets,
    config: &tauri_leptos_core::config::AppConfig,
) -> Router {
    leptos_router(options, config.api_base.clone(), assets)
        .merge(tauri_leptos_core::server::api_router(&config.cors_origins))
}
