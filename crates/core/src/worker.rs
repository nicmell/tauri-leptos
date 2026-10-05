//! The pipe's web worker: it relays every value between the page's port and
//! the port of the page's websocket.

use js_sys::{Array, ArrayBuffer};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, MessagePort};

/// Runs the worker on `page`, the far end of the page's pipe, and `socket`,
/// the far end of its websocket; `/pipe-worker.js` calls it.
#[wasm_bindgen]
pub fn worker_main(page: &MessagePort, socket: &MessagePort) {
    console_error_panic_hook::set_once();
    relay(page, socket);
    relay(socket, page);
}

/// Posts each value from `from` to `to`, and moves its buffers.
fn relay(from: &MessagePort, to: &MessagePort) {
    let to = to.clone();
    let forward = Closure::<dyn Fn(MessageEvent)>::new(move |message: MessageEvent| {
        let data = message.data();
        let _ = if data.is_instance_of::<ArrayBuffer>() {
            to.post_message_with_transferable(&data, &Array::of1(&data))
        } else {
            to.post_message(&data)
        };
    });
    from.set_onmessage(Some(forward.as_ref().unchecked_ref()));
    // The worker lives as long as its page.
    forward.forget();
}
