use axum::body::Body;
use axum::http::{Request, StatusCode};
use leptos::prelude::LeptosOptions;
use tauri_leptos_ui::config::AppConfig;
use tauri_leptos_ui::server::router;
use tower::ServiceExt;

async fn get(path: &str) -> (StatusCode, String) {
    let options = LeptosOptions::builder()
        .output_name(env!("LEPTOS_OUTPUT_NAME"))
        .build();
    let request = Request::get(path)
        .body(Body::empty())
        .expect("valid request");
    let Ok(response) = router(options, AppConfig::default()).oneshot(request).await;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (
        status,
        String::from_utf8(body.to_vec()).expect("utf-8 body"),
    )
}

#[tokio::test]
async fn home_renders_on_the_server() {
    let (status, html) = get("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("<h1>Welcome to Tauri + Leptos</h1>"),
        "{html}"
    );
    assert!(html.contains("/pkg/tauri-leptos.js"), "{html}");
    assert!(html.contains("/pkg/tauri-leptos.wasm"), "{html}");
    assert!(!html.contains("_bg.wasm"), "{html}");
}

/// The cli and the Tauri plugin add their own fallbacks.
#[tokio::test]
async fn unknown_paths_are_a_plain_404() {
    let (status, _) = get("/no-such-page").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
