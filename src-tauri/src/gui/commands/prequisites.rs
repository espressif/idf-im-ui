use crate::gui::{app_state::get_settings_non_blocking, ui::send_message};
use idf_im_lib;
use idf_im_lib::python_utils::PythonSanityCheckResponse;
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
    match idf_im_lib::system_dependencies::check_prerequisites() {
        Ok(prerequisites) => {
            if prerequisites.is_empty() {
                vec![]
            } else {
                prerequisites.into_iter().map(|p| p.to_string()).collect()
            }
        }
        Err(err) => {
            let error_msg = t!("gui.system_dependencies.error_checking_prerequisites", error = err.to_string()).to_string();
            send_message(
                &app_handle,
                error_msg.clone(),
                "error".to_string(),
            );
            error!("{}", error_msg);
            vec![]
        }
    }
}

#[tauri::command]
pub fn check_prerequisites_detailed(app_handle: AppHandle) -> serde_json::Value {
  match idf_im_lib::system_dependencies::check_prerequisites() {
    Ok(prerequisites) => {
            if prerequisites.is_empty() {
              json!({
                "all_ok": true,
                "missing": []
              })
            } else {
              json!({
                "all_ok": false,
                "missing": prerequisites.into_iter().map(|p| p.to_string()).collect::<Vec<_>>()
              })
            }
        }
        Err(err) => {
            let error_msg = t!("gui.system_dependencies.error_checking_prerequisites", error = err.to_string()).to_string();
            send_message(
                &app_handle,
                error_msg.clone(),
                "error".to_string(),
            );
            error!("{}", error_msg);
            json!({
                "all_ok": false,
                "missing": []
            })
        }
    }

}

/// Installs missing prerequisites
#[tauri::command]
pub fn install_prerequisites(app_handle: AppHandle) -> bool {
    let unsatisfied_prerequisites = match idf_im_lib::system_dependencies::check_prerequisites() {
        Ok(prereqs) => prereqs.into_iter().map(|p| p.to_string()).collect(),
        Err(err) => {
            let error_msg = t!("gui.system_dependencies.error_checking_prerequisites", error = err.to_string()).to_string();
            send_message(
                &app_handle,
                error_msg.clone(),
                "error".to_string(),
            );
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

/// Performs a sanity check on the Python installation.
/// Returns a structured response with detailed results for each check.
#[tauri::command]
pub fn python_sanity_check(app_handle: AppHandle, python: Option<&str>) -> PythonSanityCheckResponse {
    let response = idf_im_lib::python_utils::python_sanity_check(python);

    // Log failed checks (English, label + raw details for debugging)
    for result in &response.results {
        if !result.passed {
            warn!(
                "Python check failed: check_type={}, label={}, details={:?}",
                result.check_type,
                result.label,
                result.message.as_deref()
            );
            let warning_msg = t!(
                "gui.system_dependencies.python_check_failed_label",
                label = result.label.as_str()
            )
            .to_string();
            send_message(&app_handle, warning_msg, "warning".to_string());
        }
    }

    response
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
