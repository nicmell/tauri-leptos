//! The facts `tauri.conf.json`, `AppConfig` and the cargo-leptos metadata
//! repeat.

use std::net::SocketAddr;
use std::path::Path;

use tauri_leptos_ui::config::AppConfig;

fn leptos_metadata() -> toml::Value {
    let workspace: toml::Table = std::fs::read_to_string("../Cargo.toml")
        .expect("read the workspace manifest")
        .parse()
        .expect("parse the workspace manifest");
    workspace["workspace"]["metadata"]["leptos"][0].clone()
}

#[test]
fn tauri_and_cargo_leptos_agree() {
    let leptos = leptos_metadata();
    let leptos = |key: &str| leptos[key].as_str().expect(key).to_owned();

    let tauri: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tauri.conf.json").expect("read tauri.conf.json"),
    )
    .expect("parse tauri.conf.json");
    let build = &tauri["build"];

    assert_eq!(build["devUrl"], format!("http://{}", leptos("site-addr")));
    assert_eq!(
        Path::new(build["frontendDist"].as_str().expect("frontendDist")),
        Path::new("..").join(leptos("site-root"))
    );
    assert_eq!(env!("LEPTOS_OUTPUT_NAME"), leptos("name"));
}

#[test]
fn the_websocket_port_is_its_own() {
    let leptos = leptos_metadata();
    let site: SocketAddr = leptos["site-addr"]
        .as_str()
        .expect("site-addr")
        .parse()
        .expect("site-addr is an address");
    let reload = leptos["reload-port"].as_integer().expect("reload-port");
    let ws_port = AppConfig::default().ws_addr.port();

    assert_ne!(ws_port, site.port());
    assert_ne!(i64::from(ws_port), reload);
}
