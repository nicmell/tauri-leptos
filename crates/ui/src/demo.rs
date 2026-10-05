//! The demo page: a counter, a server function and a Tauri command.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::tauri_ipc;

/// The executable that ran the server function, and its OS: the cli under
/// `cargo leptos watch` in dev, the Tauri app itself in release builds.
// server functions must be async by contract, awaits or not
#[allow(clippy::unused_async)]
#[server]
pub async fn whoami() -> Result<String, ServerFnError> {
    let exe = std::env::current_exe().map_err(|error| ServerFnError::new(error.to_string()))?;
    let name = exe
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(format!("{name} ({})", std::env::consts::OS))
}

#[component]
pub fn HomePage() -> impl IntoView {
    let count = RwSignal::new(0);
    let name = RwSignal::new(String::from("Leptos"));
    let server_reply = RwSignal::new(String::new());
    let command_reply = RwSignal::new(String::new());
    let self_check = RwSignal::new(String::from("Not hydrated yet."));

    // Effects run in the browser only: once, right after hydration.
    Effect::new(move || {
        spawn_local(async move {
            let server = whoami()
                .await
                .unwrap_or_else(|error| format!("error: {error}"));
            let command = tauri_ipc::greet("self-check")
                .await
                .unwrap_or_else(|error| format!("error: {error}"));
            self_check.set(format!(
                "Hydrated. Server function: {server}. Tauri command: {command}"
            ));
        });
    });

    let ask_server = move |_| {
        spawn_local(async move {
            let reply = whoami()
                .await
                .unwrap_or_else(|error| format!("error: {error}"));
            server_reply.set(format!("The server function ran in {reply}."));
        });
    };
    let greet = move |_| {
        spawn_local(async move {
            let reply = tauri_ipc::greet(&name.get_untracked())
                .await
                .unwrap_or_else(|error| format!("error: {error}"));
            command_reply.set(reply);
        });
    };

    view! {
        <h1>"Tauri + Leptos SSR"</h1>
        <div class="logos">
            <img src="/tauri.svg" class="logo" alt="Tauri logo" />
            <img src="/leptos.svg" class="logo" alt="Leptos logo" />
        </div>
        <p id="self-check">{move || self_check.get()}</p>

        <section>
            <button on:click=move |_| *count.write() += 1>"Clicked " {count} " times"</button>
        </section>
        <section>
            <button on:click=ask_server>"Call the server function"</button>
            <p>{move || server_reply.get()}</p>
        </section>
        <section>
            <input bind:value=name />
            <button on:click=greet>"Call the Tauri command"</button>
            <p>{move || command_reply.get()}</p>
        </section>
    }
}

/// The demo websocket session: a `Tick` every second, and an `Echo` answer
/// to each `Echo` the page sends.
#[cfg(feature = "ssr")]
pub async fn session(mut socket: axum::extract::ws::WebSocket) {
    use std::time::Duration;

    use axum::extract::ws::Message;
    use tauri_leptos_protocol::{ClientMessage, ServerMessage, decode, encode};

    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    let mut count = 0;
    loop {
        let reply = tokio::select! {
            _ = ticks.tick() => {
                count += 1;
                ServerMessage::Tick { count }
            }
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => match decode(text.as_str()) {
                    Ok(ClientMessage::Echo { text }) => ServerMessage::Echo { text },
                    Err(_) => continue,
                },
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => return,
            },
        };
        if socket
            .send(Message::Text(encode(&reply).into()))
            .await
            .is_err()
        {
            return;
        }
    }
}
