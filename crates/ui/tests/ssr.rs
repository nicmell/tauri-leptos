#![cfg(feature = "ssr")]

use std::net::SocketAddr;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn options() -> leptos::prelude::LeptosOptions {
    tauri_leptos_ui::server::leptos_options(
        Path::new("target/site"),
        SocketAddr::from(([127, 0, 0, 1], 0)),
    )
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::get(uri)
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn production_router_renders_and_merges_the_api() {
    let app = tauri_leptos_ui::server::router(options());

    let (status, html) = get(app.clone(), "/").await;
    assert_eq!(status, StatusCode::OK);
    // Rendered on the server, not by the client bundle.
    assert!(html.contains("<h1>Welcome to Tauri + Leptos</h1>"));
    // Hydration assets wired with the cargo-leptos names (no _bg suffix).
    assert!(html.contains("/pkg/tauri-leptos.js"));
    assert!(html.contains("/pkg/tauri-leptos.wasm"));
    assert!(!html.contains("_bg.wasm"));
    // Single origin: no API base injected -> the client uses relative URLs.
    assert!(html.contains(r#"<meta name="api-base" content="">"#));

    let (status, body) = get(app, "/api/hello?name=ssr").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Hello, ssr!"));
}

#[tokio::test]
async fn dev_router_injects_the_api_base() {
    let app =
        tauri_leptos_ui::server::leptos_router(options(), Some("http://127.0.0.1:3001".to_owned()));
    let (status, html) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"<meta name="api-base" content="http://127.0.0.1:3001">"#));
}
