use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn hello_greets_by_name() {
    let response = tauri_leptos_core::server::api_router(&[])
        .oneshot(
            Request::get("/api/hello?name=Ada")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(
        json["message"],
        "Hello, Ada! You've been greeted from Rust!"
    );
}

#[tokio::test]
async fn hello_defaults_to_world() {
    let response = tauri_leptos_core::server::api_router(&[])
        .oneshot(
            Request::get("/api/hello")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("infallible");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(
        json["message"],
        "Hello, world! You've been greeted from Rust!"
    );
}

#[tokio::test]
async fn ws_rejects_foreign_origin() {
    let request = Request::get("/ws")
        .header("origin", "http://evil.example")
        .header("host", "127.0.0.1:3000")
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(Body::empty())
        .expect("valid request");
    let response = tauri_leptos_core::server::api_router(&[])
        .oneshot(request)
        .await
        .expect("infallible");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ws_allows_same_origin_and_configured_origins() {
    for (origin, allowed) in [
        ("http://127.0.0.1:3000", vec![]),
        ("http://app.example", vec!["http://app.example".to_owned()]),
        ("http://anywhere.example", vec!["*".to_owned()]),
    ] {
        let request = Request::get("/ws")
            .header("origin", origin)
            .header("host", "127.0.0.1:3000")
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .expect("valid request");
        let response = tauri_leptos_core::server::api_router(&allowed)
            .oneshot(request)
            .await
            .expect("infallible");
        assert_ne!(
            response.status(),
            StatusCode::FORBIDDEN,
            "origin {origin} should be allowed"
        );
    }
}
