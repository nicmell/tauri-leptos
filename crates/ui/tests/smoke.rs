#![cfg(target_arch = "wasm32")]

use leptos::prelude::*;
use tauri_leptos_ui::app::App;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn app_mounts() {
    let handle = mount_to_body(|| view! { <App /> });
    let body = document().body().expect("body");
    assert!(body.inner_html().contains("Welcome to Tauri + Leptos"));
    drop(handle);
}
