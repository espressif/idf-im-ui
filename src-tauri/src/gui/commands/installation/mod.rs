pub mod common;
pub mod offline;
pub mod online;
pub mod platform;
pub mod progress;
pub mod repair;

pub use common::InstallationPlan;
pub use offline::start_offline_installation;
pub use platform::start_installation;
pub use repair::fix_installation;

use crate::gui::app_state;
use crate::gui::app_state::get_locked_settings;
use crate::gui::app_state::update_settings;
use crate::gui::check_prequisites;
use crate::gui::commands::installation::platform::start_installation as platform_start_installation;
use crate::gui::commands::prequisites::{
    install_prerequisites, python_install, python_sanity_check,
};
use crate::gui::commands::settings;
use crate::gui::ui::{emit_installation_event, emit_log_message, MessageLevel};
use crate::gui::ui::{InstallationProgress, InstallationStage};
use tauri::{AppHandle, Emitter};

pub fn emit_installation_plan(app_handle: &AppHandle, plan: InstallationPlan) {
    let _ = app_handle.emit("installation-plan", plan);
}

#[tauri::command]
pub fn is_installing(app_handle: AppHandle) -> bool {
    app_state::is_installation_in_progress(&app_handle)
}

#[tauri::command]
pub async fn start_simple_setup(app_handle: tauri::AppHandle) -> Result<(), String> {
    println!("Starting simple setup");
    let settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(e) => {
            emit_log_message(&app_handle, MessageLevel::Error, e.clone());
            return Err(e);
        }
    };

    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 0,
            message: "Starting installation...".to_string(),
            detail: None,
            version: None,
        },
    );

    // Check prerequisites
    let mut prerequisites = check_prequisites(app_handle.clone());
    let os = std::env::consts::OS.to_lowercase();

    // Install prerequisites on Windows if needed
    if !prerequisites.is_empty() && os == "windows" {
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Prerequisites,
                percentage: 5,
                message: "Installing prerequisites...".to_string(),
                detail: Some(format!("Missing: {}", prerequisites.join(", "))),
                version: None,
            },
        );

        if !install_prerequisites(app_handle.clone()) {
            prerequisites = check_prequisites(app_handle.clone());
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Failed to install prerequisites".to_string(),
                    detail: Some(format!("Missing: {}", prerequisites.join(", "))),
                    version: None,
                },
            );
            return Err("Failed to install prerequisites".to_string());
        }

        prerequisites = check_prequisites(app_handle.clone());
    }

    // Check if any prerequisites are still missing
    if !prerequisites.is_empty() {
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Prerequisites missing".to_string(),
                detail: Some(format!("Please install: {}", prerequisites.join(", "))),
                version: None,
            },
        );
        return Err("Prerequisites missing".to_string());
    }

    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Prerequisites,
            percentage: 10,
            message: "Prerequisites verified".to_string(),
            detail: None,
            version: None,
        },
    );

    // Check for Python
    let mut python_found = python_sanity_check(app_handle.clone(), None);

    // Install Python on Windows if needed
    if !python_found && os == "windows" {
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Python,
                percentage: 15,
                message: "Installing Python...".to_string(),
                detail: None,
                version: None,
            },
        );

        if !python_install(app_handle.clone()) {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Failed to install Python".to_string(),
                    detail: Some("Python installation failed".to_string()),
                    version: None,
                },
            );
            return Err("Failed to install Python".to_string());
        }

        python_found = python_sanity_check(app_handle.clone(), None);
    }

    // Check if Python is still not found
    if !python_found {
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Python not found".to_string(),
                detail: Some("Please install Python 3.11 manually".to_string()),
                version: None,
            },
        );
        return Err("Python not found".to_string());
    }

    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Python,
            percentage: 20,
            message: "Python environment ready".to_string(),
            detail: None,
            version: None,
        },
    );

    // Check for IDF versions
    if settings.idf_versions.is_none() {
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Configure,
                percentage: 25,
                message: "Fetching available ESP-IDF versions...".to_string(),
                detail: None,
                version: None,
            },
        );

        let versions = settings::get_idf_versions(app_handle.clone()).await;

        if versions.is_empty() {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Failed to fetch ESP-IDF versions".to_string(),
                    detail: Some("Could not retrieve version list from server".to_string()),
                    version: None,
                },
            );
            return Err("Failed to fetch ESP-IDF versions".to_string());
        }

        let version = versions[0]["name"]
            .clone()
            .to_string()
            .trim_matches('"')
            .to_string();

        match update_settings(&app_handle, |settings| {
            settings.idf_versions = Some(vec![version.clone()]);
        }) {
            Ok(_) => {
                emit_log_message(
                    &app_handle,
                    MessageLevel::Info,
                    format!("Selected ESP-IDF version: {}", version),
                );

                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Configure,
                        percentage: 30,
                        message: format!("ESP-IDF {} selected", version),
                        detail: None,
                        version: Some(version.clone()),
                    },
                );
            }
            Err(e) => {
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "Failed to configure ESP-IDF version".to_string(),
                        detail: Some(e.to_string()),
                        version: None,
                    },
                );
                return Err(e);
            }
        }
    }

    // Start installation
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Download,
            percentage: 35,
            message: "Starting ESP-IDF installation...".to_string(),
            detail: None,
            version: settings
                .idf_versions
                .as_ref()
                .and_then(|v| v.first().cloned()),
        },
    );

    start_installation(app_handle.clone()).await
}
