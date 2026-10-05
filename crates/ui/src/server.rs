//! The SSR side (feature `ssr`): the app's pages, server functions and pipe as
//! an axum router. Each host adds its own fallback for the site files.

use axum::Router;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};
use crate::config::AppConfig;

/// The app's pages, server functions and pipe, with `config` as context.
pub fn router(options: LeptosOptions, config: AppConfig) -> Router {
    let routes = generate_route_list(App);
    let pipe = tauri_leptos_core::server::router(&options.output_name, &options.site_pkg_dir);
    Router::new()
        .leptos_routes_with_context(&options, routes, move || provide_context(config.clone()), {
            let options = options.clone();
            move || shell(options.clone())
        })
        .with_state(options)
        .merge(pipe)
}
