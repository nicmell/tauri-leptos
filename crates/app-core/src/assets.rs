//! Site-asset backends: one interface, one implementation per way the
//! frontend bundle can be reached. Each backend turns into an axum
//! [`Router`] mounted as the server's fallback; requests that are not
//! assets flow into `on_miss` (the SSR shell renderer in full builds).

use std::path::PathBuf;

use axum::Router;

/// A way to serve the site bundle. `on_miss` handles requests that are
/// not assets (typically: render the SSR shell so the client router
/// can take over).
pub trait Assets {
    fn into_router(self, on_miss: Router) -> Router;
}

/// The static site bundle, wherever it lives.
///
/// - [`StaticAssets::from_site_root`]: a plain directory, served by
///   `tower_http::ServeDir` (its own traversal guard, `ETag` and range
///   support) — cli builds, the watch server, tests.
/// - [`StaticAssets::from_tauri_fs`] (feature `tauri`): the Tauri
///   resource store via the fs plugin's Rust API — one code path for
///   the desktop bundle (real files) and the Android APK (assets as
///   file descriptors), the only backend that can serve straight from
///   an APK.
pub struct StaticAssets(Backend);

enum Backend {
    Dir(PathBuf),
    #[cfg(feature = "tauri")]
    TauriFs(tauri_fs::Opener),
}

impl StaticAssets {
    pub fn from_site_root(dir: impl Into<PathBuf>) -> Self {
        Self(Backend::Dir(dir.into()))
    }

    #[cfg(feature = "tauri")]
    pub fn from_tauri_fs<R: tauri::Runtime>(app: tauri::AppHandle<R>, base: PathBuf) -> Self {
        Self(Backend::TauriFs(tauri_fs::opener(app, base)))
    }
}

impl Assets for StaticAssets {
    fn into_router(self, on_miss: Router) -> Router {
        match self.0 {
            Backend::Dir(dir) => {
                let serve = tower_http::services::ServeDir::new(dir)
                    .append_index_html_on_directories(false)
                    .fallback(on_miss);
                Router::new().fallback_service(serve)
            }
            #[cfg(feature = "tauri")]
            Backend::TauriFs(opener) => tauri_fs::router(opener, on_miss),
        }
    }
}

/// Dev backend: reverse-proxies every non-api request to the
/// `cargo leptos watch` server, which renders the (hot-reload
/// instrumented) pages itself — `on_miss` never applies. Web sockets
/// pass through untouched.
#[cfg(feature = "dev")]
pub struct ProxyAssets(pub String);

#[cfg(feature = "dev")]
impl Assets for ProxyAssets {
    fn into_router(self, _on_miss: Router) -> Router {
        axum_reverse_proxy::ReverseProxy::new("/", &self.0).into()
    }
}

#[cfg(feature = "tauri")]
mod tauri_fs {
    use std::io;
    use std::path::PathBuf;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::response::{IntoResponse, Response};

    /// Opens a site asset by page-relative path, yielding a real
    /// `std::fs::File` on every platform (the runtime generics die
    /// here, in the closure).
    pub(super) type Opener = Arc<dyn Fn(&str) -> io::Result<std::fs::File> + Send + Sync>;

    pub(super) fn opener<R: tauri::Runtime>(app: tauri::AppHandle<R>, base: PathBuf) -> Opener {
        use tauri_plugin_fs::FsExt;
        Arc::new(move |rel: &str| {
            let path = std::path::Path::new(rel);
            // Only plain relative components: no traversal, no
            // absolute paths.
            if path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid asset path",
                ));
            }
            let mut open_options = tauri_plugin_fs::OpenOptions::new();
            open_options.read(true);
            app.fs()
                .open(base.join(path), open_options)
                .map_err(io::Error::other)
        })
    }

    fn serve_file(rel: &str, file: std::fs::File) -> Response {
        let mime = mime_guess::from_path(rel).first_or_octet_stream();
        let stream = tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file));
        (
            [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
            Body::from_stream(stream),
        )
            .into_response()
    }

    pub(super) fn router(opener: Opener, on_miss: Router) -> Router {
        Router::new().fallback(move |req: Request| {
            let opener = opener.clone();
            let on_miss = on_miss.clone();
            async move {
                use tower::util::ServiceExt;
                let rel = req.uri().path().trim_start_matches('/').to_owned();
                match opener(&rel) {
                    Ok(file) => serve_file(&rel, file),
                    Err(_) => on_miss.oneshot(req).await.unwrap_or_else(|e| match e {}),
                }
            }
        })
    }
}
