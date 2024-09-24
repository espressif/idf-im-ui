use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    wizard_data: Mutex<WizardData>,
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
fn get_settings() -> idf_im_lib::settings::Settings {
    println!("Get settings called");
    idf_im_lib::settings::Settings::new(None, None).unwrap()
}

#[tauri::command]
fn get_prequisites() -> Vec<&'static str> {
    println!("Get prerequisites called");
    idf_im_lib::system_dependencies::get_prequisites()
}

#[tauri::command]
fn check_prequisites() -> Vec<String> {
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
            eprintln!("Error checking prerequisites: {}", err); //TODO: emit message
            vec![]
        }
    }
}

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            get_prequisites
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
