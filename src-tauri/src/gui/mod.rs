#[cfg(target_os = "linux")]
use fork::{daemon, Fork};
use idf_im_lib::telemetry::track_event;
use idf_im_lib::{
    add_path_to_path, ensure_path,
    settings::Settings,
    ProgressMessage,
};
use log::{error, info};
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

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("Greet called with name: {}", name);
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn track_event_command(name: &str) -> Result<(), String> {
    log::debug!("Track event called with name: {}", name);
    track_event("GUI event", serde_json::json!({"event_name": name})).await;
    Ok(())
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

// async fn install_single_version(
//     app_handle: AppHandle,
//     settings: &Settings,
//     version: String,
// ) -> Result<(), Box<dyn std::error::Error>> {
//     info!("Installing IDF version: {}", version);

//     let version_path = prepare_installation_directories(app_handle.clone(), settings, &version)?;
//     let idf_path = version_path.clone().join("esp-idf");
//     download_idf(&app_handle, settings, &version, &idf_path).await?;
//     let export_vars = setup_tools(&app_handle, settings, &idf_path, &version).await?;
//     let tools_install_path = version_path.clone().join(
//         settings
//             .tool_install_folder_name
//             .clone()
//             .unwrap_or_default(),
//     );
//     let idf_python_env_path = tools_install_path.clone().join("python").join(&version).join("venv");
//     let activation_script_path = settings.esp_idf_json_path.clone().unwrap_or_default();
//     idf_im_lib::single_version_post_install(
//         &activation_script_path,
//         idf_path.to_str().unwrap(),
//         &version,
//         tools_install_path.to_str().unwrap(),
//         export_vars,
//         Some(idf_python_env_path.to_str().unwrap()),
//     );

//     Ok(())
// }

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
            cpu_count,
            track_event_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
