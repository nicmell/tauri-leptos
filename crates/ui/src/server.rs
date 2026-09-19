//! SSR-side router: leptos routes + hydration asset serving, merged with
//! the core API surface (/api, /ws).

use axum::Router;
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};

/// The complete application router for a given leptos configuration.
pub fn router(options: LeptosOptions) -> Router {
    let routes = generate_route_list(App);
    Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(options)
        .merge(tauri_leptos_core::server::api_router())
}
