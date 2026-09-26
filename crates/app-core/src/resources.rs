//! Resource retrieval, one router per host: the static site served
//! from disk (standalone), from the Tauri resource store (the shell's
//! release runs), or reverse-proxied from the watch (the shell's dev
//! runs). [`crate::bootstrap::Ctx`] builds it once at bootstrap and
//! composes it into the one router.

use axum::Router;
use tower_http::services::ServeDir;

use crate::config::AppConfig;
use crate::paths::AppPaths;

/// The site from disk with the default Tokio backend. Relative
/// `site_root` resolves against the config dir; absent = the
/// cargo-leptos output (dev workspace runs). Pin the root:
/// symlinked/relative roots resolve once, here.
pub(crate) fn standalone(config: &AppConfig, paths: &AppPaths) -> Router {
    let site_root = config.site_root_resolved(&paths.app_config_dir, "target/site");
    let site_root = site_root.canonicalize().unwrap_or(site_root);
    if !site_root.join("pkg").exists() {
        tracing::warn!(
            site_root = %site_root.display(),
            "site root looks empty - run `cargo leptos build --release` \
             and install/point `site_root` in the config at it"
        );
    }
    tracing::info!(site_root = %site_root.display(), "serving the site from disk");
    // Miss = plain 404, and directories are never listed or
    // index-mapped — "/" belongs to the SSR pages in front.
    let dir = ServeDir::new(site_root).append_index_html_on_directories(false);
    Router::new().fallback_service(dir)
}

/// The site from the Tauri resource store via [`backend::TauriBackend`].
/// Relative `site_root` resolves against `resource_dir`; absent = the
/// bundled "site" map.
#[cfg(feature = "tauri")]
pub(crate) fn tauri(handle: tauri::AppHandle, config: &AppConfig) -> Result<Router, tauri::Error> {
    use tauri::Manager;

    let resource_dir = handle.path().resource_dir()?;
    let resource_site = config.site_root_resolved(&resource_dir, resource_dir.join("site"));
    tracing::info!(base = %resource_site.display(), "serving bundled resources");
    let dir = ServeDir::with_backend(resource_site, backend::TauriBackend::new(handle))
        .append_index_html_on_directories(false);
    Ok(Router::new().fallback_service(dir))
}

/// Everything reverse-proxied to the watch server.
#[cfg(feature = "tauri")]
pub(crate) fn proxy(upstream: &str) -> Router {
    Router::from(axum_reverse_proxy::ReverseProxy::new("/", upstream))
}

/// A [`tower_http::services::fs::Backend`] over the Tauri resource
/// store: the fs plugin's Rust API opens plain files on desktop and
/// APK assets (as file descriptors) on Android, always yielding a real
/// `std::fs::File`. `ServeDir::with_backend` does the rest — path
/// decoding, traversal guard, mime, `ETag`/ranges — identically to the
/// default Tokio backend the standalone server uses.
#[cfg(feature = "tauri")]
mod backend {
    use std::future::Future;
    use std::io;
    use std::path::PathBuf;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncSeek};
    use tower_http::services::fs::{Backend, File};

    #[derive(Clone)]
    pub(crate) struct TauriBackend {
        app: tauri::AppHandle,
    }

    impl TauriBackend {
        pub(crate) fn new(app: tauri::AppHandle) -> Self {
            Self { app }
        }

        /// The plugin call is synchronous (APK fds on Android): run it off
        /// the async workers and wrap the result for tokio.
        async fn open_std(&self, path: PathBuf) -> io::Result<std::fs::File> {
            let app = self.app.clone();
            tokio::task::spawn_blocking(move || {
                use tauri_plugin_fs::FsExt;
                let mut options = tauri_plugin_fs::OpenOptions::new();
                options.read(true);
                // The plugin returns io::Error directly — keep the kind
                // intact (ServeDir maps NotFound to 404).
                app.fs().open(path, options)
            })
            .await
            .map_err(io::Error::other)?
        }
    }

    impl Backend for TauriBackend {
        type File = TauriFile;
        type Metadata = std::fs::Metadata;
        type OpenFuture = Pin<Box<dyn Future<Output = io::Result<TauriFile>> + Send>>;
        type MetadataFuture = Pin<Box<dyn Future<Output = io::Result<std::fs::Metadata>> + Send>>;

        fn open(&self, path: PathBuf) -> Self::OpenFuture {
            let backend = self.clone();
            Box::pin(async move {
                let file = backend.open_std(path).await?;
                Ok(TauriFile(tokio::fs::File::from_std(file)))
            })
        }

        fn metadata(&self, path: PathBuf) -> Self::MetadataFuture {
            let backend = self.clone();
            Box::pin(async move { backend.open_std(path).await?.metadata() })
        }
    }

    pub(crate) struct TauriFile(tokio::fs::File);

    impl AsyncRead for TauriFile {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_read(cx, buf)
        }
    }

    impl AsyncSeek for TauriFile {
        fn start_seek(mut self: Pin<&mut Self>, position: io::SeekFrom) -> io::Result<()> {
            Pin::new(&mut self.0).start_seek(position)
        }

        fn poll_complete(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<u64>> {
            Pin::new(&mut self.0).poll_complete(cx)
        }
    }

    impl File for TauriFile {
        type Metadata = std::fs::Metadata;
        type MetadataFuture<'a> =
            Pin<Box<dyn Future<Output = io::Result<std::fs::Metadata>> + Send + 'a>>;

        fn metadata(&self) -> Self::MetadataFuture<'_> {
            Box::pin(async move { self.0.metadata().await })
        }
    }
}
