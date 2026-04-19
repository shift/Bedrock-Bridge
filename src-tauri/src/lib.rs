mod profile;
mod proxy;
mod settings;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(proxy::ProxyHandle::default())
        .manage(profile::AppState::default())
        .invoke_handler(tauri::generate_handler![
            profile::list_profiles,
            profile::add_profile,
            profile::update_profile,
            profile::delete_profile,
            profile::activate_profile,
            profile::deactivate_profile,
            profile::export_profiles,
            profile::import_profiles,
            proxy::start_proxy,
            proxy::stop_proxy,
            proxy::get_traffic_stats,
            settings::set_autostart,
            settings::is_autostart_enabled,
        ])
        .setup(|app| {
            tracing_subscriber::fmt::init();
            tracing::info!("Bedrock Bridge starting up");

            // Check --hidden flag (desktop only)
            #[cfg(desktop)]
            {
                let args: Vec<String> = std::env::args().collect();
                if args.contains(&"--hidden".to_string()) {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                        tracing::info!("Started hidden (--hidden flag)");
                    }
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Bedrock Bridge");
}
