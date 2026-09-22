use futures::{SinkExt, StreamExt};
use gloo_net::http::Request;
use gloo_net::websocket::{Message, futures::WebSocket};
use leptos::prelude::*;
use leptos::task::spawn_local;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use serde::Deserialize;

/// The SSR document shell; cargo-leptos injects the hydration assets.
///
/// `api_base` tells the page where the api lives: `None` = same origin
/// (empty meta). The meta is always rendered — a conditional view in
/// the head would not emit.
#[cfg(feature = "ssr")]
pub(crate) fn shell(options: LeptosOptions, api_base: Option<String>) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="api-base" content=api_base.unwrap_or_default() />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
                <link rel="stylesheet" href="/styles.css" />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text="Tauri + Leptos" />
        <Router>
            <Routes fallback=|| "not found">
                <Route path=path!("") view=HomePage />
            </Routes>
        </Router>
    }
}

/// Where the client sends api/ws requests: the origin from the
/// `api-base` meta when present, otherwise "" = same origin.
fn api_base() -> String {
    document()
        .query_selector("meta[name='api-base']")
        .ok()
        .flatten()
        .and_then(|meta| meta.get_attribute("content"))
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct HelloResponse {
    message: String,
}

/// Demo server function: typed isomorphic RPC, no hand-written endpoint.
// server functions must be async by contract, awaits or not
#[allow(clippy::unused_async)]
#[server]
async fn server_greet(name: String) -> Result<String, ServerFnError> {
    Ok(format!("Hello, {name}! (rendered by a server function)"))
}

/// One round trip through the api server's `/ws` echo endpoint.
async fn ws_roundtrip(text: &str) -> Result<String, String> {
    let base = api_base();
    let ws_url = if base.is_empty() {
        let location = window().location();
        let host = location.host().map_err(|_| "no window host".to_owned())?;
        let scheme = if location.protocol().map_err(|_| "no protocol".to_owned())? == "https:" {
            "wss"
        } else {
            "ws"
        };
        format!("{scheme}://{host}/ws")
    } else if let Some(rest) = base.strip_prefix("https") {
        format!("wss{rest}/ws")
    } else if let Some(rest) = base.strip_prefix("http") {
        format!("ws{rest}/ws")
    } else {
        return Err(format!("unsupported api base: {base}"));
    };
    let mut ws = WebSocket::open(&ws_url).map_err(|e| e.to_string())?;
    ws.send(Message::Text(text.to_owned()))
        .await
        .map_err(|e| e.to_string())?;
    let reply = match ws.next().await {
        Some(Ok(Message::Text(t))) => t,
        Some(Ok(Message::Bytes(_))) => return Err("unexpected binary reply".to_owned()),
        Some(Err(e)) => return Err(e.to_string()),
        None => return Err("connection closed".to_owned()),
    };
    ws.close(None, None).map_err(|e| e.to_string())?;
    Ok(reply)
}

#[component]
fn HomePage() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (greet_msg, set_greet_msg) = signal(String::new());
    let (server_msg, set_server_msg) = signal(String::new());
    let (echo_msg, set_echo_msg) = signal(String::new());

    let update_name = move |ev| {
        let v = event_target_value(&ev);
        set_name.set(v);
    };

    let greet = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let name = name.get_untracked();
            if name.is_empty() {
                return;
            }

            let message = match Request::get(&format!("{}/api/hello", api_base()))
                .query([("name", name.as_str())])
                .send()
                .await
            {
                Ok(response) => match response.json::<HelloResponse>().await {
                    Ok(body) => body.message,
                    Err(e) => format!("invalid response: {e}"),
                },
                Err(e) => format!("request failed: {e}"),
            };
            set_greet_msg.set(message);
        });
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

    let ws_echo = move |_| {
        spawn_local(async move {
            let message = match ws_roundtrip("ping from the ui").await {
                Ok(reply) => format!("echo: {reply}"),
                Err(e) => format!("websocket failed: {e}"),
            };
            set_echo_msg.set(message);
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

            <form class="row" on:submit=greet>
                <input id="greet-input" placeholder="Enter a name..." on:input=update_name />
                <button type="submit">"Greet"</button>
            </form>
            <p>{move || greet_msg.get()}</p>

            <div class="row">
                <button on:click=greet_server_fn>"Server fn greet"</button>
                <button on:click=ws_echo>"WebSocket echo"</button>
            </div>
            <p>{move || server_msg.get()}</p>
            <p>{move || echo_msg.get()}</p>
        </main>
    }
}
