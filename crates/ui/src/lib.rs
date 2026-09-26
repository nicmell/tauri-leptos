pub mod api;
pub mod app;
mod demo;
#[cfg(feature = "ssr")]
pub mod server;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
    // Signal for tooling that the page is interactive.
    if let Some(body) = leptos::prelude::document().body() {
        let _ = body.set_attribute("data-hydrated", "true");
    }
}
