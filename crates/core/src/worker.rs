//! The web worker at the far end of a browser's pipe: it holds a websocket to
//! `/ws` on its own origin, and relays frames between it and the page.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, ArrayBuffer, Object, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, MessageEvent, MessagePort, WebSocket, WorkerGlobalScope};

/// Runs the worker on `port`, the far end of the page's pipe;
/// `/pipe-worker.js` calls it.
#[wasm_bindgen]
pub fn worker_main(port: MessagePort) {
    console_error_panic_hook::set_once();
    let location = js_sys::global()
        .unchecked_into::<WorkerGlobalScope>()
        .location();
    let scheme = if location.protocol() == "https:" {
        "wss"
    } else {
        "ws"
    };
    let worker = Rc::new(Worker {
        url: format!("{scheme}://{}/ws", location.host()),
        port,
        connection: RefCell::new(None),
    });
    let on_message = {
        let worker = worker.clone();
        Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
            worker.from_page(&message.data());
        })
    };
    worker
        .port
        .set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    worker.open();
}

struct Worker {
    url: String,
    port: MessagePort,
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
    /// A frame for the socket, or the page's request to connect again.
    fn from_page(self: &Rc<Self>, data: &JsValue) {
        if let Some(text) = data.as_string() {
            self.send(|socket| socket.send_with_str(&text));
        } else if let Some(buffer) = data.dyn_ref::<ArrayBuffer>() {
            self.send(|socket| socket.send_with_array_buffer(buffer));
        } else if field(data, "command").as_deref() == Some("connect") {
            self.open();
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

    fn open(self: &Rc<Self>) {
        self.close();
        let Ok(socket) = WebSocket::new(&self.url) else {
            self.post_event("disconnected");
            return;
        };
        socket.set_binary_type(BinaryType::Arraybuffer);
        let on_open = {
            let worker = self.clone();
            Closure::<dyn Fn()>::new(move || worker.post_event("connected"))
        };
        let on_message = {
            let worker = self.clone();
            Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
                let data = message.data();
                // The socket's buffer is fresh, so it moves to the page.
                let _ = if data.is_instance_of::<ArrayBuffer>() {
                    worker
                        .port
                        .post_message_with_transferable(&data, &Array::of1(&data))
                } else {
                    worker.port.post_message(&data)
                };
            })
        };
        let on_close = {
            let worker = self.clone();
            Closure::<dyn Fn()>::new(move || worker.post_event("disconnected"))
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

    /// Detached first: a replaced socket's late `onclose` would otherwise
    /// tell the page `disconnected`.
    fn close(&self) {
        if let Some(old) = self.connection.borrow_mut().take() {
            old.socket.set_onopen(None);
            old.socket.set_onmessage(None);
            old.socket.set_onclose(None);
            let _ = old.socket.close();
        }
    }
}

/// The string at `name` of an object, if `value` is one.
fn field(value: &JsValue, name: &str) -> Option<String> {
    if !value.is_object() {
        return None;
    }
    Reflect::get(value, &name.into()).ok()?.as_string()
}
