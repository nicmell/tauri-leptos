#![cfg(feature = "ssr")]

use std::net::SocketAddr;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tauri_leptos_core::assets::DirAssets;
use tower::ServiceExt;

fn options() -> leptos::prelude::LeptosOptions {
    tauri_leptos_ui::server::leptos_options(SocketAddr::from(([127, 0, 0, 1], 0)))
}

fn site_assets() -> DirAssets {
    DirAssets("target/site".into())
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, Option<String>, String) {
    let response = app
        .oneshot(
            Request::get(uri)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible");
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or_default().to_owned());
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (
        status,
        content_type,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

#[tokio::test]
async fn production_router_renders_and_merges_the_api() {
    let app = tauri_leptos_ui::server::router(options(), site_assets());

    let (status, _, html) = get(app.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);
    // Rendered on the server, not by the client bundle.
    assert!(html.contains("<h1>Welcome to Tauri + Leptos</h1>"));
    // Hydration assets wired with the cargo-leptos names (no _bg suffix).
    assert!(html.contains("/pkg/tauri-leptos.js"));
    assert!(html.contains("/pkg/tauri-leptos.wasm"));
    assert!(!html.contains("_bg.wasm"));

    let (status, _, body) = get(app, "/api/hello?name=ssr").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Hello, ssr!"));
}

// Needs a built site (cargo leptos build); CI builds it before testing.
#[tokio::test]
async fn assets_are_served_with_their_mime_type() {
    if !std::path::Path::new("../../target/site/pkg").exists() {
        eprintln!("skipping: target/site not built");
        return;
    }
    let app = tauri_leptos_ui::server::router(options(), DirAssets("../../target/site".into()));
    let (status, content_type, body) = get(app, "/pkg/tauri-leptos.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("text/javascript"));
    assert!(!body.is_empty());
}

#[tokio::test]
async fn unknown_paths_render_the_shell() {
    let app = tauri_leptos_ui::server::router(options(), site_assets());
    let (_, _, html) = get(app, "/no-such-page").await;
    assert!(html.contains("not found"));
}

// ServeDir carries its own traversal guard; escaping the root must
// fall through to the shell, never leak a file.
#[tokio::test]
async fn traversal_never_leaks_files() {
    for path in ["/..%2fCargo.toml", "/../Cargo.toml", "/%2e%2e/Cargo.toml"] {
        let app = tauri_leptos_ui::server::router(options(), site_assets());
        let (_, _, body) = get(app, path).await;
        assert!(!body.contains("[package]"), "{path} leaked a file");
    }
}
