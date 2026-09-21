//! The app: one entrypoint shape for cli and shell. `serve` binds
//! immediately — pass port 0 for an ephemeral port — builds the
//! host-inferred router ([`Ctx::router`]) on the bound address and
//! returns a [`Serving`]: read the address, then await it (or hand it
//! to a runtime spawn).

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::bootstrap::Ctx;
use crate::server::{Server, shutdown_signal};

pub use crate::bootstrap::BoxError;

pub struct App {
    ctx: Ctx,
}

pub fn app(ctx: Ctx) -> App {
    App { ctx }
}

impl App {
    /// Ensure the app dirs, bind `listen`, build the router and return
    /// the serving future together with the bound address. The server
    /// runs until the shutdown signal (ctrl-c / SIGTERM).
    pub fn serve(self, listen: SocketAddr) -> Result<Serving, BoxError> {
        self.ctx.paths.ensure_dirs()?;
        let server = Server::bind(listen)?;
        let addr = server.local_addr()?;
        let router = self.ctx.router(addr)?;
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
