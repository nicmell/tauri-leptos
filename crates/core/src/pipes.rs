//! The pipes of the Tauri app's webviews: each runs a session, which its page
//! reaches over Tauri channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use tauri_leptos_protocol::{ClientMessage, PipeEvent, decode, encode};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::session::session;

/// The open pipes, by id.
pub struct Pipes {
    runtime: Handle,
    next: AtomicU32,
    open: Mutex<HashMap<u32, Pipe>>,
}

struct Pipe {
    webview: String,
    to_session: mpsc::Sender<ClientMessage>,
    closed: Arc<AtomicBool>,
}

impl Pipes {
    /// Pipes whose sessions run on `runtime`.
    pub fn new(runtime: Handle) -> Self {
        Self {
            runtime,
            next: AtomicU32::new(1),
            open: Mutex::new(HashMap::new()),
        }
    }

    /// Opens a pipe for `webview`: starts a session and hands each event of
    /// it to `emit` as JSON. Returns the pipe's id.
    pub fn open(&self, webview: &str, emit: impl Fn(String) + Send + Sync + 'static) -> u32 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (to_session, incoming) = mpsc::channel(16);
        let (outgoing, mut from_session) = mpsc::channel(16);
        let closed = Arc::new(AtomicBool::new(false));
        self.runtime.spawn(session(incoming, outgoing));
        self.runtime.spawn({
            let closed = closed.clone();
            async move {
                // Once its page is gone, a channel still takes messages and the
                // next page logs them as unknown callbacks.
                let emit = |event: &PipeEvent| {
                    if !closed.load(Ordering::Acquire) {
                        emit(encode(event));
                    }
                };
                emit(&PipeEvent::Connected);
                while let Some(message) = from_session.recv().await {
                    emit(&PipeEvent::Received { message });
                }
                emit(&PipeEvent::Disconnected);
            }
        });
        let pipe = Pipe {
            webview: webview.to_owned(),
            to_session,
            closed,
        };
        self.lock().insert(id, pipe);
        log::info!("pipe {id} open for webview {webview}");
        id
    }

    /// Hands `data`, an encoded `ClientMessage`, to the session of pipe `id`.
    pub fn post(&self, id: u32, data: &str) -> Result<(), String> {
        let message: ClientMessage = decode(data).map_err(|error| error.to_string())?;
        let to_session = self
            .lock()
            .get(&id)
            .map(|pipe| pipe.to_session.clone())
            .ok_or_else(|| format!("no pipe {id}"))?;
        to_session
            .try_send(message)
            .map_err(|error| error.to_string())
    }

    /// Closes pipe `id`, which ends its session.
    pub fn close(&self, id: u32) {
        self.remove(|pipe_id, _| pipe_id == id);
    }

    /// Closes the pipes of `webview`, which ends their sessions.
    pub fn close_webview(&self, webview: &str) {
        self.remove(|_, pipe| pipe.webview == webview);
    }

    fn remove(&self, closes: impl Fn(u32, &Pipe) -> bool) {
        let removed: Vec<(u32, Pipe)> = self
            .lock()
            .extract_if(|id, pipe| closes(*id, pipe))
            .collect();
        for (id, pipe) in removed {
            pipe.closed.store(true, Ordering::Release);
            log::info!("pipe {id} of webview {} closed", pipe.webview);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Pipe>> {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
