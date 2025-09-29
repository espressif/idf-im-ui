use super::online::install_single_version;
use crate::gui::__cmd__get_installed_versions;
use crate::gui::app_state::set_installation_status;
use crate::gui::get_installed_versions;
use crate::gui::ui::{
    emit_installation_event, emit_log_message, InstallationProgress, InstallationStage,
    MessageLevel,
};
use idf_im_lib::version_manager::prepare_settings_for_fix_idf_installation;
use idf_im_lib::{ensure_path, idf_config::IdfConfig, version_manager::get_default_config_path};
use log::{debug, error, info, warn};
use std::path::PathBuf;
use tauri::AppHandle;

#[tauri::command]
pub async fn fix_installation(app_handle: AppHandle, id: String) -> Result<(), String> {
    debug!("Fixing installation with id {}", id);

    // Set installation flag to indicate installation is running
    set_installation_status(&app_handle, true)?;

    // Initial progress - checking installation
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 0,
            message: "Checking installation to repair...".to_string(),
            detail: Some(format!("Looking up installation ID: {}", id)),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        format!("Starting repair process for installation: {}", id),
    );

    let versions = get_installed_versions();
    let installation = match versions.iter().find(|v| v.id == id) {
        Some(inst) => {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Checking,
                    percentage: 10,
                    message: format!("Found installation: ESP-IDF {}", inst.name),
                    detail: Some(format!("Path: {}", inst.path)),
                    version: Some(inst.name.clone()),
                },
            );

            emit_log_message(
                &app_handle,
                MessageLevel::Info,
                format!("Found installation {} at: {}", inst.name, inst.path),
            );

            inst
        }
        None => {
            let error_msg = format!("Installation with ID {} not found", id);
            error!("{}", error_msg);

            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Installation not found".to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                },
            );

            emit_log_message(&app_handle, MessageLevel::Error, error_msg.clone());
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }
    };

    // Preparing settings
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 20,
            message: "Preparing repair configuration...".to_string(),
            detail: Some("Setting up installation parameters".to_string()),
            version: Some(installation.name.clone()),
        },
    );

    let mut settings =
        match prepare_settings_for_fix_idf_installation(PathBuf::from(installation.path.clone()))
            .await
        {
            Ok(settings) => {
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Prerequisites,
                        percentage: 30,
                        message: "Configuration prepared successfully".to_string(),
                        detail: Some("Ready to begin repair process".to_string()),
                        version: Some(installation.name.clone()),
                    },
                );

                emit_log_message(
                    &app_handle,
                    MessageLevel::Success,
                    "Repair configuration prepared successfully".to_string(),
                );

                settings
            }
            Err(e) => {
                let error_msg = format!("Failed to prepare settings for repair: {}", e);
                error!("{}", error_msg);

                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "Failed to prepare repair configuration".to_string(),
                        detail: Some(e.to_string()),
                        version: Some(installation.name.clone()),
                    },
                );

                emit_log_message(&app_handle, MessageLevel::Error, error_msg.clone());
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        };

    // Get the config path for IDE configuration
    let config_path = get_default_config_path();

    // Starting actual repair (reinstallation)
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Download,
            percentage: 35,
            message: format!("Starting ESP-IDF {} repair...", installation.name),
            detail: Some("Beginning reinstallation process".to_string()),
            version: Some(installation.name.clone()),
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        format!(
            "Starting repair installation for ESP-IDF {}",
            installation.name
        ),
    );

    // The actual repair process - this will generate detailed progress events
    match install_single_version(app_handle.clone(), &settings, installation.name.clone()).await {
        Ok(_) => {
            emit_log_message(
                &app_handle,
                MessageLevel::Success,
                format!(
                    "Successfully repaired ESP-IDF {} installation",
                    installation.name
                ),
            );

            info!("Successfully fixed installation {}", id);
        }
        Err(e) => {
            let error_msg = format!("Failed to repair installation: {}", e);
            error!("{}", error_msg);

            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: format!("ESP-IDF {} repair failed", installation.name),
                    detail: Some(e.to_string()),
                    version: Some(installation.name.clone()),
                },
            );

            emit_log_message(&app_handle, MessageLevel::Error, error_msg.clone());
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }
    }

    // Configure IDE configuration update
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Configure,
            percentage: 90,
            message: "Updating IDE configuration...".to_string(),
            detail: Some("Saving installation information".to_string()),
            version: Some(installation.name.clone()),
        },
    );

    // The installation should have been added back by single_version_post_install()
    // but let's ensure the IDE configuration is properly saved by reconstructing
    // the settings with the right configuration
    let paths = settings
        .get_version_paths(&installation.name)
        .map_err(|err| {
            error!("Failed to get version paths after repair: {}", err);
            format!("Failed to get version paths after repair: {}", err)
        })?;

    // Debug: Check what config_path contains
    info!("Config path for IDE JSON: {}", config_path.display());
    info!("Config path exists: {}", config_path.exists());
    info!("Config path is file: {}", config_path.is_file());
    info!("Config path is dir: {}", config_path.is_dir());

    // Create a properly configured Settings object for IDE JSON saving
    let mut updated_settings = settings.clone();
    // esp_idf_json_path should be the directory, not the file itself
    // because save_esp_ide_json() will append the filename
    if let Some(parent_dir) = config_path.parent() {
        updated_settings.esp_idf_json_path = Some(parent_dir.to_string_lossy().to_string());
    } else {
        updated_settings.esp_idf_json_path = Some(config_path.to_string_lossy().to_string());
    }
    updated_settings.idf_versions = Some(vec![installation.name.clone()]);
    updated_settings.idf_path = Some(paths.idf_path.clone());

    // Ensure the parent directory exists (not the file itself)
    let ide_json_path = updated_settings.esp_idf_json_path.as_ref().unwrap();
    info!("IDE JSON path to save: {}", ide_json_path);

    if let Some(parent_dir) = std::path::Path::new(ide_json_path).parent() {
        info!("Parent directory: {}", parent_dir.display());
        info!("Parent directory exists: {}", parent_dir.exists());
        match ensure_path(parent_dir.to_str().unwrap()) {
            Ok(_) => info!("Parent directory ensured successfully"),
            Err(e) => error!("Failed to ensure parent directory: {}", e),
        }
    }

    // Let's check if single_version_post_install already added the installation back
    let ide_json_path = updated_settings
        .esp_idf_json_path
        .clone()
        .unwrap_or_default();

    // Try to read the current IDE config to see what's in it
    match IdfConfig::from_file(&config_path) {
        Ok(current_config) => {
            info!(
                "Current IDE config has {} installed versions",
                current_config.idf_installed.len()
            );
            for installed in &current_config.idf_installed {
                info!(
                    "Installed version: {} (ID: {})",
                    installed.name, installed.id
                );
            }

            // Check if our repaired installation is already there
            let found_installation = current_config
                .idf_installed
                .iter()
                .find(|inst| inst.name == installation.name && inst.path == installation.path);

            if found_installation.is_some() {
                info!("Repaired installation already found in IDE config - no need to save again");
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Configure,
                        percentage: 95,
                        message: "IDE configuration already updated".to_string(),
                        detail: Some("Installation found in configuration".to_string()),
                        version: Some(installation.name.clone()),
                    },
                );
            } else {
                info!("Repaired installation not found in IDE config - trying to save");
                // Try to save the configuration
                match updated_settings.save_esp_ide_json() {
                    Ok(_) => {
                        emit_installation_event(
                            &app_handle,
                            InstallationProgress {
                                stage: InstallationStage::Configure,
                                percentage: 95,
                                message: "IDE configuration saved successfully".to_string(),
                                detail: Some(format!("Configuration saved to: {}", ide_json_path)),
                                version: Some(installation.name.clone()),
                            },
                        );

                        emit_log_message(
                            &app_handle,
                            MessageLevel::Success,
                            format!("IDE JSON file updated at: {}", ide_json_path),
                        );

                        info!("IDE JSON saved to {}", ide_json_path);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to save IDE configuration: {}", e);
                        warn!("{}", error_msg);

                        emit_installation_event(
                            &app_handle,
                            InstallationProgress {
                                stage: InstallationStage::Configure,
                                percentage: 95,
                                message: "Warning: IDE configuration save failed".to_string(),
                                detail: Some(e.to_string()),
                                version: Some(installation.name.clone()),
                            },
                        );

                        emit_log_message(&app_handle, MessageLevel::Warning, error_msg);
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to read current IDE config: {}", e);
            // Try to save anyway
            match updated_settings.save_esp_ide_json() {
                Ok(_) => {
                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Configure,
                            percentage: 95,
                            message: "IDE configuration saved successfully".to_string(),
                            detail: Some(format!("Configuration saved to: {}", ide_json_path)),
                            version: Some(installation.name.clone()),
                        },
                    );

                    emit_log_message(
                        &app_handle,
                        MessageLevel::Success,
                        format!("IDE JSON file updated at: {}", ide_json_path),
                    );

                    info!("IDE JSON saved to {}", ide_json_path);
                }
                Err(e) => {
                    let error_msg = format!("Failed to save IDE configuration: {}", e);
                    warn!("{}", error_msg);

                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Configure,
                            percentage: 95,
                            message: "Warning: IDE configuration save failed".to_string(),
                            detail: Some(e.to_string()),
                            version: Some(installation.name.clone()),
                        },
                    );

                    emit_log_message(&app_handle, MessageLevel::Warning, error_msg);
                }
            }
        }
    }

    // Final completion
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Complete,
            percentage: 100,
            message: format!(
                "ESP-IDF {} repair completed successfully!",
                installation.name
            ),
            detail: Some(format!("Installation repaired at: {}", installation.path)),
            version: Some(installation.name.clone()),
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Success,
        format!(
            "Repair process completed successfully for ESP-IDF {}",
            installation.name
        ),
    );

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}
