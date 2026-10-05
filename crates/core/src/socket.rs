//! The pipe's websocket, on the page's main thread: a socket to `/ws` on the
//! page's own origin, relayed to the worker through a port.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use js_sys::{Array, ArrayBuffer, Object, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, MessageEvent, MessagePort, WebSocket};

/// The socket, the port to the worker, and the handlers that must live as
/// long as they do. Dropping it closes both.
pub(crate) struct Socket {
    url: String,
    port: MessagePort,
    connection: RefCell<Option<Connection>>,
    on_port: Closure<dyn Fn(MessageEvent)>,
}

struct Connection {
    socket: WebSocket,
    _on_open: Closure<dyn Fn()>,
    _on_message: Closure<dyn Fn(MessageEvent)>,
    _on_close: Closure<dyn Fn()>,
}

impl Socket {
    /// Relays `port` to a websocket to `/ws`, and opens it.
    pub(crate) fn open(port: MessagePort) -> Rc<Self> {
        let socket = Rc::new_cyclic(|this: &Weak<Self>| {
            let this = this.clone();
            Self {
                url: url(),
                port,
                connection: RefCell::new(None),
                on_port: Closure::new(move |message: MessageEvent| {
                    if let Some(socket) = this.upgrade() {
                        socket.from_worker(&message.data());
                    }
                }),
            }
        });
        socket
            .port
            .set_onmessage(Some(socket.on_port.as_ref().unchecked_ref()));
        socket.connect();
        socket
    }

    /// A frame for the socket, or the worker's request to connect again.
    fn from_worker(self: &Rc<Self>, data: &JsValue) {
        if let Some(text) = data.as_string() {
            self.send(|socket| socket.send_with_str(&text));
        } else if let Some(buffer) = data.dyn_ref::<ArrayBuffer>() {
            self.send(|socket| socket.send_with_array_buffer(buffer));
        } else if field(data, "command").as_deref() == Some("connect") {
            self.connect();
        }
    }

    /// Writes to an open socket; a frame for a closed one is dropped.
    fn send(&self, write: impl FnOnce(&WebSocket) -> Result<(), JsValue>) {
        if let Some(connection) = self.connection.borrow().as_ref()
            && connection.socket.ready_state() == WebSocket::OPEN
        {
            let _ = write(&connection.socket);
        }
    }

    fn post_event(&self, event: &str) {
        let object = Object::new();
        let _ = Reflect::set(&object, &"event".into(), &event.into());
        let _ = self.port.post_message(&object);
    }

    fn connect(self: &Rc<Self>) {
        self.disconnect();
        let Ok(socket) = WebSocket::new(&self.url) else {
            self.post_event("disconnected");
            return;
        };
        socket.set_binary_type(BinaryType::Arraybuffer);
        let weak = Rc::downgrade(self);
        let on_open = handler(&weak, |socket| socket.post_event("connected"));
        let on_close = handler(&weak, |socket| socket.post_event("disconnected"));
        let on_message = Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
            let Some(socket) = weak.upgrade() else {
                return;
            };
            let data = message.data();
            // The socket's buffer is fresh, so it moves to the worker.
            let _ = if data.is_instance_of::<ArrayBuffer>() {
                socket
                    .port
                    .post_message_with_transferable(&data, &Array::of1(&data))
            } else {
                socket.port.post_message(&data)
            };
        });
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

    /// Detached first: a replaced socket's late `onclose` would otherwise
    /// tell the worker `disconnected`.
    fn disconnect(&self) {
        if let Some(old) = self.connection.borrow_mut().take() {
            old.socket.set_onopen(None);
            old.socket.set_onmessage(None);
            old.socket.set_onclose(None);
            let _ = old.socket.close();
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.port.set_onmessage(None);
        self.port.close();
        self.disconnect();
    }
}

fn handler(socket: &Weak<Socket>, run: fn(&Socket)) -> Closure<dyn Fn()> {
    let socket = socket.clone();
    Closure::<dyn Fn()>::new(move || {
        if let Some(socket) = socket.upgrade() {
            run(&socket);
        }
    })
}

/// `/ws` on the page's origin: `ws:` for `http:`, `wss:` for `https:`, and
/// the page's own scheme otherwise. On macOS, the Tauri app's pages are on
/// `leptos://localhost`, which tauri-plugin-leptos-ssr takes.
fn url() -> String {
    let location = web_sys::window().expect("a page").location();
    let protocol = location.protocol().unwrap_or_default();
    let scheme = match protocol.as_str() {
        "http:" => "ws:",
        "https:" => "wss:",
        other => other,
    };
    format!("{scheme}//{}/ws", location.host().unwrap_or_default())
}

/// The string at `name` of an object, if `value` is one.
fn field(value: &JsValue, name: &str) -> Option<String> {
    if !value.is_object() {
        return None;
    }
    Reflect::get(value, &name.into()).ok()?.as_string()
}
