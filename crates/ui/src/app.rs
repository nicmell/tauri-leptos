use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::demo::HomePage;

/// The SSR document shell; cargo-leptos injects the hydration assets.
///
/// `api_base` tells the page where the api lives: `None` = same origin
/// (empty meta). The meta is always rendered — a conditional view in
/// the head would not emit.
#[cfg(feature = "ssr")]
pub(crate) fn shell(options: LeptosOptions, api_base: Option<String>) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="api-base" content=api_base.unwrap_or_default() />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
                <link rel="stylesheet" href="/styles.css" />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text="Tauri + Leptos" />
        <Router>
            <Routes fallback=|| "not found">
                <Route path=path!("") view=HomePage />
            </Routes>
        </Router>
    }
}
