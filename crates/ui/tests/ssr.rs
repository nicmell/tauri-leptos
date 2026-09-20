#![cfg(feature = "ssr")]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tauri_leptos_ui::server::{DirAssets, SiteAssets};
use tower::ServiceExt;

fn options() -> leptos::prelude::LeptosOptions {
    tauri_leptos_ui::server::leptos_options(SocketAddr::from(([127, 0, 0, 1], 0)))
}

fn site_assets() -> Arc<dyn SiteAssets> {
    Arc::new(DirAssets("target/site".into()))
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
    // Single origin: no API base injected -> the client uses relative URLs.
    assert!(html.contains(r#"<meta name="api-base" content="">"#));

    let (status, _, body) = get(app, "/api/hello?name=ssr").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Hello, ssr!"));
}

#[tokio::test]
async fn dev_router_injects_the_api_base() {
    let app = tauri_leptos_ui::server::leptos_router(
        options(),
        Some("http://127.0.0.1:3001".to_owned()),
        site_assets(),
    );
    let (status, _, html) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"<meta name="api-base" content="http://127.0.0.1:3001">"#));
}

// Needs a built site (cargo leptos build); CI builds it before testing.
#[tokio::test]
async fn assets_are_served_with_their_mime_type() {
    if !std::path::Path::new("../../target/site/pkg").exists() {
        eprintln!("skipping: target/site not built");
        return;
    }
    let app =
        tauri_leptos_ui::server::router(options(), Arc::new(DirAssets("../../target/site".into())));
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

#[test]
fn dir_assets_reject_traversal() {
    let assets = DirAssets("target/site".into());
    assert!(assets.open("../Cargo.toml").is_err());
    assert!(assets.open("/etc/hosts").is_err());
}
