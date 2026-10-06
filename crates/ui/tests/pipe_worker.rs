//! The pipe's worker script, a site file, loads the app's wasm.

use std::path::Path;

use tauri_leptos_core::page::WORKER_SCRIPT;

#[test]
fn the_worker_script_loads_the_apps_wasm() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace: toml::Table = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("read the workspace manifest")
        .parse()
        .expect("parse the workspace manifest");
    let leptos = &workspace["workspace"]["metadata"]["leptos"][0];
    let leptos = |key: &str| leptos[key].as_str().expect(key).to_owned();

    let path = root
        .join(leptos("assets-dir"))
        .join(WORKER_SCRIPT.trim_start_matches('/'));
    let script = std::fs::read_to_string(&path).expect("read the worker script");
    let pkg = format!("/{}/{}", leptos("site-pkg-dir"), env!("LEPTOS_OUTPUT_NAME"));
    assert!(script.contains(&format!("'{pkg}.js'")), "{script}");
    assert!(script.contains(&format!("'{pkg}.wasm'")), "{script}");
}
