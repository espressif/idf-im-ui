use idf_im_lib::{self, ensure_path};
use idf_im_lib::telemetry::{
    self, get_linux_os_name, InstallMode, InstallOutcome, Interface,
};
use log::{error, info};
use serde_json::{json, Value};
use tauri_plugin_store::StoreExt;
use std::fs;
use std::{
    path::PathBuf,
    process::Command,
};
use tauri::{AppHandle, Manager};
use anyhow::{Result};
use sysinfo::System;

use crate::gui::app_state::AppState;
use crate::gui::ui::send_message;

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
    let mut sys = System::new_all();
    sys.refresh_all();

    // Get architecture from sysinfo
    System::cpu_arch()
}

/// Returns the operating system name
#[tauri::command]
pub fn get_operating_system() -> String {
    if std::env::consts::OS == "linux" {
        return get_linux_os_name();
    }

    let mut sys = System::new_all();
    sys.refresh_all();

    System::name()
        .filter(|s| !s.is_empty() && s != "Unknown")
        .unwrap_or_else(|| std::env::consts::OS.to_string())
}

#[tauri::command]
pub fn get_system_info() -> String {
  idf_im_lib::telemetry::get_system_info()
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
    let mut sys = System::new_all();
    sys.cpus().len()
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

/// Check if running with elevated permissions (Administrator on Windows, root on POSIX)
#[tauri::command]
pub fn check_elevated_permissions() -> Result<bool, String> {
    Ok(idf_im_lib::utils::is_running_elevated())
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
pub async fn track_event_command(
    app_handle: AppHandle,
    event: String,
    mode: String,
    versions: Option<Vec<String>>,
    installation_ids: Option<Vec<String>>,
    outcome: Option<String>,
    error_kind: Option<String>,
    error_message: Option<String>,
    feature_count: Option<usize>,
    tool_count: Option<usize>,
    target_count: Option<usize>,
    non_interactive: Option<bool>,
    used_existing_idf: Option<bool>,
) -> Result<(), String> {
    let app_settings = get_app_settings(app_handle.clone());
    let usage_statistics = match app_settings.get("usage_statistics") {
        Some(val) => val,
        None => {
            log::debug!("Usage statistics setting not found, skipping event tracking.");
            return Ok(());
        }
    };
    if usage_statistics != &Value::Bool(true) {
        log::debug!("Usage statistics not allowed, skipping event tracking.");
        return Ok(());
    }

    let install_mode = match mode.as_str() {
        "wizard" => InstallMode::Wizard,
        "simple" => InstallMode::Simple,
        "offline" => InstallMode::Offline,
        "fix" => InstallMode::Fix,
        other => return Err(format!("Unknown install mode: {}", other)),
    };

    let app_state = app_handle.state::<AppState>();

    match event.as_str() {
        "install_started" => {
            let ctx = telemetry::new_session(
                Interface::Gui,
                install_mode,
                versions.unwrap_or_default(),
                installation_ids.unwrap_or_default(),
            );
            telemetry::track_install_started(&ctx);
            let mut session = app_state
                .telemetry_session
                .lock()
                .map_err(|_| "Failed to lock telemetry session".to_string())?;
            *session = Some(ctx);
        }
        "install_finished" => {
            let ctx = {
                let mut session = app_state
                    .telemetry_session
                    .lock()
                    .map_err(|_| "Failed to lock telemetry session".to_string())?;
                session.take()
            };
            let Some(ctx) = ctx else {
                log::debug!("install_finished without a matching install_started; skipping.");
                return Ok(());
            };
            let outcome = match outcome.as_deref() {
                Some("success") => InstallOutcome::Success,
                Some("failure") | None => InstallOutcome::Failure,
                Some(other) => return Err(format!("Unknown outcome: {}", other)),
            };
            let kind = parse_error_kind(error_kind.as_deref())?;
            let error = match (outcome, kind, error_message.as_deref()) {
                (InstallOutcome::Failure, Some(k), Some(msg)) => Some(build_anyhow(k, msg)),
                _ => None,
            };
            let extras = telemetry::OutcomeExtras {
                feature_count,
                tool_count,
                target_count,
                non_interactive,
                used_existing_idf,
            };
            telemetry::track_install_outcome(&ctx, outcome, kind, error.as_ref(), extras);
        }
        other => return Err(format!("Unknown event: {}", other)),
    }

    Ok(())
}

fn parse_error_kind(s: Option<&str>) -> Result<Option<idf_im_lib::telemetry::ErrorKind>, String> {
    use idf_im_lib::telemetry::ErrorKind;
    Ok(match s {
        None => None,
        Some("network") => Some(ErrorKind::Network),
        Some("filesystem") => Some(ErrorKind::Filesystem),
        Some("git") => Some(ErrorKind::Git),
        Some("python") => Some(ErrorKind::Python),
        Some("dependency_missing") => Some(ErrorKind::DependencyMissing),
        Some("user_cancelled") => Some(ErrorKind::UserCancelled),
        Some("configuration") => Some(ErrorKind::Configuration),
        Some("unknown") => Some(ErrorKind::Unknown),
        Some(other) => return Err(format!("Unknown error kind: {}", other)),
    })
}

fn build_anyhow(kind: idf_im_lib::telemetry::ErrorKind, message: &str) -> anyhow::Error {
    anyhow::anyhow!("{:?}: {}", kind, message)
}



#[tauri::command]
pub fn open_terminal_with_script(script_path: String) -> Result<bool,String> {
    #[cfg(target_os = "windows")]
    {
        if !std::path::Path::new(&script_path).exists() {
            return Err(format!("Script not found: {}", script_path));
        }

        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-NoExit",
            "-File", &script_path,
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

/// Writes content to a text file at the given path
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, &content)
        .map_err(|e| format!("Failed to write to file {}: {}", path, e))?;
    info!("Successfully wrote to file: {}", path);
    Ok(())
}
