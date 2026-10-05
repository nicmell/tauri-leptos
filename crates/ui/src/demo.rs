//! The demo page: a counter, a server function, a Tauri command and a
//! websocket worker; and the websocket session behind the worker.

use leptos::prelude::*;
use leptos::task::spawn_local;
use tauri_leptos_protocol::{ClientMessage, ServerMessage, WorkerEvent};

use crate::config::AppConfig;
use crate::socket::{self, SocketWorker};
use crate::tauri_ipc;

/// The text the self-check sends through the worker.
const SELF_CHECK: &str = "self-check";

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
    let socket_check = RwSignal::new(String::new());

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
        <p id="self-check">{move || format!("{}{}", self_check.get(), socket_check.get())}</p>

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
        <SocketSection socket_check />
    }
}

/// The websocket worker: its state, the server's tick, and an echo through
/// it. The self-check's echo is reported to `socket_check`.
#[component]
fn SocketSection(socket_check: RwSignal<String>) -> impl IntoView {
    let state = RwSignal::new(String::from("Not started."));
    let tick = RwSignal::new(0_u64);
    let echo_text = RwSignal::new(String::from("Hello through the worker"));
    let echo_reply = RwSignal::new(String::new());
    let socket = StoredValue::new_local(None::<SocketWorker>);
    let ws_addr = SharedValue::new(|| {
        use_context::<AppConfig>()
            .unwrap_or_default()
            .ws_addr
            .to_string()
    })
    .into_inner();

    Effect::new(move || {
        let on_event = move |event| match event {
            WorkerEvent::Ready => state.set("Connecting.".into()),
            WorkerEvent::Connected => {
                state.set("Connected.".into());
                socket.with_value(|socket| {
                    if let Some(socket) = socket {
                        socket.send(ClientMessage::Echo {
                            text: SELF_CHECK.into(),
                        });
                    }
                });
            }
            WorkerEvent::Disconnected => state.set("Disconnected, reconnecting.".into()),
            WorkerEvent::Received {
                message: ServerMessage::Tick { count },
            } => tick.set(count),
            WorkerEvent::Received {
                message: ServerMessage::Echo { text },
            } => {
                if text == SELF_CHECK {
                    socket_check.set(" Websocket worker: the echo answered.".into());
                }
                echo_reply.set(text);
            }
        };
        match SocketWorker::spawn(socket::url(&ws_addr), on_event) {
            Ok(worker) => socket.set_value(Some(worker)),
            Err(error) => state.set(format!("error: {error}")),
        }
    });

    let send_echo = move |_| {
        socket.with_value(|socket| {
            if let Some(socket) = socket {
                socket.send(ClientMessage::Echo {
                    text: echo_text.get_untracked(),
                });
            }
        });
    };

    view! {
        <section>
            <p>"Websocket worker: " {move || state.get()} " Tick " {tick} "."</p>
            <input bind:value=echo_text />
            <button on:click=send_echo>"Send through the worker"</button>
            <p>{move || echo_reply.get()}</p>
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
