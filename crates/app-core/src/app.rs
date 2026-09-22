//! The app: one entrypoint shape for cli and shell. `app(ctx)` binds
//! immediately on the host-inferred address (the configured listen
//! standalone, an ephemeral port in the shell) — read [`App::addr`],
//! then [`App::start`] builds the router and serves until the
//! shutdown signal (await it, or hand it to a runtime spawn).

use std::net::SocketAddr;

use crate::bootstrap::Ctx;
use crate::server::{Server, shutdown_signal};

pub use crate::bootstrap::BoxError;

pub struct App {
    ctx: Ctx,
    server: Server,
}

/// Ensure the app dirs and bind. The bound address is known from here
/// on ([`App::addr`]).
pub fn app(ctx: Ctx) -> Result<App, BoxError> {
    ctx.paths.ensure_dirs()?;
    let server = Server::bind(ctx.listen())?;
    tracing::info!(addr = %server.local_addr()?, "server up");
    Ok(App { ctx, server })
}

impl App {
    /// The bound address (real port for ephemeral binds).
    ///
    /// # Panics
    /// Never in practice: the listener was just bound by [`app`].
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.server
            .local_addr()
            .expect("bound listener has an addr")
    }

    /// Build the host-inferred router and serve until ctrl-c/SIGTERM.
    pub async fn start(self) -> Result<(), BoxError> {
        let router = self.ctx.router(self.addr());
        self.server.serve(router, shutdown_signal()).await?;
        Ok(())
    }
}
