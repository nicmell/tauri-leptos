//! `public/worker.js` loads the files that cargo-leptos builds.

#[test]
fn worker_js_names_the_cargo_leptos_output() {
    let workspace: toml::Table = include_str!("../../../Cargo.toml")
        .parse()
        .expect("parse the workspace manifest");
    let pkg = workspace["workspace"]["metadata"]["leptos"][0]["site-pkg-dir"]
        .as_str()
        .expect("site-pkg-dir");
    let name = env!("LEPTOS_OUTPUT_NAME");
    let script = include_str!("../public/worker.js");

    assert!(script.contains(&format!("'/{pkg}/{name}.js'")), "{script}");
    assert!(
        script.contains(&format!("'/{pkg}/{name}.wasm'")),
        "{script}"
    );
}
