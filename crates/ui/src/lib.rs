pub mod app;
pub mod config;
mod demo;
#[cfg(feature = "ssr")]
pub mod server;
pub mod socket;
mod tauri_ipc;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
}

/// The web worker's entry point, which `public/worker.js` calls.
// Here and not in the worker crate: rustc does not link an export of a crate
// that nothing references.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn worker_main() {
    tauri_leptos_worker::run();
}
