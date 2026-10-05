//! The page's web worker: it holds the websocket and relays its messages to
//! the page.
#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;

use tauri_leptos_protocol::{
    ClientMessage, ServerMessage, WorkerCommand, WorkerEvent, decode, encode,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, WebSocket};

/// The wait before the worker opens a closed websocket again.
const REOPEN_AFTER_MS: i32 = 1_000;

/// Runs the worker in the current worker scope.
pub fn run() {
    console_error_panic_hook::set_once();
    let worker = Rc::new(Worker {
        scope: js_sys::global().unchecked_into(),
        url: RefCell::new(None),
        connection: RefCell::new(None),
    });
    let on_command = {
        let worker = worker.clone();
        Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
            match message.data().as_string().map(|json| decode(&json)) {
                Some(Ok(command)) => worker.handle(command),
                _ => web_sys::console::error_1(&"worker: not a WorkerCommand".into()),
            }
        })
    };
    worker
        .scope
        .set_onmessage(Some(on_command.as_ref().unchecked_ref()));
    on_command.forget();
    worker.post(&WorkerEvent::Ready);
}

struct Worker {
    scope: DedicatedWorkerGlobalScope,
    url: RefCell<Option<String>>,
    connection: RefCell<Option<Connection>>,
}

/// An open websocket and its handlers, which must live as long as it does.
struct Connection {
    socket: WebSocket,
    _on_open: Closure<dyn Fn()>,
    _on_message: Closure<dyn Fn(MessageEvent)>,
    _on_close: Closure<dyn Fn()>,
}

impl Worker {
    fn handle(self: &Rc<Self>, command: WorkerCommand) {
        match command {
            WorkerCommand::Connect { url } => {
                *self.url.borrow_mut() = Some(url);
                self.open();
            }
            WorkerCommand::Send { message } => self.send(&message),
        }
    }

    fn post(&self, event: &WorkerEvent) {
        let _ = self.scope.post_message(&encode(event).into());
    }

    fn send(&self, message: &ClientMessage) {
        if let Some(connection) = self.connection.borrow().as_ref()
            && connection.socket.ready_state() == WebSocket::OPEN
        {
            let _ = connection.socket.send_with_str(&encode(message));
        }
    }

    fn open(self: &Rc<Self>) {
        if let Some(old) = self.connection.borrow_mut().take() {
            old.socket.set_onopen(None);
            old.socket.set_onmessage(None);
            old.socket.set_onclose(None);
            let _ = old.socket.close();
        }
        let Some(url) = self.url.borrow().clone() else {
            return;
        };
        let Ok(socket) = WebSocket::new(&url) else {
            return self.open_later();
        };
        let on_open = {
            let worker = self.clone();
            Closure::<dyn Fn()>::new(move || worker.post(&WorkerEvent::Connected))
        };
        let on_message = {
            let worker = self.clone();
            Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
                let decoded = message
                    .data()
                    .as_string()
                    .map(|json| decode::<ServerMessage>(&json));
                if let Some(Ok(message)) = decoded {
                    worker.post(&WorkerEvent::Received { message });
                }
            })
        };
        let on_close = {
            let worker = self.clone();
            Closure::<dyn Fn()>::new(move || {
                worker.post(&WorkerEvent::Disconnected);
                worker.open_later();
            })
        };
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        *self.connection.borrow_mut() = Some(Connection {
            socket,
            _on_open: on_open,
            _on_message: on_message,
            _on_close: on_close,
        });
    }

    fn open_later(self: &Rc<Self>) {
        let worker = self.clone();
        let open = Closure::once_into_js(move || worker.open());
        let _ = self
            .scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                open.unchecked_ref(),
                REOPEN_AFTER_MS,
            );
    }
}
