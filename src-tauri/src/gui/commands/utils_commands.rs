use gui::ui::send_message;
use idf_im_lib::{self, ensure_path};
use log::{error, info};
use serde_json::{json,Value};
use tauri_plugin_store::StoreExt;
use std::fs;
use std::{
    path::PathBuf,
    process::Command,
};
use tauri::AppHandle;
use num_cpus;

use crate::gui;
use crate::gui::utils::is_path_empty_or_nonexistent;

const EIM_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tauri::command]
pub fn get_app_info() -> Value {
    json!({
      "version": EIM_VERSION
    })
}

#[tauri::command]
pub fn get_system_arch() -> String {
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

/// Returns the operating system name
#[tauri::command]
pub fn get_operating_system() -> String {
    std::env::consts::OS.to_string()
}

/// Gets the logs folder path
#[tauri::command]
pub fn get_logs_folder(app_handle: AppHandle) -> PathBuf {
    match idf_im_lib::get_log_directory() {
        Some(folder) => folder,
        None => {
            send_message(
                &app_handle,
                "Error getting log folder".to_string(),
                "error".to_string(),
            );
            error!("Error getting log folder");
            PathBuf::new()
        }
    }
}

/// Shows a file or folder in the system file explorer
#[tauri::command]
pub fn show_in_folder(path: String) {
    #[cfg(target_os = "windows")]
    {
        match Command::new("explorer")
            .args(["/select,", &path]) // The comma after select is not a typo
            .spawn()
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to open folder with explorer: {}", e);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let path = if path.contains(",") {
            // see https://gitlab.freedesktop.org/dbus/dbus/-/issues/76
            match std::fs::metadata(&path).unwrap().is_dir() {
                true => path,
                false => {
                    let mut path2 = PathBuf::from(path);
                    path2.pop();
                    path2.into_os_string().into_string().unwrap()
                }
            }
        } else {
            path
        };

        // Try using xdg-open first
        if Command::new("xdg-open").arg(&path).spawn().is_err() {
            // Fallback to dbus-send if xdg-open fails
            let uri = format!("file://{}", path);
            match Command::new("dbus-send")
                .args([
                    "--session",
                    "--dest=org.freedesktop.FileManager1",
                    "--type=method_call",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                    format!("array:string:\"{}\"", uri).as_str(),
                    "string:\"\"",
                ])
                .spawn()
            {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to open file with dbus-send: {}", e);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        match Command::new("open").args(["-R", &path]).spawn() {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to open file with open: {}", e);
            }
        }
    }
}

/// Quits the application
#[tauri::command]
pub fn quit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
pub fn cpu_count() -> usize {
    num_cpus::get()
}

#[tauri::command]
pub fn scan_for_archives() -> Vec<String> {
  let mut archives = Vec::new();
  for entry in fs::read_dir(".").unwrap() {
      let entry = entry.unwrap();
      let path = entry.path();

      if path.extension().map(|e| e == "zst").unwrap_or(false) {
          archives.push(path.to_str().unwrap().to_string());
      }
  }

  archives
}

#[tauri::command]
pub fn get_app_settings(app_handle: AppHandle) -> Value { // TODO: persist
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
pub async fn save_app_settings(app_handle: AppHandle, firstRun: bool, skipWelcome: bool) {
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

#[tauri::command]
pub async fn install_drivers() -> Result<(), Box<dyn std::error::Error>> {
    // Implementation for installing drivers
    match std::env::consts::OS {
      "windows" => {
        info!("Installing drivers...");
        match idf_im_lib::install_drivers().await {
          Ok(_) => {
            info!("Drivers installed successfully.");
          }
          Err(err) => {
            error!("Failed to install drivers: {}", err);
            return Err(format!("Failed to install drivers: {}", err).into());
          }
        }
        Ok(())
      }
      _ => {
        return Err(format!("Driver installation is only supported on Windows.").into());
      }
    }
}
