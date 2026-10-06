//! Calls into the Tauri app through `window.__TAURI__` (`app.withGlobalTauri`).

use js_sys::{Function, Object, Reflect};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"], js_name = invoke, catch)]
    async fn invoke_command(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    /// The page's end of a `tauri::ipc::Channel` argument of a command.
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"])]
    #[derive(Clone)]
    pub type Channel;

    #[wasm_bindgen(constructor, js_namespace = ["__TAURI__", "core"], catch)]
    pub fn new() -> Result<Channel, JsValue>;

    /// The handler of each message on the channel.
    #[wasm_bindgen(method, getter)]
    pub fn onmessage(this: &Channel) -> Function;

    #[wasm_bindgen(method, setter)]
    pub fn set_onmessage(this: &Channel, handler: &Function);
}

/// Whether the page runs in the Tauri app.
pub fn available() -> bool {
    Reflect::has(&js_sys::global(), &"__TAURI__".into()).unwrap_or(false)
}

/// Calls the command `cmd` of the Tauri app with the named `args`.
pub async fn invoke(cmd: &str, args: &[(&str, &JsValue)]) -> Result<JsValue, String> {
    // Without the global, the binding throws past `catch` and the caller's
    // task never finishes.
    if !available() {
        return Err("Tauri commands run in the Tauri app only.".to_owned());
    }
    let object = Object::new();
    for (name, value) in args {
        Reflect::set(&object, &(*name).into(), value).map_err(|error| describe(&error))?;
    }
    invoke_command(cmd, object.into())
        .await
        .map_err(|error| error.as_string().unwrap_or_else(|| describe(&error)))
}

pub(crate) fn describe(error: &JsValue) -> String {
    format!("{error:?}")
}
