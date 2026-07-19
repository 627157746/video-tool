mod commands;
mod config;
mod error;
mod models;
mod sidecar;
mod workspace;

use commands::AppState;
use config::AppConfig;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_config = AppConfig::load_or_init().unwrap_or_else(|error| {
        eprintln!("failed to load config, using defaults: {error}");
        AppConfig::default()
    });

    if let Err(error) = app_config.ensure_workspace() {
        eprintln!("failed to ensure workspace: {error}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(app_config),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_config,
            commands::reload_config,
            commands::list_jobs,
            commands::get_job,
            commands::create_download_job,
            commands::create_live_record_job,
            commands::create_import_job,
            commands::open_job_directory,
            commands::probe_sidecars,
            commands::mark_job_placeholder_failed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
