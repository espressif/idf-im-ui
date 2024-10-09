use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::Read;
use std::sync::Mutex;

#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    wizard_data: Mutex<WizardData>,
    settings: Mutex<idf_im_lib::settings::Settings>,
}

#[derive(Default, Serialize, Deserialize)]
struct WizardData {
    // Add fields relevant to your installation process
    step_completed: Vec<bool>,
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    println!("Greet called with name: {}", name);
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_settings(app_handle: tauri::AppHandle) -> idf_im_lib::settings::Settings {
    let app_state = app_handle.state::<AppState>();
    let settings = app_state.settings.lock().expect("Failed to lock settings");
    (*settings).clone()
}

#[tauri::command]
fn get_prequisites() -> Vec<&'static str> {
    println!("Get prerequisites called"); // todo remove debug statement
    idf_im_lib::system_dependencies::get_prequisites()
}

#[tauri::command]
fn get_operating_system() -> String {
    println!("Get operating system called"); // todo remove debug statement
    std::env::consts::OS.to_string()
}

#[tauri::command]
fn install_prerequisites(app_handle: AppHandle) -> bool {
    println!("Install prerequisites called"); // todo remove debug statement
    let unsatisfied_prerequisites = idf_im_lib::system_dependencies::check_prerequisites()
        .unwrap()
        .into_iter()
        .map(|p| p.to_string())
        .collect();
    match idf_im_lib::system_dependencies::install_prerequisites(unsatisfied_prerequisites) {
        Ok(_) => true,
        Err(err) => {
            send_message(
                app_handle.clone(),
                format!("Error installing prerequisites: {}", err),
                "error".to_string(),
            );
            eprintln!("Error installing prerequisites: {}", err);
            false
        }
    }
}

#[tauri::command]
fn check_prequisites(app_handle: AppHandle) -> Vec<String> {
    match idf_im_lib::system_dependencies::check_prerequisites() {
        Ok(prerequisites) => {
            if prerequisites.is_empty() {
                // debug!("{}", t!("prerequisites.ok"));
                vec![]
            } else {
                // info!("{} {:?}", t!("prerequisites.missing"), prerequisites);
                prerequisites.into_iter().map(|p| p.to_string()).collect()
            }
        }
        Err(err) => {
            send_message(
                app_handle.clone(),
                format!("Error checking prerequisites: {}", err),
                "error".to_string(),
            );
            eprintln!("Error checking prerequisites: {}", err); //TODO: emit message
            vec![]
        }
    }
}

#[tauri::command]
fn python_sanity_check(app_handle: AppHandle, python: Option<&str>) -> bool {
    let outpusts = idf_im_lib::python_utils::python_sanity_check(python);
    let mut all_ok = true;
    for output in outpusts {
        match output {
            Ok(_) => {}
            Err(err) => {
                all_ok = false;
                send_message(
                    app_handle.clone(),
                    format!("Python sanity check failed: {}", err),
                    "warning".to_string(),
                );
                println!("{:?}", err)
            }
        }
    }
    all_ok
}

#[tauri::command]
fn python_install(app_handle: AppHandle) -> bool {
    match idf_im_lib::system_dependencies::install_prerequisites(vec!["python".to_string()]) {
        Ok(_) => true,
        Err(err) => {
            send_message(
                app_handle.clone(),
                format!("Error installing python: {}", err),
                "error".to_string(),
            );
            eprintln!("Error installing python: {}", err); //TODO: emit message
            false
        }
    }
}
#[tauri::command]
async fn get_available_targets() -> Vec<String> {
    let mut available_targets = idf_im_lib::idf_versions::get_avalible_targets()
        .await
        .unwrap();
    available_targets.insert(0, "all".to_string());
    available_targets
}

#[tauri::command]
fn set_targets(app_handle: AppHandle, targets: Vec<String>) {
    println!("set_targets called: {:?}", targets); //todo: switch to debug!
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state
        .settings
        .lock()
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                "Failed to obtain lock on settings".to_string(),
                "error".to_string(),
            )
        })
        .expect("Failed to lock settings");
    (*settings).target = Some(targets);
    println!("Setting after targets: {:?}", settings); //todo: switch to debug!
}

