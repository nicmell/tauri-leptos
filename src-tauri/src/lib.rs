use tauri::ipc::Channel;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, State, Webview, WindowEvent};
use tauri_leptos_core::pipes::Pipes;
use tauri_leptos_ui::config::AppConfig;
use tauri_plugin_leptos_ssr::LeptosSsrExt;

#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("greet({name})");
    format!("Hello, {name}! You've been greeted from Rust!")
}

/// Opens the page's pipe; `events` carries what its session reports.
// Tauri hands State and Webview over by value only.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn pipe_open(webview: Webview, events: Channel<String>, pipes: State<'_, Pipes>) -> u32 {
    pipes.open(webview.label(), move |event| {
        let _ = events.send(event);
    })
}

/// Hands `data`, an encoded `ClientMessage`, to the session of pipe `id`.
// Tauri hands State over by value only.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn pipe_post(id: u32, data: &str, pipes: State<'_, Pipes>) -> Result<(), String> {
    pipes.post(id, data)
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
        .manage(Pipes::new(tauri::async_runtime::handle().inner().clone()))
        .invoke_handler(tauri::generate_handler![greet, pipe_open, pipe_post])
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
