//! The websocket of the page's worker: the server half (feature `ssr`).

#[cfg(feature = "ssr")]
pub use server::{OriginPolicy, router, serve};

#[cfg(feature = "ssr")]
mod server {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::Router;
    use axum::extract::{State, WebSocketUpgrade};
    use axum::http::{HeaderMap, StatusCode, header};
    use axum::response::{IntoResponse, Response};
    use axum::routing::get;

    /// The pages that may open the websocket, by their `Origin` header.
    #[derive(Clone, Debug)]
    pub enum OriginPolicy {
        /// Pages of the server's own origin: `Origin` names the `Host` the
        /// request went to.
        SameOrigin,
        /// Pages of these origins only.
        Only(Vec<String>),
    }

    impl OriginPolicy {
        fn allows(&self, origin: &str, host: Option<&str>) -> bool {
            match self {
                Self::SameOrigin => host.is_some_and(|host| {
                    origin
                        .strip_prefix("http://")
                        .or_else(|| origin.strip_prefix("https://"))
                        == Some(host)
                }),
                Self::Only(origins) => origins.iter().any(|allowed| allowed == origin),
            }
        }
    }

    /// `GET /ws`: the websocket, for the pages that `policy` allows.
    pub fn router(policy: OriginPolicy) -> Router {
        Router::new()
            .route("/ws", get(upgrade))
            .with_state(Arc::new(policy))
    }

    /// Serves [`router`] on `addr`.
    pub async fn serve(addr: SocketAddr, policy: OriginPolicy) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        log::info!("websocket on ws://{}/ws", listener.local_addr()?);
        axum::serve(listener, router(policy)).await
    }

    async fn upgrade(
        ws: WebSocketUpgrade,
        State(policy): State<Arc<OriginPolicy>>,
        headers: HeaderMap,
    ) -> Response {
        let value_of = |name| headers.get(name).and_then(|value| value.to_str().ok());
        let origin = value_of(header::ORIGIN).unwrap_or_default();
        if !policy.allows(origin, value_of(header::HOST)) {
            log::warn!("websocket refused for origin {origin:?}");
            return StatusCode::FORBIDDEN.into_response();
        }
        log::info!("websocket for origin {origin:?}");
        ws.on_upgrade(crate::demo::session)
    }
}
