use crate::gui::{app_state::get_settings_non_blocking, ui::send_message};
use idf_im_lib;
use log::{error, warn};
use tauri::AppHandle;
use serde_json::{json, Value};
use rust_i18n::t;


/// Gets the list of prerequisites for ESP-IDF
#[tauri::command]
pub fn get_prequisites() -> Vec<&'static str> {
   idf_im_lib::system_dependencies::get_prequisites()
    .into_iter()
    .chain(idf_im_lib::system_dependencies::get_general_prerequisites_based_on_package_manager())
    .collect()
}

/// Checks which prerequisites are missing
#[tauri::command]
pub fn check_prequisites(app_handle: AppHandle) -> Vec<String> {
    match idf_im_lib::system_dependencies::check_prerequisites_with_result() {
        Ok(result) => {
            if result.shell_failed {
                let warning_msg = t!("gui.system_dependencies.shell_verification_failed").to_string();
                send_message(&app_handle, warning_msg.clone(), "warning".to_string());
                warn!("{}", warning_msg);
                vec![]
            } else if result.missing.is_empty() {
                vec![]
            } else {
                result.missing.into_iter().map(|p| p.to_string()).collect()
            }
        }
        Err(err) => {
            let warning_msg = t!("gui.system_dependencies.verification_error", error = err.to_string()).to_string();
            send_message(&app_handle, warning_msg.clone(), "warning".to_string());
            warn!("{}", warning_msg);
            vec![]
        }
    }
}

#[tauri::command]
pub fn check_prerequisites_detailed(app_handle: AppHandle) -> serde_json::Value {
    match idf_im_lib::system_dependencies::check_prerequisites_with_result() {
        Ok(result) => {
            if result.shell_failed {
                // Shell execution failed - can't verify, user can skip
                let warning_msg = t!("gui.system_dependencies.shell_verification_failed").to_string();
                send_message(&app_handle, warning_msg.clone(), "warning".to_string());
                warn!("{}", warning_msg);
                json!({
                    "all_ok": false,
                    "missing": [],
                    "can_verify": false,
                    "shell_failed": true
                })
            } else if result.missing.is_empty() {
                // All prerequisites satisfied
                json!({
                    "all_ok": true,
                    "missing": [],
                    "can_verify": true,
                    "shell_failed": false
                })
            } else {
                // Some prerequisites missing - normal flow
                json!({
                    "all_ok": false,
                    "missing": result.missing.into_iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                    "can_verify": true,
                    "shell_failed": false
                })
            }
        }
        Err(err) => {
            // Error during checking (e.g., unsupported package manager) - can't verify, user can skip
            let error_msg = t!("gui.system_dependencies.verification_error", error = err.to_string()).to_string();
            send_message(&app_handle, error_msg.clone(), "warning".to_string());
            warn!("{}", error_msg);
            json!({
                "all_ok": false,
                "missing": [],
                "can_verify": false,
                "shell_failed": false
            })
        }
    }
}

/// Installs missing prerequisites
#[tauri::command]
pub fn install_prerequisites(app_handle: AppHandle) -> bool {
    let unsatisfied_prerequisites = match idf_im_lib::system_dependencies::check_prerequisites_with_result() {
        Ok(result) => {
            if result.shell_failed {
                let error_msg = t!("gui.system_dependencies.shell_verification_failed").to_string();
                send_message(&app_handle, error_msg.clone(), "error".to_string());
                error!("{}", error_msg);
                return false;
            }
            result.missing.into_iter().map(|p| p.to_string()).collect()
        }
        Err(err) => {
            let error_msg = t!("gui.system_dependencies.verification_error", error = err.to_string()).to_string();
            send_message(&app_handle, error_msg.clone(), "error".to_string());
            error!("{}", error_msg);
            return false;
        }
    };

    match idf_im_lib::system_dependencies::install_prerequisites(unsatisfied_prerequisites) {
        Ok(_) => true,
        Err(err) => {
            let error_msg = t!("gui.system_dependencies.error_installing_prerequisites", error = err.to_string()).to_string();
            send_message(
                &app_handle,
                error_msg.clone(),
                "error".to_string(),
            );
            error!("{}", error_msg);
            false
        }
    }
}

/// Performs a sanity check on the Python installation
#[tauri::command]
pub fn python_sanity_check(app_handle: AppHandle, python: Option<&str>) -> bool {
    let outputs = idf_im_lib::python_utils::python_sanity_check(python);
    let mut all_ok = true;

    for output in outputs {
        match output {
            Ok(_) => {}
            Err(err) => {
                all_ok = false;
                let warning_msg = t!("gui.system_dependencies.python_sanity_check_failed", error = err.to_string()).to_string();
                send_message(
                    &app_handle,
                    warning_msg.clone(),
                    "warning".to_string(),
                );
                warn!("{}", warning_msg);
            }
        }
    }
    all_ok
}

/// Installs Python
#[tauri::command]
pub fn python_install(app_handle: AppHandle) -> bool {
  let settings = match get_settings_non_blocking(&app_handle) {
      Ok(settings) => settings,
      Err(err) => {
          let error_msg = t!("gui.system_dependencies.error_getting_settings", error = err.to_string()).to_string();
          send_message(
              &app_handle,
              error_msg.clone(),
              "error".to_string(),
          );
          error!("{}", error_msg);
          return false;
      }
  };
    match idf_im_lib::system_dependencies::install_prerequisites(vec![settings.python_version_override.clone().unwrap_or_else(|| idf_im_lib::system_dependencies::PYTHON_NAME_TO_INSTALL.to_string())]) {
        Ok(_) => true,
        Err(err) => {
            let error_msg = t!("gui.system_dependencies.error_installing_python", error = err.to_string()).to_string();
            send_message(
                &app_handle,
                error_msg.clone(),
                "error".to_string(),
            );
            error!("{}", error_msg);
            false
        }
    }
}
