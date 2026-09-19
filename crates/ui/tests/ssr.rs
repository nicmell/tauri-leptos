#![cfg(feature = "ssr")]

use std::net::SocketAddr;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

fn test_router() -> axum::Router {
    let options = tauri_leptos_ui::server::leptos_options(
        Path::new("target/site"),
        SocketAddr::from(([127, 0, 0, 1], 0)),
    );
    tauri_leptos_ui::server::router(options)
}

#[tokio::test]
async fn home_page_is_server_rendered() {
    let response = test_router()
        .oneshot(
            Request::get("/")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let html = String::from_utf8(body.to_vec()).expect("utf8 html");
    // Rendered on the server, not by the client bundle.
    assert!(html.contains("<h1>Welcome to Tauri + Leptos</h1>"));
    // Hydration assets wired with the cargo-leptos names (no _bg suffix).
    assert!(html.contains("/pkg/tauri-leptos.js"));
    assert!(html.contains("/pkg/tauri-leptos.wasm"));
    assert!(!html.contains("_bg.wasm"));
}

#[tokio::test]
async fn api_is_merged_into_the_ssr_router() {
    let response = test_router()
        .oneshot(
            Request::get("/api/hello?name=ssr")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    assert!(String::from_utf8_lossy(&body).contains("Hello, ssr!"));
}
