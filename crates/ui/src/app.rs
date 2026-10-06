use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::demo::HomePage;

/// The SSR document around [`App`].
#[cfg(feature = "ssr")]
pub(crate) fn shell(options: LeptosOptions) -> impl IntoView {
    let stylesheet = format!("/pkg/{}.css", options.output_name);
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="icon" type="image/svg+xml" href="/tauri.svg" />
                <link rel="stylesheet" href=stylesheet />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
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
        <Title text="Tauri + Leptos SSR" />
        <Router>
            <main>
                <Routes fallback=|| "Page not found.">
                    <Route path=path!("") view=HomePage />
                </Routes>
            </main>
        </Router>
    }
}
