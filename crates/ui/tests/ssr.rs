#![cfg(feature = "ssr")]

use std::net::SocketAddr;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tauri_leptos_core::assets::StaticAssets;
use tauri_leptos_core::config::AppConfig;
use tower::ServiceExt;

fn options() -> leptos::prelude::LeptosOptions {
    tauri_leptos_ui::server::leptos_options(SocketAddr::from(([127, 0, 0, 1], 0)))
}

fn site_assets() -> StaticAssets {
    StaticAssets::from_site_root("target/site")
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
    let app = tauri_leptos_ui::server::router(options(), site_assets(), &AppConfig::default());

    let (status, _, html) = get(app.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);
    // Rendered on the server, not by the client bundle.
    assert!(html.contains("<h1>Welcome to Tauri + Leptos</h1>"));
    // Hydration assets wired with the cargo-leptos names (no _bg suffix).
    assert!(html.contains("/pkg/tauri-leptos.js"));
    assert!(html.contains("/pkg/tauri-leptos.wasm"));
    assert!(!html.contains("_bg.wasm"));
    // No api_base configured -> empty meta, the client stays relative.
    assert!(html.contains(r#"<meta name="api-base" content="">"#));

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
    let app = tauri_leptos_ui::server::router(
        options(),
        StaticAssets::from_site_root("../../target/site"),
        &AppConfig::default(),
    );
    let (status, content_type, body) = get(app, "/pkg/tauri-leptos.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("text/javascript"));
    assert!(!body.is_empty());
}

#[tokio::test]
async fn unknown_paths_render_the_shell() {
    let app = tauri_leptos_ui::server::router(options(), site_assets(), &AppConfig::default());
    let (_, _, html) = get(app, "/no-such-page").await;
    assert!(html.contains("not found"));
}

#[tokio::test]
async fn configured_api_base_is_injected() {
    let config = AppConfig {
        api_base: Some("http://pi.local:3000".to_owned()),
        ..AppConfig::default()
    };
    let app = tauri_leptos_ui::server::router(options(), site_assets(), &config);
    let (status, _, html) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"<meta name="api-base" content="http://pi.local:3000">"#));
}

// ServeDir carries its own traversal guard; escaping the root must
// fall through to the shell, never leak a file.
#[tokio::test]
async fn traversal_never_leaks_files() {
    for path in ["/..%2fCargo.toml", "/../Cargo.toml", "/%2e%2e/Cargo.toml"] {
        let app = tauri_leptos_ui::server::router(options(), site_assets(), &AppConfig::default());
        let (_, _, body) = get(app, path).await;
        assert!(!body.contains("[package]"), "{path} leaked a file");
    }
}
