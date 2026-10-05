use tauri_leptos_ui::config::AppConfig;
use tauri_leptos_ui::socket::{self, OriginPolicy};
use tauri_plugin_leptos_ssr::LeptosSsrExt;

#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("greet({name})");
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = AppConfig::default();
    let ws_addr = config.ws_addr;
    tauri::Builder::default()
        // Plugins set up in order: the logger first, so leptos-ssr can log.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .level_for("tauri_plugin_leptos_ssr", log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_leptos_ssr::init(move |options| {
            tauri_leptos_ui::server::router(options, config)
        }))
        .invoke_handler(tauri::generate_handler![greet])
        .setup(move |app| {
            let policy = OriginPolicy::Only(vec![app.leptos_ssr().origin().to_owned()]);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = socket::serve(ws_addr, policy).await {
                    log::error!("websocket on {ws_addr}: {error}");
                }
            });
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
