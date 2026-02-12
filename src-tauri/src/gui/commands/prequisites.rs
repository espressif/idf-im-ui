use crate::gui::{app_state::get_settings_non_blocking, ui::send_message};
use idf_im_lib::python_utils::SanityCheck;
use log::{error, warn};
use rust_i18n::t;
use serde_json::{json, Value};
use tauri::AppHandle;


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

/// Translated display name for a sanity check (GUI uses same locale keys as CLI).
pub fn check_display_name(check: SanityCheck) -> String {
    match check {
        SanityCheck::PythonVersion => t!("python.sanitycheck.check.version"),
        SanityCheck::Pip => t!("python.sanitycheck.check.pip"),
        SanityCheck::Venv => t!("python.sanitycheck.check.venv"),
        SanityCheck::StdLib => t!("python.sanitycheck.check.stdlib"),
        SanityCheck::Ctypes => t!("python.sanitycheck.check.ctypes"),
        SanityCheck::Ssl => t!("python.sanitycheck.check.ssl"),
    }
    .to_string()
}

/// OS-aware translated hint for a failed sanity check.
pub fn check_hint(check: SanityCheck) -> String {
    let os = std::env::consts::OS;
    match (check, os) {
        (SanityCheck::PythonVersion, _) => t!("python.sanitycheck.hint.version"),
        (SanityCheck::Pip, _) => t!("python.sanitycheck.hint.pip"),
        (SanityCheck::StdLib, _) => t!("python.sanitycheck.hint.stdlib"),
        (SanityCheck::Venv, "macos") => t!("python.sanitycheck.hint.venv.macos"),
        (SanityCheck::Venv, "windows") => t!("python.sanitycheck.hint.venv.windows"),
        (SanityCheck::Venv, _) => t!("python.sanitycheck.hint.venv.linux"),
        (SanityCheck::Ctypes, "macos") => t!("python.sanitycheck.hint.ctypes.macos"),
        (SanityCheck::Ctypes, "windows") => t!("python.sanitycheck.hint.ctypes.windows"),
        (SanityCheck::Ctypes, _) => t!("python.sanitycheck.hint.ctypes.linux"),
        (SanityCheck::Ssl, "macos") => t!("python.sanitycheck.hint.ssl.macos"),
        (SanityCheck::Ssl, "windows") => t!("python.sanitycheck.hint.ssl.windows"),
        (SanityCheck::Ssl, _) => t!("python.sanitycheck.hint.ssl.linux"),
    }
    .to_string()
}

/// One item for the GUI: translated name, pass/fail, and hint when failed.
/// Generic struct that can be reused for other check types (e.g., prerequisites, tool checks).
#[derive(serde::Serialize)]
pub struct CheckResultItem {
    pub display_name: String,
    pub passed: bool,
    pub hint: Option<String>,
}

/// Performs a sanity check and returns structured results for the GUI.
/// Raw command output is logged only; user sees display_name + hint per failure.
#[tauri::command]
pub fn python_sanity_check(app_handle: AppHandle, python: Option<&str>) -> Vec<CheckResultItem> {
    let results = idf_im_lib::python_utils::python_sanity_check(python);

    results
        .iter()
        .map(|r| {
            let display_name = check_display_name(r.check);
            if !r.passed {
                warn!("[FAIL] {}: {}", display_name, r.message);
                send_message(
                    &app_handle,
                    format!("{} — {}", display_name, check_hint(r.check)),
                    "warning".to_string(),
                );
            }
            CheckResultItem {
                display_name,
                passed: r.passed,
                hint: if r.passed { None } else { Some(check_hint(r.check)) },
            }
        })
        .collect()
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
