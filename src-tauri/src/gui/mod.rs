use anyhow::Result;
use fern::Dispatch;
use idf_im_lib::get_log_directory;
use idf_im_lib::logging::formatter;
use idf_im_lib::settings::Settings;
use log::LevelFilter;
use std::{env, path::PathBuf, sync::Mutex};
use tauri::Manager;
mod app_state;
pub mod commands;
pub mod telemetry_session;
mod ui;
pub mod utils;

// Only used inside the Linux-only Nvidia/GBM workaround below. The imports must
// carry the same `cfg` as the code that uses them, otherwise a macOS-only
// `cargo clippy --fix` strips them as unused and the Linux build stops compiling.
#[cfg(target_os = "linux")]
use log::info;
#[cfg(target_os = "linux")]
use std::path::Path;

use app_state::AppState;
use commands::{
    idf_tools::*, installation::*, prerequisites::*, settings::*, utils_commands::*,
    version_management::*,
};
use serde_json::Value;
use tauri_plugin_store::StoreExt;

/// Setup logging for the GUI application.
///
/// # Arguments
/// * `log_level_override` - Optional log level override (uses Info if None)
///
/// # Log Level Behavior
/// - File: Always Trace level (all logs)
/// - Console: Info level in debug builds, no console in production
pub fn setup_gui_logging(log_level_override: Option<LevelFilter>) -> Result<(), fern::InitError> {
    let console_level = log_level_override.unwrap_or(LevelFilter::Info);
    let log_dir = get_log_directory().unwrap_or_else(|| PathBuf::from("logs"));

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        log::error!(
            "Failed to create log directory {}: {}",
            log_dir.display(),
            e
        );
    }

    let log_file_path = log_dir.join("eim_gui.log");

    // Build dispatch with file chain (always Trace) and console chain (debug only)
    let dispatch = Dispatch::new()
        .format(formatter)
        // File at Trace level
        .chain(
            Dispatch::new()
                .level(LevelFilter::Trace)
                .chain(fern::log_file(&log_file_path)?),
        );

    // Add console in debug builds. Shadowing instead of `let mut` keeps release
    // builds free of an `unused_mut` warning, since this arm is compiled out.
    #[cfg(debug_assertions)]
    let dispatch = dispatch.chain(
        Dispatch::new()
            .level(console_level)
            .chain(std::io::stderr()),
    );

    dispatch.apply()?;

    log::info!(
        "GUI logging initialized. File: {:?}, Console: {:?}",
        log_file_path,
        console_level
    );
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(
    settings: Option<Settings>,
    log_level_override: Option<log::LevelFilter>,
    do_not_track: bool,
) {
    // this is here because macos bundled .app does not inherit path
    #[cfg(target_os = "macos")]
    {
        env::set_var("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/local/bin:/opt/local/sbin");
    }

    let _ = setup_gui_logging(log_level_override);

    idf_im_lib::telemetry::set_enabled(!do_not_track);

    // Workaround for WebKitGTK DMA-BUF renderer issues on Nvidia (#421, #523).
    // GBM buffer allocation fails when using the DMA-BUF renderer with
    // Nvidia's proprietary driver, causing a blank window or crash.
    // This affects both Wayland and X11 sessions.
    #[cfg(target_os = "linux")]
    {
        if env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            let has_nvidia = Path::new("/proc/driver/nvidia/version").exists();

            if has_nvidia {
                env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
                info!("Nvidia detected: disabled WebKitGTK DMA-BUF renderer (WEBKIT_DISABLE_DMABUF_RENDERER=1)");
            }
        }
    }

    // Create owned copies before the closure to avoid borrow issues
    let do_not_track_value = do_not_track;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(move |app| {
          let app_state = match settings {
            Some(s) => AppState {
              settings: Mutex::new(s),
              ..Default::default()
            },
            None => AppState::default()
          };
          // Initialize eim_idf.json at the configured path (not hardcoded default)
          match app_state.settings.lock() {
            Ok(settings) => {
              match settings.initialize_esp_ide_json() {
                Ok(_) => log::debug!("ESP-IDF JSON initialized at configured path."),
                Err(e) => log::warn!("Failed to initialize ESP-IDF JSON: {}. IDE integration may not work correctly.", e),
              }
            }
            Err(e) => log::warn!("Failed to lock settings: {}", e),
          }
          app.manage(app_state);
          // Set usage_statistics based on do_not_track
          let config_dir = dirs::config_dir()
            .ok_or("Failed to get config directory")?
            .join("eim");
          let config_file = config_dir.join("eim.json");
          if let Err(e) = std::fs::create_dir_all(&config_dir) {
            log::error!("Failed to create config directory: {}", e);
          } else if let Ok(store) = app.handle().store_builder(config_file).build() {
            // Only an explicit --do-not-track overrides the user's saved preference
            if do_not_track_value {
              store.set("usage_statistics".to_string(), Value::Bool(false));
              if let Err(e) = store.save() {
                  log::error!("Failed to save usage_statistics setting: {}", e);
              }
            }
          }

          Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            check_prerequisites,
            install_prerequisites,
            get_prerequisites,
            get_operating_system,
            python_sanity_check,
            python_install,
            get_available_targets,
            set_targets,
            get_idf_versions,
            set_versions,
            get_idf_mirror_latency_entries,
            get_idf_mirror_urls,
            set_idf_mirror,
            get_tools_mirror_latency_entries,
            get_tools_mirror_urls,
            set_tools_mirror,
            load_settings,
            get_installation_path,
            set_installation_path,
            start_installation,
            is_installing,
            start_simple_setup,
            quit_app,
            save_config,
            get_logs_folder,
            show_in_folder,
            is_path_empty_or_nonexistent_command,
            is_path_idf_directory,
            get_app_info,
            get_system_arch,
            get_installed_versions,
            scan_for_archives,
            check_prerequisites_detailed,
            rename_installation,
            remove_installation,
            purge_all_installations,
            fix_installation,
            check_incomplete_installations,
            get_app_settings,
            save_app_settings,
            start_offline_installation,
            check_elevated_permissions,
            install_drivers,
            get_system_info,
            cpu_count,
            set_locale,
            open_terminal_with_script,
            get_pypi_mirror_latency_entries,
            get_pypi_mirror_urls,
            set_pypi_mirror,
            fetch_json_from_url,
            get_features_list_all_versions,
            set_selected_features_per_version,
            get_selected_features_per_version,
            reset_settings_to_default,
            get_tools_list_all_versions,
            set_selected_tools_per_version,
            get_selected_tools_per_version,
            generate_installation_config_for_version,
            list_idf_tools,
            list_idf_features,
            write_text_file,
            get_tool_download_folder_name,
            set_tool_download_folder_name,
            get_tool_install_folder_name,
            set_tool_install_folder_name,
            get_cleanup,
            set_cleanup,
            get_offline_archives,
            get_available_drives,
            start_simple_offline_setup,
            delete_offline_archive,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // ExitRequested is only a request: another handler can call
            // prevent_exit(). Recording a failure there would leave a false
            // app_closed outcome if the user then stays. Exit fires only after
            // nobody cancelled, so that is the safe place to abandon and flush.
            if let tauri::RunEvent::Exit = event {
                let abandoned = telemetry_session::abandon_open_session(app_handle);
                if abandoned || idf_im_lib::telemetry::has_pending_dispatches() {
                    tauri::async_runtime::block_on(idf_im_lib::telemetry::flush(
                        std::time::Duration::from_millis(500),
                    ));
                }
            }
        });
}
