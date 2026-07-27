mod commands;
mod config;
mod distribution;
mod error;
mod models;
mod pipeline;
mod search;
mod sidecar;
mod storage;
mod workspace;

use commands::AppState;
use config::AppConfig;
use error::{AppError, AppResult};
use pipeline::RunnerState;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

#[derive(Debug)]
struct ApplicationInstanceLock {
    _lock_file: File,
}

fn acquire_application_instance_lock() -> AppResult<ApplicationInstanceLock> {
    let lock_path = config::app_config_dir()?.join("application.lock");
    acquire_application_instance_lock_at(&lock_path)
}

fn acquire_application_instance_lock_at(lock_path: &Path) -> AppResult<ApplicationInstanceLock> {
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock_file).map_err(|error| {
        AppError::message(format!(
            "video-tool 已在运行，无法获取应用锁 {}: {error}",
            lock_path.display()
        ))
    })?;
    Ok(ApplicationInstanceLock {
        _lock_file: lock_file,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let application_instance_lock = match acquire_application_instance_lock() {
        Ok(instance_lock) => instance_lock,
        Err(error) => {
            eprintln!("failed to acquire application instance lock: {error}");
            return;
        }
    };
    let app_config = AppConfig::load_or_init().unwrap_or_else(|error| {
        eprintln!("failed to load config, using defaults: {error}");
        AppConfig::default()
    });

    if let Err(error) = app_config.ensure_workspace() {
        eprintln!("failed to ensure workspace: {error}");
    }
    if let Err(error) = workspace::recover_interrupted_jobs(app_config.workspace_path()) {
        eprintln!("failed to recover interrupted jobs: {error}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(application_instance_lock)
        .manage(AppState {
            config: Mutex::new(app_config),
            operation_lock: Mutex::new(()),
            runner: Arc::new(RunnerState::default()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_config,
            commands::reload_config,
            commands::save_config,
            commands::list_jobs,
            commands::get_job,
            commands::delete_job,
            commands::create_download_job,
            commands::create_download_jobs_batch,
            commands::create_live_record_job,
            commands::create_import_job,
            commands::run_job,
            commands::retry_job_step,
            commands::retry_transcript_segment,
            commands::stop_recording,
            commands::select_job_segments,
            commands::update_job_title,
            commands::update_job_group,
            commands::update_job_media_save_mode,
            commands::update_job_pipeline,
            commands::export_job,
            commands::test_provider,
            commands::get_job_transcript,
            commands::get_job_summary,
            commands::get_job_summaries,
            commands::get_job_chapters,
            commands::get_transcript_segment_texts,
            commands::get_transcript_cues,
            commands::save_transcript_edit,
            commands::get_job_media_overview,
            commands::generate_media_preview,
            commands::get_workspace_usage,
            commands::purge_job_media,
            commands::search_workspace,
            commands::rebuild_search_index,
            commands::get_job_log,
            commands::open_job_directory,
            commands::probe_sidecars,
            commands::get_dependency_report,
            commands::list_transcribe_models,
            commands::open_transcribe_model_directory,
            commands::export_app_config,
            commands::import_app_config,
            commands::check_app_update,
            commands::install_app_update,
            commands::get_system_diagnostics,
            commands::check_yt_dlp_update,
            commands::inspect_workspace_health,
            commands::repair_workspace_health,
        ])
        .setup(|app| {
            let bundled_sidecar_root = app
                .path()
                .resource_dir()
                .ok()
                .or_else(|| std::env::current_exe().ok()?.parent().map(PathBuf::from));
            app.state::<AppState>()
                .runner
                .set_bundled_sidecar_root(bundled_sidecar_root);

            // Media preview reads workspace files through the asset protocol;
            // scope is granted at runtime so a configurable workspace works.
            let workspace_path = app
                .state::<AppState>()
                .config
                .lock()
                .expect("config lock")
                .workspace_path();
            if let Err(error) = app
                .asset_protocol_scope()
                .allow_directory(&workspace_path, true)
            {
                eprintln!("failed to allow asset scope for workspace: {error}");
            }

            let mut tray_builder = TrayIconBuilder::new()
                .tooltip("video-tool - 点击恢复窗口")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
                    ) {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            tray_builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state.runner.has_live_recording() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn rejects_a_second_application_instance_lock() {
        let test_directory =
            std::env::temp_dir().join(format!("video-tool-instance-lock-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&test_directory).expect("create lock test directory");
        let lock_path = test_directory.join("application.lock");

        let first_lock =
            acquire_application_instance_lock_at(&lock_path).expect("acquire first lock");
        let second_lock_error = acquire_application_instance_lock_at(&lock_path)
            .expect_err("second lock must be rejected");
        assert!(second_lock_error.to_string().contains("已在运行"));

        drop(first_lock);
        acquire_application_instance_lock_at(&lock_path)
            .expect("lock must be reusable after the first instance exits");
        std::fs::remove_dir_all(test_directory).expect("remove lock test directory");
    }
}
