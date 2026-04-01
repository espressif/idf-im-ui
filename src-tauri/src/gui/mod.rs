use anyhow::Result;
use fern::Dispatch;
#[cfg(target_os = "linux")]
use fork::{daemon, Fork};
use idf_im_lib::get_log_directory;
use idf_im_lib::logging::formatter;
use idf_im_lib::{
    add_path_to_path, ensure_path,
    logging,
    settings::Settings,
};
use log::{LevelFilter, debug, error, info};
use std::process::Command;
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
};
use tauri::{AppHandle, Manager}; // dep: fork = "0.1"
mod app_state;
mod ui;
pub mod commands;
pub mod utils;

use app_state::{AppState};
use ui::{send_message, ProgressBar};
use commands::{utils_commands::*, prequisites::*, installation::*, settings::*, idf_tools::*, version_management::*};
use tauri_plugin_store::StoreExt;
use serde_json::Value;

fn prepare_installation_directories(
    app_handle: AppHandle,
    settings: &Settings,
    version: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let version_path = settings.path.as_ref().unwrap().as_path().join(version);

    ensure_path(version_path.to_str().unwrap())?;
    send_message(
        &app_handle,
        format!(
            "IDF installation folder created at: {}",
            version_path.display()
        ),
        "info".to_string(),
    );

    Ok(version_path)
}

async fn download_idf(
    app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    idf_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    let progress = ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", version));

    let handle = spawn_progress_monitor(app_handle.clone(), version.to_string(), rx);

    match idf_im_lib::git_tools::get_esp_idf(
      idf_path.to_str().unwrap(),
      settings.repo_stub.as_deref(),
      version,
      settings.idf_mirror.as_deref(),
      settings.recurse_submodules.unwrap_or_default(),
      tx,
    ) {
        Ok(_) => {
          send_message(
            app_handle,
            format!(
                "IDF {} installed successfully at: {}",
                version,
                idf_path.display()
            ),
            "info".to_string(),
          );
          progress.finish();
        }
        Err(e) => {
          send_message(
            app_handle,
            format!("Failed to install IDF {}. Reason: {}", version, e),
            "error".to_string(),
        );
        progress.finish();
        return Err(e.into());
        }
    }

    handle.join().unwrap();
    Ok(())
}

// Tool installation types
#[derive(Debug)]
struct ToolSetup {
    download_dir: String,
    install_dir: String,
    tools_json_path: String,
}

impl ToolSetup {
    fn new(settings: &Settings, version_path: &PathBuf) -> Result<Self, String> {
        let p = version_path;
        let tools_json_path = p
            .join("esp-idf")
            .join(settings.tools_json_file.clone().unwrap_or_default());
        let download_dir = p.join(
            settings
                .tool_download_folder_name
                .clone()
                .unwrap_or_default(),
        );
        let install_dir = p.join(
            settings
                .tool_install_folder_name
                .clone()
                .unwrap_or_default(),
        );
        Ok(Self {
            download_dir: download_dir.to_str().unwrap().to_string(),
            install_dir: install_dir.to_str().unwrap().to_string(),
            tools_json_path: tools_json_path.to_str().unwrap().to_string(),
        })
    }

    fn create_directories(&self, app_handle: &AppHandle) -> Result<(), String> {
        // Create download directory
        ensure_path(&self.download_dir).map_err(|e| {
            send_message(
                app_handle,
                format!("Failed to create download directory: {}", e),
                "error".to_string(),
            );
            e.to_string()
        })?;

        // Create installation directory
        ensure_path(&self.install_dir).map_err(|e| {
            send_message(
                app_handle,
                format!("Failed to create installation directory: {}", e),
                "error".to_string(),
            );
            e.to_string()
        })?;

        // Add installation directory to PATH
        add_path_to_path(&self.install_dir);

        Ok(())
    }

    fn validate_tools_json(&self) -> Result<(), String> {
        if fs::metadata(&self.tools_json_path).is_err() {
            return Err(format!(
                "tools.json file not found at: {}",
                self.tools_json_path
            ));
        }
        Ok(())
    }
}

/// Setup logging for the GUI application.
///
/// # Arguments
/// * `log_level_override` - Optional log level override (uses Info if None)
///
/// # Log Level Behavior
/// - File: Always Trace level (all logs)
/// - Console: Info level in debug builds, no console in production
pub fn setup_gui_logging(
    log_level_override: Option<LevelFilter>,
) -> Result<(), fern::InitError> {
    let console_level = log_level_override.unwrap_or(LevelFilter::Info);
    let log_dir = get_log_directory().unwrap_or_else(|| PathBuf::from("logs"));

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        log::error!("Failed to create log directory {}: {}", log_dir.display(), e);
    }

    let log_file_path = log_dir.join("eim_gui.log");

    // Build dispatch with file chain (always Trace) and console chain (debug only)
    let mut dispatch = Dispatch::new()
        .format(formatter)
        // File at Trace level
        .chain(
            Dispatch::new()
                .level(LevelFilter::Trace)
                .chain(fern::log_file(&log_file_path)?)
        );

    // Add console in debug builds
    #[cfg(debug_assertions)]
    {
        dispatch = dispatch.chain(
            Dispatch::new()
                .level(console_level)
                .chain(std::io::stdout())
        );
    }

    dispatch.apply()?;

    log::info!("GUI logging initialized. File: {:?}, Console: {:?}", log_file_path, console_level);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(settings: Option<Settings>, log_level_override: Option<log::LevelFilter>, do_not_track: bool) {
    // this is here because macos bundled .app does not inherit path
    #[cfg(target_os = "macos")]
    {
        env::set_var("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/local/bin:/opt/local/sbin");
    }

    let _ = setup_gui_logging(log_level_override);

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
          app.manage(app_state);

          // Set usage_statistics based on do_not_track
          let config_dir = dirs::config_dir()
            .ok_or("Failed to get config directory")?
            .join("eim");
          let config_file = config_dir.join("eim.json");
          if let Err(e) = std::fs::create_dir_all(&config_dir) {
            log::error!("Failed to create config directory: {}", e);
          } else if let Ok(store) = app.handle().store_builder(config_file).build() {
            // If do_not_track is true, set usage_statistics to false
            store.set("usage_statistics".to_string(), Value::Bool(!do_not_track_value));
            if let Err(e) = store.save() {
                log::error!("Failed to save usage_statistics setting: {}", e);
            }
          }

          Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            check_prequisites,
            install_prerequisites,
            get_prequisites,
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
            get_app_settings,
            save_app_settings,
            start_offline_installation,
            check_elevated_permissions,
            install_drivers,
            get_system_info,
            cpu_count,
            track_event_command,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
