use gui::ui::send_message;
use idf_im_lib;
use log::{error, info};
use std::{
    path::PathBuf,
    process::Command,
};
use tauri::AppHandle;
use num_cpus;

use crate::gui;
use crate::gui::utils::is_path_empty_or_nonexistent;

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
