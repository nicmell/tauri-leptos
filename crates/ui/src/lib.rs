pub mod app;
pub mod config;
mod demo;
#[cfg(feature = "ssr")]
pub mod server;
mod tauri_ipc;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
}
