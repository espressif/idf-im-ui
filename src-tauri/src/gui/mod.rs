use anyhow::Result;
#[cfg(target_os = "linux")]
use fork::{daemon, Fork};
use idf_im_lib::idf_config::IdfInstallation;
use idf_im_lib::version_manager::prepare_settings_for_fix_idf_installation;
use idf_im_lib::{
    add_path_to_path, ensure_path,
    settings::Settings,
    ProgressMessage,
};
use log::{debug, error, info};
use serde_json::{json, Value};
use tauri::http::version;
use tauri_plugin_store::StoreExt;
use std::process::Command;
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
    thread,
};
use tauri::{AppHandle, Manager}; // dep: fork = "0.1"
mod app_state;
mod ui;
pub mod commands;
pub mod utils;

use app_state::{AppState};
use ui::{send_message, ProgressBar};
use commands::{utils_commands::*, prequisites::*, installation::*, settings::*, idf_tools::*};

const EIM_VERSION: &str = env!("CARGO_PKG_VERSION");


// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("Greet called with name: {}", name);
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_app_info() -> Value {
    json!({
      "version": EIM_VERSION
    })
}

#[tauri::command]
fn get_system_arch() -> String {
    let arch = if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    arch.to_string()
}

#[tauri::command]
fn get_installed_versions() -> Vec<IdfInstallation>{
  // return vec![];
  match idf_im_lib::version_manager::get_esp_ide_config() {
    Ok(config) => {
      if config.idf_installed.is_empty() {
        debug!(
          "No versions found. Use eim install to install a new ESP-IDF version."
        );
        vec![]
      } else {
        config.idf_installed
      }
    }
    Err(err) => {
      debug!("No versions found. Use eim install to install a new ESP-IDF version.");
      debug!("Error: {}", err);
      vec![]
    }
  }
}

#[tauri::command]
fn scan_for_archives() -> Vec<String> {
  let mut archives = Vec::new();
  // archives.push("archive_5.5.zst".to_string());
  // Scan the file system for archive files
  for entry in fs::read_dir(".").unwrap() {
      let entry = entry.unwrap();
      let path = entry.path();

      if path.extension().map(|e| e == "zst").unwrap_or(false) {
          archives.push(path.to_str().unwrap().to_string());
      }
  }

  archives
}
// fn get_available_versions()

#[tauri::command]
fn rename_installation(id: String, new_name: String) {
  debug!("Renaming installation with id {} to {}", id, new_name);

  match idf_im_lib::version_manager::rename_idf_version(&id, new_name) {
    Ok(_) => {
        debug!("Successfully renamed installation {}", id);
    }
    Err(e) => {
      error!("Failed to rename installation: {}", e);
    }
  };
}
#[tauri::command]
fn remove_installation(id: String) {
  debug!("Removing installation with id {}", id);

  match idf_im_lib::version_manager::remove_single_idf_version(&id, false) {
    Ok(_) => {
        debug!("Successfully removed installation {}", id);
    }
    Err(e) => {
      error!("Failed to remove installation: {}", e);
    }
  };
}



#[tauri::command]
fn get_app_settings(app_handle: AppHandle) -> Value { // TODO: persist
  let config_dir = dirs::config_dir()
        .ok_or("Failed to get config directory").unwrap()
        .join("eim");

  let config_file = config_dir.join("eim.json");
  ensure_path(config_dir.to_str().unwrap()).unwrap();
  match app_handle.store_builder(config_file).build() {
        Ok(store) => {
            let first_run = store.get("first_run")
                .unwrap_or(Value::Bool(true))
                .clone();
            let skip_welcome = store.get("skip_welcome")
                .unwrap_or(Value::Bool(false))
                .clone();

            json!({
                "first_run": first_run,
                "skip_welcome": skip_welcome
            })
        }
        Err(_) => {
            // If store doesn't exist or can't be loaded, return defaults
            json!({
                "first_run": true,
                "skip_welcome": false
            })
        }
    }
}

#[tauri::command]
async fn save_app_settings(app_handle: AppHandle, firstRun: bool, skipWelcome: bool) {
  let config_dir = dirs::config_dir()
        .ok_or("Failed to get config directory").unwrap()
        .join("eim");

  let config_file = config_dir.join("eim.json");
  ensure_path(config_dir.to_str().unwrap()).unwrap();

  match app_handle.store_builder(config_file).build() {
        Ok(store) => {
            store.set("first_run".to_string(), Value::Bool(firstRun));

            store.set("skip_welcome".to_string(), Value::Bool(skipWelcome));

            match store.save() {
              Ok(_) => {
                  log::info!("App settings saved: first_run={}, skip_welcome={}", firstRun, skipWelcome);
              }
              Err(e) => {
                  error!("Failed to save store: {}", e);
              }
            }
        }
        Err(e) => {
            error!("Failed to create/access store: {}", e);
        }
    }
}

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

    match idf_im_lib::get_esp_idf(
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

// Helper function to check if a process is running on Windows
#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> bool {
    use std::process::Command;

    // Check if process exists using tasklist
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output();

    match output {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str.contains(&pid.to_string())
        },
        Err(_) => false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // this is here because macos bundled .app does not inherit path
    #[cfg(target_os = "macos")]
    {
        env::set_var("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/local/bin:/opt/local/sbin");
    }
    let log_dir = idf_im_lib::get_log_directory().unwrap_or_else(|| {
        error!("Failed to get log directory.");
        PathBuf::from("")
    });
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    // this actually can not keep pace with the console, so maybe we should disable it for production build
                    tauri_plugin_log::TargetKind::Webview,
                ))
                // Add new file target with path configuration
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: log_dir,
                        file_name: Some("eim_gui_log".to_string()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .level_for("idf_im_lib", log::LevelFilter::Info)
                .level_for("eim_lib", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_state = AppState::default();
            app.manage(app_state);
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet,
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
            get_idf_mirror_list,
            set_idf_mirror,
            get_tools_mirror_list,
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
            cpu_count, //move these to proper submodules
            get_app_info,
            get_system_arch,
            get_installed_versions,
            scan_for_archives,
            check_prerequisites_detailed,
            rename_installation,
            remove_installation,
            fix_installation,
            get_app_settings,
            save_app_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
