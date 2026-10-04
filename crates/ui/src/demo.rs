//! The demo page: a server function, one button.

use leptos::prelude::*;
use leptos::task::spawn_local;

/// Demo server function: typed isomorphic RPC, no hand-written endpoint.
// server functions must be async by contract, awaits or not
#[allow(clippy::unused_async)]
#[server]
async fn server_greet(name: String) -> Result<String, ServerFnError> {
    Ok(format!("Hello, {name}! (rendered by a server function)"))
}

#[component]
pub fn HomePage() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (server_msg, set_server_msg) = signal(String::new());

    let update_name = move |ev| {
        let v = event_target_value(&ev);
        set_name.set(v);
    };

    let greet_server_fn = move |_| {
        spawn_local(async move {
            let name = name.get_untracked();
            let message = match server_greet(if name.is_empty() {
                "world".into()
            } else {
                name
            })
            .await
            {
                Ok(m) => m,
                Err(e) => format!("server fn failed: {e}"),
            };
            set_server_msg.set(message);
        });
    };

    view! {
        <main class="container">
            <h1>"Welcome to Tauri + Leptos"</h1>

            <div class="row">
                <a href="https://tauri.app" target="_blank">
                    <img src="/public/tauri.svg" class="logo tauri" alt="Tauri logo" />
                </a>
                <a href="https://docs.rs/leptos/" target="_blank">
                    <img src="/public/leptos.svg" class="logo leptos" alt="Leptos logo" />
                </a>
            </div>
            <p>"Click on the Tauri and Leptos logos to learn more."</p>

            <div class="row">
                <input id="greet-input" placeholder="Enter a name..." on:input=update_name />
                <button on:click=greet_server_fn>"Server fn greet"</button>
            </div>
            <p>{move || server_msg.get()}</p>
        </main>
    }
}