#[tauri::command]
async fn get_idf_versions(app_handle: AppHandle) -> Vec<String> {
    let app_state = app_handle.state::<AppState>();
    // Clone the settings to avoid holding the MutexGuard across await points
    let settings = {
        let guard = app_state
            .settings
            .lock()
            .map_err(|_| {
                send_message(
                    app_handle.clone(),
                    "Failed to obtain lock on settings".to_string(),
                    "error".to_string(),
                )
            })
            .expect("Failed to lock settings");
        guard.clone()
    };
    let targets = settings
        .target
        .clone()
        .unwrap_or_else(|| vec!["all".to_string()]);
    let targets_vec: Vec<String> = targets.iter().cloned().collect();
    let mut available_versions = if targets_vec.contains(&"all".to_string()) {
        idf_im_lib::idf_versions::get_idf_names().await
    } else {
        // todo: handle multiple targets
        idf_im_lib::idf_versions::get_idf_name_by_target(&targets[0].to_string().to_lowercase())
            .await
    };
    available_versions.push("master".to_string());
    available_versions
}

#[tauri::command]
fn set_versions(app_handle: AppHandle, versions: Vec<String>) {
    println!("set_versions called: {:?}", versions); //todo: switch to debug!
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state
        .settings
        .lock()
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                "Failed to obtain lock on settings".to_string(),
                "error".to_string(),
            )
        })
        .expect("Failed to lock settings");
    (*settings).idf_versions = Some(versions);
    println!("Setting after versions: {:?}", settings); //todo: switch to debug!
}

#[tauri::command]
fn get_idf_mirror_list() -> &'static [&'static str] {
    idf_im_lib::get_idf_mirrors_list()
}

#[tauri::command]
fn set_idf_mirror(app_handle: AppHandle, mirror: &str) {
    println!("set_idf_mirror called: {}", mirror); //todo: switch to debug!
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state
        .settings
        .lock()
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                "Failed to obtain lock on settings".to_string(),
                "error".to_string(),
            )
        })
        .expect("Failed to lock settings");
    (*settings).idf_mirror = Some(mirror.to_string());
    println!("Setting after idf_mirror: {:?}", settings); //todo: switch to debug!
}

#[tauri::command]
fn get_tools_mirror_list() -> &'static [&'static str] {
    idf_im_lib::get_idf_tools_mirrors_list()
}
#[tauri::command]
fn set_tools_mirror(app_handle: AppHandle, mirror: &str) {
    println!("set_tools_mirror called: {}", mirror); //todo: switch to debug!
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state
        .settings
        .lock()
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                "Failed to obtain lock on settings".to_string(),
                "error".to_string(),
            )
        })
        .expect("Failed to lock settings");
    (*settings).mirror = Some(mirror.to_string());
    println!("Setting after tools_mirror: {:?}", settings); //todo: switch to debug!
}

#[tauri::command]
fn load_settings(app_handle: AppHandle, path: &str) {
    let mut file = File::open(path)
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                format!("Failed to open file: {}", path),
                "warning".to_string(),
            )
        })
        .expect("Failed to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                format!("Failed to read file: {}", path),
                "warning".to_string(),
            )
        })
        .expect("Failed to read file");
    let settings: idf_im_lib::settings::Settings = toml::from_str(&contents)
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                format!("Failed to parse TOML: {}", path),
                "warning".to_string(),
            )
        })
        .expect("Failed to parse TOML");
    println!("settings {:?}", settings); // TODO: remove debug print statement
                                         // TODO: load setting to the app state
    let app_state = app_handle.state::<AppState>();
    let mut state_settings = app_state
        .settings
        .lock()
        .map_err(|_| {
            send_message(
                app_handle.clone(),
                "Failed to obtain lock on settings".to_string(),
                "error".to_string(),
            )
        })
        .expect("Failed to lock settings");
    *state_settings = settings;

    send_message(
        app_handle.clone(),
        format!("Settings loaded from {}", path),
        "info".to_string(),
    );
}

fn send_message(app_handle: AppHandle, message: String, message_type: String) {
    let _ = app_handle.emit(
        "user-message",
        json!({
          "type": message_type,
          "message": message
        }),
    );
}

use tauri::Manager;
use tauri::{AppHandle, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            load_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
