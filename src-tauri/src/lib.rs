use std::sync::Mutex;
use serde::{Serialize, Deserialize};

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
    format!("Hello, {}! You've been greeted from Rust!", name)
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
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
