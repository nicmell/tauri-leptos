use tauri_leptos_ui::config::AppConfig;
use tauri_plugin_leptos_ssr::LeptosSsrExt;

#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("greet({name})");
    format!("Hello, {name}! You've been greeted from Rust!")
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
        .invoke_handler(tauri::generate_handler![greet])
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
