use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn hello_greets_by_name() {
    let response = tauri_leptos_core::server::router()
        .oneshot(
            Request::get("/api/hello?name=Nick")
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
        "Hello, Nick! You've been greeted from Rust!"
    );
}

#[tokio::test]
async fn hello_defaults_to_world() {
    let response = tauri_leptos_core::server::router()
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
