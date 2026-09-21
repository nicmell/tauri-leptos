//! SSR-side routing (feature `ssr`): leptos pages + hydration assets,
//! merged with the core API into the single-origin server used
//! everywhere (dev watch, cli, tauri shell).
//!
//! Asset serving goes through [`SiteAssets`], one interface for every
//! run mode: a plain directory (cli, dev frontend-server, tests) or the
//! Tauri resource store (desktop bundle and Android APK, via the fs
//! plugin) — the shells provide their own implementation.

use std::io;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use leptos::prelude::LeptosOptions;
use leptos_axum::{LeptosRoutes, generate_route_list};

use crate::app::{App, shell};

/// The JS/wasm bundle name, fixed at build time by the cargo-leptos
/// metadata (`name` in `[[workspace.metadata.leptos]]`).
pub const OUTPUT_NAME: &str = "tauri-leptos";

/// Resolves site assets by page-relative path ("pkg/tauri-leptos.wasm").
pub trait SiteAssets: Send + Sync + 'static {
    fn open(&self, rel: &str) -> io::Result<std::fs::File>;
}

/// Assets from a plain directory (cli `--site-root`, the dev
/// frontend-server's `target/site`, tests).
pub struct DirAssets(pub PathBuf);

impl SiteAssets for DirAssets {
    fn open(&self, rel: &str) -> io::Result<std::fs::File> {
        let path = Path::new(rel);
        // Only plain relative components: no traversal, no absolute paths.
        if path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid asset path",
            ));
        }
        std::fs::File::open(self.0.join(path))
    }
}

/// Leptos options assembled from our own values — the runtime never
/// depends on cargo-leptos environment variables. (`site_root` is not
/// needed: asset serving goes through [`SiteAssets`].)
pub fn leptos_options(addr: SocketAddr) -> LeptosOptions {
    LeptosOptions::builder()
        .output_name(OUTPUT_NAME)
        .site_addr(addr)
        // Must match `reload-port` in the cargo-leptos metadata: the
        // AutoReload script connects to it (dev builds only).
        .reload_port(3002)
        .build()
}

/// SSR routes + asset serving.
pub fn leptos_router(options: LeptosOptions, assets: Arc<dyn SiteAssets>) -> Router {
    let routes = generate_route_list(App);
    // Non-asset misses render the app shell (its router shows the
    // fallback route), same behavior as leptos's own file handler.
    let render_shell = leptos_axum::render_app_to_stream({
        let options = options.clone();
        move || shell(options.clone())
    });

    Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(move |req: Request| {
            let assets = assets.clone();
            let render_shell = render_shell.clone();
            async move {
                let rel = req.uri().path().trim_start_matches('/').to_owned();
                match assets.open(&rel) {
                    Ok(file) => serve_file(&rel, file),
                    Err(_) => render_shell(req).await.into_response(),
                }
            }
        })
        .with_state(options)
}

fn serve_file(rel: &str, file: std::fs::File) -> Response {
    let mime = mime_guess::from_path(rel).first_or_octet_stream();
    let stream = tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file));
    (
        [(header::CONTENT_TYPE, mime.as_ref())],
        Body::from_stream(stream),
    )
        .into_response()
}

/// The complete router: SSR + the core API on one origin, everywhere —
/// the client always uses relative URLs.
pub fn router(options: LeptosOptions, assets: Arc<dyn SiteAssets>) -> Router {
    leptos_router(options, assets).merge(tauri_leptos_core::server::api_router())
}
