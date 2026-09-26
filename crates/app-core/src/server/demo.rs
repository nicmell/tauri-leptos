//! The demo API: a JSON endpoint, an in-memory counter and a WebSocket
//! echo — the server half of what the demo page exercises. Delete this
//! file (and the `demo::routes` call in [`super::api_router`]) to start
//! from an empty API; `crates/ui/src/demo.rs` is its frontend twin.

use axum::extract::Query;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// `cors_origins` reaches the ws guard, which the CORS layer of
/// [`super::api_router`] does not cover.
pub(super) fn routes(cors_origins: Vec<String>) -> Router {
    Router::new()
        .route("/api/hello", get(hello))
        // POST: the demo counter mutates state.
        .route("/api/counter", axum::routing::post(counter))
        .route(
            "/ws",
            get(ws_upgrade).route_layer(axum::middleware::from_fn(
                move |req: axum::extract::Request, next: axum::middleware::Next| {
                    let allowed = cors_origins.clone();
                    async move {
                        if super::origin_allowed(req.headers(), &allowed) {
                            next.run(req).await
                        } else {
                            axum::http::StatusCode::FORBIDDEN.into_response()
                        }
                    }
                },
            )),
        )
}

#[derive(Deserialize)]
struct HelloParams {
    name: Option<String>,
}

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

async fn hello(Query(params): Query<HelloParams>) -> Json<HelloResponse> {
    let name = params.name.unwrap_or_else(|| "world".to_owned());
    Json(HelloResponse {
        message: format!("Hello, {name}! You've been greeted from Rust!"),
    })
}

/// In-memory state marker: proves across dev rebuilds that the API server
/// process was not restarted (the count survives UI changes).
async fn counter() -> Json<serde_json::Value> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let value = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    Json(serde_json::json!({ "count": value }))
}

async fn ws_upgrade(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(echo)
}

async fn echo(mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(_) | Message::Binary(_) => {
                if socket.send(message).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
