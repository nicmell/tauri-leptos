//! The far end of a pipe, which the app brings: an async function of the
//! page's frames and of the frames it sends back.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::Frame;

/// Runs the session of one pipe. A session ends when the page's frames end,
/// or when it returns on its own.
pub type Session = Arc<
    dyn Fn(mpsc::Receiver<Frame>, mpsc::Sender<Frame>) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// The [`Session`] that runs `f` for each pipe.
pub fn session<F, Fut>(f: F) -> Session
where
    F: Fn(mpsc::Receiver<Frame>, mpsc::Sender<Frame>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Arc::new(move |incoming, outgoing| Box::pin(f(incoming, outgoing)))
}
