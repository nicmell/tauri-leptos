use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::webview::PageLoadEvent;
use tauri::{Manager, State, Webview, WindowEvent};
use tauri_leptos_core::pipes::Pipes;
use tauri_leptos_core::{Frame, PipeEvent};
use tauri_leptos_ui::config::AppConfig;
use tauri_plugin_leptos_ssr::LeptosSsrExt;

#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("greet({name})");
    format!("Hello, {name}! You've been greeted from Rust!")
}

/// Opens the page's pipe; `events` carries what the page hears from it.
// Tauri hands State and Webview over by value only.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn pipe_open(
    webview: Webview,
    events: Channel<InvokeResponseBody>,
    pipes: State<'_, Pipes>,
) -> u32 {
    pipes.open(webview.label(), move |event| {
        let _ = events.send(body(event));
    })
}

/// `event` as the page's end of the pipe reads it: see "The pipe" in
/// `docs/architecture.md`.
fn body(event: PipeEvent) -> InvokeResponseBody {
    match event {
        PipeEvent::Connected => InvokeResponseBody::Json(r#"{"event":"connected"}"#.to_owned()),
        PipeEvent::Disconnected => {
            InvokeResponseBody::Json(r#"{"event":"disconnected"}"#.to_owned())
        }
        PipeEvent::Frame(Frame::Text(text)) => InvokeResponseBody::Json(
            serde_json::to_string(&text).expect("a string serializes to JSON"),
        ),
        PipeEvent::Frame(Frame::Binary(bytes)) => InvokeResponseBody::Raw(bytes),
    }
}

/// Hands `frame` to the session of pipe `id`.
// Tauri hands State over by value only.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
async fn pipe_post(id: u32, frame: Frame, pipes: State<'_, Pipes>) -> Result<(), String> {
    pipes.post(id, frame).await
}

/// Closes pipe `id`, which ends its session.
// Tauri hands State over by value only.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn pipe_close(id: u32, pipes: State<'_, Pipes>) {
    pipes.close(id);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Plugins set up in order: the logger first, so leptos-ssr can log.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("tauri_plugin_leptos_ssr", log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_leptos_ssr::init(|options| {
            tauri_leptos_ui::server::router(options, AppConfig::default())
        }))
        .manage(Pipes::new(
            tauri::async_runtime::handle().inner().clone(),
            tauri_leptos_protocol::session(),
        ))
        .invoke_handler(tauri::generate_handler![
            greet, pipe_open, pipe_post, pipe_close
        ])
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                webview.state::<Pipes>().close_webview(webview.label());
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                window.state::<Pipes>().close_webview(window.label());
            }
        })
        .setup(|app| {
            let url = app.leptos_ssr().webview_url("/")?;
            tauri::WebviewWindowBuilder::new(app, "main", url)
                .title("tauri-leptos")
                .inner_size(800.0, 600.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
