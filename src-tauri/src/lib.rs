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
fn get_operating_system() -> String {
    println!("Get operating system called");
    std::env::consts::OS.to_string()
}

#[tauri::command]
fn install_prerequisites() -> bool {
    println!("Install prerequisites called");
    let unsatisfied_prerequisites = idf_im_lib::system_dependencies::check_prerequisites()
        .unwrap()
        .into_iter()
        .map(|p| p.to_string())
        .collect();
    match idf_im_lib::system_dependencies::install_prerequisites(unsatisfied_prerequisites) {
        Ok(_) => true,
        Err(err) => {
            eprintln!("Error installing prerequisites: {}", err); //TODO: emit message
            false
        }
    }
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

#[tauri::command]
fn python_sanity_check(python: Option<&str>) -> bool {
    let outpusts = idf_im_lib::python_utils::python_sanity_check(python);
    let mut all_ok = true;
    for output in outpusts {
        match output {
            Ok(_) => {}
            Err(err) => {
                all_ok = false;
                println!("{:?}", err)
            }
        }
    }
    all_ok
}

#[tauri::command]
fn python_install() -> bool {
    match idf_im_lib::system_dependencies::install_prerequisites(vec!["python".to_string()]) {
        Ok(_) => true,
        Err(err) => {
            eprintln!("Error installing prerequisites: {}", err); //TODO: emit message
            false
        }
    }
}
#[tauri::command]
async fn get_available_targets() -> Vec<String> {
    let mut available_targets = idf_im_lib::idf_versions::get_avalible_targets().await.unwrap();
    available_targets.insert(0, "all".to_string());
    available_targets
}

#[tauri::command]
async fn get_idf_versions() -> Vec<String> {
  let target = "all".to_string(); //todo: get from state or user
  let mut available_versions = if target == "all" {
      //todo process vector of targets
      idf_im_lib::idf_versions::get_idf_names().await
  } else {
      idf_im_lib::idf_versions::get_idf_name_by_target(&target.to_string().to_lowercase()).await
  };
  available_versions.push("master".to_string());
  available_versions
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
            install_prerequisites,
            get_prequisites,
            get_operating_system,
            python_sanity_check,
            python_install,
            get_available_targets,
            get_idf_versions
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
