//! The demo page: a counter, a server function, a Tauri command and the pipe.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use tauri_leptos_core::page::Pipe;
use tauri_leptos_core::{Frame, PipeEvent};
use tauri_leptos_protocol::{ClientMessage, ServerMessage, decode, encode};

use crate::tauri_ipc;

/// The text the self-check sends through the pipe.
const SELF_CHECK: &str = "self-check";

/// The bytes the self-check sends through the pipe: 2 KiB, over the 1 KiB
/// above which Tauri carries bytes through its IPC fetch path.
fn self_check_bytes() -> Vec<u8> {
    (0..=u8::MAX).cycle().take(2048).collect()
}

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
    let pipe_check = RwSignal::new(String::new());

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
        <p id="self-check">{move || format!("{}{}", self_check.get(), pipe_check.get())}</p>

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
        <PipeSection pipe_check />
    }
}

/// The pipe: its state, the session's tick, and an echo through it. The
/// self-check's two echoes, text and binary, are reported to `pipe_check`.
#[component]
fn PipeSection(pipe_check: RwSignal<String>) -> impl IntoView {
    let state = RwSignal::new(String::from("Not open."));
    let tick = RwSignal::new(0_u64);
    let echo_text = RwSignal::new(String::from("Hello through the pipe"));
    let echo_reply = RwSignal::new(String::new());
    let echoes = RwSignal::new((false, false));
    let pipe = StoredValue::new_local(None::<Pipe>);
    let send = move |frame: Frame| {
        pipe.with_value(|pipe| {
            if let Some(pipe) = pipe {
                pipe.send(frame);
            }
        });
    };

    Effect::new(move || {
        let on_event = move |event| match event {
            PipeEvent::Connected => {
                state.set("Connected.".into());
                send(Frame::Text(encode(&ClientMessage::Echo {
                    text: SELF_CHECK.into(),
                })));
                send(Frame::Binary(self_check_bytes()));
            }
            PipeEvent::Disconnected => {
                state.set("Disconnected, reconnecting.".into());
                set_timeout(
                    move || {
                        pipe.with_value(|pipe| {
                            if let Some(pipe) = pipe {
                                pipe.reconnect();
                            }
                        });
                    },
                    Duration::from_secs(1),
                );
            }
            PipeEvent::Frame(Frame::Text(json)) => match decode(&json) {
                Ok(ServerMessage::Tick { count }) => tick.set(count),
                Ok(ServerMessage::Echo { text }) => {
                    if text == SELF_CHECK {
                        echoes.update(|(text, _)| *text = true);
                    }
                    echo_reply.set(text);
                }
                Err(_) => {}
            },
            PipeEvent::Frame(Frame::Binary(bytes)) => {
                if bytes == self_check_bytes() {
                    echoes.update(|(_, binary)| *binary = true);
                }
            }
        };
        match Pipe::open(on_event) {
            Ok(opened) => pipe.set_value(Some(opened)),
            Err(error) => state.set(format!("error: {error}")),
        }
    });
    Effect::new(move || {
        if echoes.get() == (true, true) {
            pipe_check.set(" Pipe: both echoes answered.".into());
        }
    });

    let send_echo = move |_| {
        send(Frame::Text(encode(&ClientMessage::Echo {
            text: echo_text.get_untracked(),
        })));
    };

    view! {
        <section>
            <p>"Pipe: " {move || state.get()} " Tick " {tick} "."</p>
            <input bind:value=echo_text />
            <button on:click=send_echo>"Send through the pipe"</button>
            <p>{move || echo_reply.get()}</p>
        </section>
    }
}
