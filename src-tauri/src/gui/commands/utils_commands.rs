use gui::ui::send_message;
use idf_im_lib::{self, ensure_path};
use idf_im_lib::telemetry::track_event;
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
use anyhow::{Result};

use crate::gui;
use crate::gui::utils::is_path_empty_or_nonexistent;

#[cfg(windows)]
use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
#[cfg(windows)]
use winapi::um::securitybaseapi::GetTokenInformation;
#[cfg(windows)]
use winapi::um::winnt::{TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
#[cfg(windows)]
use winapi::um::handleapi::CloseHandle;
#[cfg(windows)]
use winapi::shared::minwindef::{DWORD, FALSE};
#[cfg(windows)]
use std::mem;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const EIM_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tauri::command]
pub async fn fetch_json_from_url(url: String) -> Result<Value, String> {
    let response = reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?;

    let json:Value = response
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())?;

    Ok(json)
}

#[tauri::command]
pub fn set_locale(locale: String) {
    rust_i18n::set_locale(&locale);
    info!("Set locale to: {}", locale);
}

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

#[tauri::command]
pub fn get_system_info() -> String {
    let info = os_info::get();
    format!("OS: {} {} | Architecture: {} | Kernel: {}",
        info.os_type(),
        info.version(),
        info.architecture().unwrap_or("unknown"),
        std::env::consts::ARCH
    )
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
pub fn scan_for_archives() -> Result<Vec<String>, String> {
    let binary = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get current executable path: {}", e);
            return Err("Failed to get current executable path".to_string());
        }
    };
    let scan_dir = binary
        .parent()
        .ok_or("Could not get parent directory")?;

    let mut archives = Vec::new();

    for entry in fs::read_dir(scan_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.extension().map(|e| e == "zst").unwrap_or(false) {
            archives.push(path.to_string_lossy().to_string());
        }
    }

    Ok(archives)
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

            let usage_statistics = store.get("usage_statistics")
                .unwrap_or(Value::Bool(false))
                .clone();

            json!({
                "first_run": first_run,
                "skip_welcome": skip_welcome,
                "usage_statistics": usage_statistics
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
pub async fn save_app_settings(app_handle: AppHandle, firstRun: bool, skipWelcome: bool, usageStatistics: bool) {
  let config_dir = dirs::config_dir()
        .ok_or("Failed to get config directory").unwrap()
        .join("eim");

  let config_file = config_dir.join("eim.json");
  ensure_path(config_dir.to_str().unwrap()).unwrap();

  match app_handle.store_builder(config_file).build() {
        Ok(store) => {
            store.set("first_run".to_string(), Value::Bool(firstRun));

            store.set("skip_welcome".to_string(), Value::Bool(skipWelcome));

            store.set("usage_statistics".to_string(), Value::Bool(usageStatistics));

            match store.save() {
              Ok(_) => {
                  log::info!("App settings saved: first_run={}, skip_welcome={}, usage_statistics={}", firstRun, skipWelcome, usageStatistics);
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

#[cfg(windows)]
fn is_elevated() -> Result<bool, Box<dyn std::error::Error>> {
    unsafe {
        let process = GetCurrentProcess();
        let mut token = std::ptr::null_mut();

        // Open process token
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == FALSE {
            return Err("Failed to open process token".into());
        }

        // Get token elevation information
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length: DWORD = 0;

        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            mem::size_of::<TOKEN_ELEVATION>() as DWORD,
            &mut return_length,
        );

        // Clean up token handle
        CloseHandle(token);

        if result == FALSE {
            return Err("Failed to get token information".into());
        }

        Ok(elevation.TokenIsElevated != 0)
    }
}

#[cfg(not(windows))]
fn is_elevated() -> Result<bool, Box<dyn std::error::Error>> {
    Ok(false)
}

#[tauri::command]
pub fn check_elevation() -> Result<bool, String> {
    match std::env::consts::OS {
      "windows" => {
        match is_elevated() {
          Ok(elevated) => Ok(elevated),
          Err(err) => {
            error!("Failed to check elevation: {}", err);
            Err(format!("Failed to check elevation: {}", err))
          }
        }
      }
      _ => {
        // On non-Windows systems, assume no elevation is needed
        Ok(false)
      }
    }
}


#[tauri::command]
pub async fn install_drivers() -> Result<(), String> {
    match std::env::consts::OS {
      "windows" => {
        info!("Installing drivers...");
        match idf_im_lib::install_drivers().await {
          Ok(_) => {
            info!("Drivers installed successfully.");
          }
          Err(err) => {
            error!("Failed to install drivers: {}", err);
            return Err(format!("Failed to install drivers: {}", err));
          }
        }
        Ok(())
      }
      _ => {
        return Err(format!("Driver installation is only supported on Windows."));
      }
    }
}

#[tauri::command]
pub async fn track_event_command(app_handle: AppHandle,name: &str, additional_data: Option<serde_json::Value>) -> Result<(), String> {
  let app_settings = get_app_settings(app_handle);
  let usage_statistics = match app_settings.get("usage_statistics"){
    Some(val) => val,
    None => {
      log::debug!("Usage statistics setting not found, skipping event tracking.");
      return Ok(()) // If the setting is not found, do not track
    }
  };
  if usage_statistics != &Value::Bool(true) {
    log::debug!("Usage statistics not allowed, skipping event tracking.");
    return Ok(());
  }
  let system_info = get_system_info();
  log::debug!("System info: {}", system_info);
  log::debug!("Track event called with name: {}", name);
  track_event("GUI event", serde_json::json!({
    "event_name": name,
    "system_info": system_info,
    "eim_version": EIM_VERSION,
    "additional_data": additional_data
  })).await;
  Ok(())
}



#[tauri::command]
pub fn open_terminal_with_script(script_path: String) -> Result<bool,String> {
    #[cfg(target_os = "windows")]
    {
        // Windows: Open PowerShell and dot-source the script
        let ps_command = format!(
            "$scriptPath = '{}'; if (Test-Path $scriptPath) {{ . $scriptPath }} else {{ Write-Host \"Script not found: $scriptPath\" }}",
            script_path.replace("\\", "\\\\").replace("'", "''")
        );

        let mut cmd = Command::new("powershell");
        cmd.args(&[
            "-NoExit",
            "-Command",
            &ps_command
        ]);

        #[cfg(windows)]
        unsafe {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x00000010); // CREATE_NEW_CONSOLE
        }

        cmd.spawn()
            .map_err(|e| format!("Failed to open PowerShell: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let escaped_script_path = script_path.replace("'", "'\"'\"'");

        // Create a temporary initialization script that sources your script in bash
        let shell_cmd = format!(
            "bash --rcfile <(cat ~/.bashrc 2>/dev/null; echo 'source \"{}\"' 2>/dev/null || echo 'echo Script not found: {}')",
            escaped_script_path, escaped_script_path
        );

        let applescript_escaped = shell_cmd
            .replace("\\", "\\\\")
            .replace("\"", "\\\"");

        let applescript = format!(
            "tell application \"Terminal\"\n\
             do script \"{}\"\n\
             activate\n\
             end tell",
            applescript_escaped
        );

        Command::new("osascript")
            .arg("-e")
            .arg(applescript)
            .spawn()
            .map_err(|e| format!("Failed to open Terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let escaped_script_path = script_path.replace("'", "'\"'\"'");

        // Use bash --rcfile to source the script as part of the interactive shell initialization
        let shell_cmd = format!(
            "bash --rcfile <(cat ~/.bashrc 2>/dev/null; echo 'source \"{}\"' 2>/dev/null || echo 'echo Script not found: {}')",
            escaped_script_path, escaped_script_path
        );

        let terminals = [
            ("gnome-terminal", vec!["--", "bash", "-c", &shell_cmd]),
            ("konsole", vec!["-e", "bash", "-c", &shell_cmd]),
            ("xfce4-terminal", vec!["-e", "bash", "-c", &shell_cmd]),
            ("xterm", vec!["-e", "bash", "-c", &shell_cmd]),
            ("alacritty", vec!["-e", "bash", "-c", &shell_cmd]),
            ("kitty", vec!["bash", "-c", &shell_cmd]),
        ];

        let mut success = false;
        for (terminal, args) in terminals.iter() {
            if Command::new(terminal)
                .args(args)
                .spawn()
                .is_ok()
            {
                success = true;
                break;
            }
        }

        if !success {
            return Err("No supported terminal found".to_string());
        }
    }

    Ok(true)
}
