//! A [`tower_http::services::fs::Backend`] over the Tauri resource
//! store: the fs plugin's Rust API opens plain files on desktop and
//! APK assets (as file descriptors) on Android, always yielding a real
//! `std::fs::File`. `ServeDir::with_backend` does the rest — path
//! decoding, traversal guard, mime, `ETag`/ranges — identically to the
//! default Tokio backend the standalone server uses.
#![cfg(feature = "tauri")]

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
