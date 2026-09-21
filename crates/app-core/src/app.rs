//! The app: one builder for every entrypoint. `app(ctx)` serves the
//! api-only router by default; `with_router` swaps in a custom one
//! (the leptos site router in `site` builds, the dev proxy in the
//! shell's dev runs). `serve` binds immediately — pass port 0 for an
//! ephemeral port — and returns a [`Serving`]: read the bound address,
//! then await it (or hand it to a runtime spawn).

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::Router;

use crate::bootstrap::Ctx;
use crate::server::{Server, shutdown_signal};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
type RouterFactory = Box<dyn FnOnce(&Ctx, SocketAddr) -> Result<Router, BoxError> + Send>;

pub struct App {
    ctx: Ctx,
    router: Option<RouterFactory>,
}

pub fn app(ctx: Ctx) -> App {
    App { ctx, router: None }
}

impl App {
    /// Replace the default (api-only) router. The factory receives the
    /// bound address — known only after `serve` binds, which is why
    /// this is a factory and not a router.
    #[must_use]
    pub fn with_router(
        mut self,
        f: impl FnOnce(&Ctx, SocketAddr) -> Result<Router, BoxError> + Send + 'static,
    ) -> Self {
        self.router = Some(Box::new(f));
        self
    }

    /// Ensure the app dirs, bind `listen`, build the router and return
    /// the serving future together with the bound address. The server
    /// runs until the shutdown signal (ctrl-c / SIGTERM).
    pub fn serve(self, listen: SocketAddr) -> Result<Serving, BoxError> {
        self.ctx.paths.ensure_dirs()?;
        let server = Server::bind(listen)?;
        let addr = server.local_addr()?;
        let router = match self.router {
            Some(f) => f(&self.ctx, addr)?,
            None => self.ctx.router(),
        };
        tracing::info!(%addr, "server up");
        Ok(Serving {
            addr,
            fut: Box::pin(server.serve(router, shutdown_signal())),
        })
    }
}

/// A bound, running server: [`Serving::addr`] is the real address
/// (ephemeral binds included); the future resolves when the server
/// shuts down.
pub struct Serving {
    addr: SocketAddr,
    fut: Pin<Box<dyn Future<Output = io::Result<()>> + Send>>,
}

impl Serving {
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Future for Serving {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.fut.as_mut().poll(cx)
    }
}
