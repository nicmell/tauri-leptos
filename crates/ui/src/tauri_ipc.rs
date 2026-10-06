//! The Tauri app's commands, as the page calls them through
//! `window.__TAURI__` (`app.withGlobalTauri`).

/// The `greet` command of the Tauri app.
#[cfg(feature = "hydrate")]
pub async fn greet(name: &str) -> Result<String, String> {
    let reply = invoke("greet", &[("name", &name.into())]).await?;
    reply
        .as_string()
        .ok_or_else(|| "`greet` did not return a string".to_owned())
}

/// The `greet` command of the Tauri app; unreachable outside the browser.
#[cfg(not(feature = "hydrate"))]
// async like the hydrate version, which its callers await
#[allow(clippy::unused_async)]
pub async fn greet(_name: &str) -> Result<String, String> {
    Err("Tauri commands are called from the browser".to_owned())
}

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"], js_name = invoke, catch)]
    async fn invoke_command(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

/// Calls the command `cmd` of the Tauri app with the named `args`.
#[cfg(feature = "hydrate")]
async fn invoke(cmd: &str, args: &[(&str, &JsValue)]) -> Result<JsValue, String> {
    use js_sys::{Object, Reflect};

    // Without the global, the binding throws past `catch` and the caller's
    // task never finishes.
    if !Reflect::has(&js_sys::global(), &"__TAURI__".into()).unwrap_or(false) {
        return Err("Tauri commands run in the Tauri app only.".to_owned());
    }
    let object = Object::new();
    for (name, value) in args {
        Reflect::set(&object, &(*name).into(), value).map_err(|error| format!("{error:?}"))?;
    }
    invoke_command(cmd, object.into())
        .await
        .map_err(|error| error.as_string().unwrap_or_else(|| format!("{error:?}")))
}
