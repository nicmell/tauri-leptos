//! The SSR side (feature `ssr`): the app's pages and server functions as an
//! axum router. Each host adds its own fallback for the site files.

use axum::Router;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};
use crate::config::AppConfig;

/// The app's pages and server functions, with `config` as context.
pub fn router(options: LeptosOptions, config: AppConfig) -> Router {
    let routes = generate_route_list(App);
    Router::new()
        .leptos_routes_with_context(&options, routes, move || provide_context(config.clone()), {
            let options = options.clone();
            move || shell(options.clone())
        })
        .with_state(options)
}
