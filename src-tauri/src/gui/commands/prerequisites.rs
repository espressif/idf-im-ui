use std::path::PathBuf;

use crate::gui::{app_state::get_settings_non_blocking, ui::send_message};
use idf_im_lib::settings::Settings;
use log::{error, warn};
use rust_i18n::t;
use serde_json::json;
use tauri::{AppHandle, Emitter};

fn report_warning(app_handle: &AppHandle, warning_msg: String) {
    send_message(app_handle, warning_msg.clone(), "warning".to_string());
    warn!("{}", warning_msg);
}

fn report_error(app_handle: &AppHandle, error_msg: String) {
    send_message(app_handle, error_msg.clone(), "error".to_string());
    error!("{}", error_msg);
}

fn settings_or_report(app_handle: &AppHandle) -> Option<Settings> {
    get_settings_non_blocking(app_handle)
        .map_err(|err| {
            report_error(
                app_handle,
                t!(
                    "gui.system_dependencies.error_getting_settings",
                    error = err.to_string()
                )
                .to_string(),
            )
        })
        .ok()
}

fn tool_install_directory(settings: &Settings) -> PathBuf {
    PathBuf::from(
        settings
            .tool_install_folder_name
            .clone()
            .expect("Tools install folder not defined"),
    )
}

/// Installs `packages` in a background task and emits `completion_event` with
/// the outcome, so the calling command can return immediately.
fn spawn_install(
    app_handle: AppHandle,
    packages: Vec<String>,
    tool_install_directory: PathBuf,
    completion_event: &'static str,
    failure_message: fn(String) -> String,
) {
    tokio::spawn(async move {
        match idf_im_lib::system_dependencies::install_prerequisites(
            packages,
            tool_install_directory,
        )
        .await
        {
            Ok(_) => {
                let _ = app_handle.emit(
                    completion_event,
                    json!({
                        "success": true
                    }),
                );
            }
            Err(err) => {
                report_error(&app_handle, failure_message(err.to_string()));
                let _ = app_handle.emit(
                    completion_event,
                    json!({
                        "success": false,
                        "error": err.to_string()
                    }),
                );
            }
        }
    });
}

/// Gets the list of prerequisites for ESP-IDF
#[tauri::command]
pub fn get_prerequisites() -> Vec<&'static str> {
    idf_im_lib::system_dependencies::get_prerequisites()
        .into_iter()
        .chain(
            idf_im_lib::system_dependencies::get_general_prerequisites_based_on_package_manager(),
        )
        .collect()
}

/// Checks which prerequisites are missing
#[tauri::command]
pub fn check_prerequisites(app_handle: AppHandle) -> Vec<String> {
    match idf_im_lib::system_dependencies::check_prerequisites_with_result() {
        Ok(result) => {
            if result.shell_failed {
                report_warning(
                    &app_handle,
                    t!("gui.system_dependencies.shell_verification_failed").to_string(),
                );
                vec![]
            } else if result.missing.is_empty() {
                vec![]
            } else {
                result.missing.into_iter().map(|p| p.to_string()).collect()
            }
        }
        Err(err) => {
            report_warning(
                &app_handle,
                t!(
                    "gui.system_dependencies.verification_error",
                    error = err.to_string()
                )
                .to_string(),
            );
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
                report_warning(
                    &app_handle,
                    t!("gui.system_dependencies.shell_verification_failed").to_string(),
                );
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
            report_warning(
                &app_handle,
                t!(
                    "gui.system_dependencies.verification_error",
                    error = err.to_string()
                )
                .to_string(),
            );
            json!({
                "all_ok": false,
                "missing": [],
                "can_verify": false,
                "shell_failed": false
            })
        }
    }
}

/// Installs missing prerequisites (non-blocking - runs in tokio background task)
#[tauri::command]
pub fn install_prerequisites(app_handle: AppHandle) -> bool {
    let unsatisfied_prerequisites =
        match idf_im_lib::system_dependencies::check_prerequisites_with_result() {
            Ok(result) => {
                if result.shell_failed {
                    report_error(
                        &app_handle,
                        t!("gui.system_dependencies.shell_verification_failed").to_string(),
                    );
                    return false;
                }
                result.missing.into_iter().map(|p| p.to_string()).collect()
            }
            Err(err) => {
                report_error(
                    &app_handle,
                    t!(
                        "gui.system_dependencies.verification_error",
                        error = err.to_string()
                    )
                    .to_string(),
                );
                return false;
            }
        };

    let Some(settings) = settings_or_report(&app_handle) else {
        return false;
    };
    spawn_install(
        app_handle,
        unsatisfied_prerequisites,
        tool_install_directory(&settings),
        "prerequisites-install-complete",
        |error| {
            t!(
                "gui.system_dependencies.error_installing_prerequisites",
                error = error
            )
            .to_string()
        },
    );

    // Return immediately - installation runs in background
    true
}

/// GUI presentation DTO serialized to the Vue frontend.
///
/// Not a duplicate of [`GenericCheckResult<T>`](idf_im_lib::utils::GenericCheckResult):
/// that struct carries the raw enum variant + raw command output and lives in the
/// i18n-free library, whereas this struct carries translated strings ready for display.
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
    let results = idf_im_lib::python_utils::python_sanity_check(python, false);

    results
        .iter()
        .map(|r| {
            let display_name = t!(r.check.display_key()).to_string();
            let hint = t!(r.check.hint_key_for_os(std::env::consts::OS)).to_string();
            if !r.passed {
                warn!("[FAIL] {}: {}", display_name, r.message);
                send_message(
                    &app_handle,
                    format!("{} — {}", display_name, hint),
                    "warning".to_string(),
                );
            }
            CheckResultItem {
                display_name,
                passed: r.passed,
                hint: if r.passed { None } else { Some(hint) },
            }
        })
        .collect()
}

/// Installs Python (non-blocking - runs in tokio background task)
#[tauri::command]
pub fn python_install(app_handle: AppHandle) -> bool {
    let Some(settings) = settings_or_report(&app_handle) else {
        return false;
    };
    let python_version = settings
        .python_version_override
        .clone()
        .unwrap_or_else(|| idf_im_lib::system_dependencies::PYTHON_NAME_TO_INSTALL.to_string());
    spawn_install(
        app_handle,
        vec![python_version],
        tool_install_directory(&settings),
        "python-install-complete",
        |error| {
            t!(
                "gui.system_dependencies.error_installing_python",
                error = error
            )
            .to_string()
        },
    );

    // Return immediately - installation runs in background
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_install_directory_uses_configured_folder_name() {
        let settings = Settings {
            tool_install_folder_name: Some("tools".to_string()),
            ..Settings::default()
        };
        assert_eq!(tool_install_directory(&settings), PathBuf::from("tools"));
    }

    #[test]
    #[should_panic(expected = "Tools install folder not defined")]
    fn tool_install_directory_requires_folder_name() {
        let settings = Settings {
            tool_install_folder_name: None,
            ..Settings::default()
        };
        tool_install_directory(&settings);
    }
}
