//! The Tauri app's commands, as the page calls them.

/// The `greet` command of the Tauri app.
#[cfg(feature = "hydrate")]
pub async fn greet(name: &str) -> Result<String, String> {
    let reply = tauri_leptos_core::tauri_ipc::invoke("greet", &[("name", &name.into())]).await?;
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
