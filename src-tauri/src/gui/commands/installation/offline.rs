use std::path::PathBuf;
use tauri::AppHandle;
use tempfile::TempDir;

use crate::gui::app_state::{get_settings_non_blocking, set_installation_status};
use crate::gui::commands::idf_tools::setup_tools;
use crate::gui::commands::prequisites::check_prequisites;
use crate::gui::get_file_name;
use crate::gui::ui::{
    emit_installation_event, emit_log_message, InstallationProgress, InstallationStage,
    MessageLevel,
};
use crate::gui::utils::is_path_empty_or_nonexistent;
use idf_im_lib::{
    ensure_path,
    offline_installer::{
        copy_idf_from_offline_archive, install_prerequisites_offline, use_offline_archive,
    },
    python_utils,
    utils::copy_dir_contents,
};
use log::{debug, error, info, warn};

#[tauri::command]
pub async fn start_offline_installation(
    app_handle: AppHandle,
    archives: Vec<String>,
    install_path: String,
) -> Result<(), String> {
    // Set installation flag
    if let Err(e) = set_installation_status(&app_handle, true) {
        return Err(e);
    }

    // Validate archives
    if archives.is_empty() {
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "No archives provided for offline installation".to_string(),
                detail: Some("Please select at least one archive file".to_string()),
                version: None,
            },
        );
        set_installation_status(&app_handle, false)?;
        return Err("No archives provided for offline installation".to_string());
    }

    // Initial progress event
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 0,
            message: "Starting offline installation...".to_string(),
            detail: Some(format!("Processing {} archive(s)", archives.len())),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        format!(
            "Starting offline installation with {} archive(s)",
            archives.len()
        ),
    );

    let total_archives = archives.len();

    for (archive_index, archive) in archives.iter().enumerate() {
        let archive_path = std::path::PathBuf::from(archive);

        // Check archive exists
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Checking,
                percentage: (archive_index * 10 / total_archives) as u32,
                message: format!("Validating archive: {}", get_file_name(archive)),
                detail: Some(format!(
                    "Archive {} of {}",
                    archive_index + 1,
                    total_archives
                )),
                version: None,
            },
        );

        if !archive_path.try_exists().unwrap_or(false) {
            let error_msg = format!("Archive file does not exist: {}", archive);
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Archive not found".to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                },
            );
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }

        emit_log_message(
            &app_handle,
            MessageLevel::Info,
            format!("Validated archive: {}", archive),
        );

        // Create temporary directory for extraction
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Extract,
                percentage: ((archive_index * 90 + 10) / total_archives) as u32,
                message: "Creating temporary workspace...".to_string(),
                detail: Some("Preparing for archive extraction".to_string()),
                version: None,
            },
        );

        let offline_archive_dir = TempDir::new().map_err(|e| {
            let error_msg = format!("Failed to create temporary directory: {}", e);
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Failed to create workspace".to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                },
            );
            error_msg
        })?;

        emit_log_message(
            &app_handle,
            MessageLevel::Info,
            format!(
                "Created temporary directory: {}",
                offline_archive_dir.path().display()
            ),
        );

        // Get and configure settings
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Extract,
                percentage: ((archive_index * 90 + 15) / total_archives) as u32,
                message: "Configuring installation settings...".to_string(),
                detail: Some("Preparing installation configuration".to_string()),
                version: None,
            },
        );

        let mut settings = get_settings_non_blocking(&app_handle)?;
        if !install_path.is_empty() && is_path_empty_or_nonexistent(&install_path, &[]) {
            settings.path = Some(PathBuf::from(install_path.clone()));
            emit_log_message(
                &app_handle,
                MessageLevel::Info,
                format!("Using custom installation path: {}", install_path),
            );
        }
        settings.use_local_archive = Some(archive_path);

        // Extract and configure archive
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Extract,
                percentage: ((archive_index * 90 + 20) / total_archives) as u32,
                message: format!("Extracting archive: {}", get_file_name(archive)),
                detail: Some("Processing offline archive contents".to_string()),
                version: None,
            },
        );

        settings = match use_offline_archive(settings, &offline_archive_dir) {
            Ok(updated_config) => {
                emit_log_message(
                    &app_handle,
                    MessageLevel::Success,
                    "Archive extracted and configured successfully".to_string(),
                );
                updated_config
            }
            Err(err) => {
                let error_msg = format!("Failed to use offline archive: {}", err);
                error!("{}", error_msg);
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "Archive extraction failed".to_string(),
                        detail: Some(error_msg.clone()),
                        version: None,
                    },
                );
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        };

        // Install prerequisites on Windows
        if std::env::consts::OS == "windows" {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Prerequisites,
                    percentage: ((archive_index * 90 + 25) / total_archives) as u32,
                    message: "Installing prerequisites...".to_string(),
                    detail: Some("Installing required Windows components".to_string()),
                    version: None,
                },
            );

            match install_prerequisites_offline(&offline_archive_dir) {
                Ok(_) => {
                    emit_log_message(
                        &app_handle,
                        MessageLevel::Success,
                        "Prerequisites installed successfully from offline archive".to_string(),
                    );
                    settings.skip_prerequisites_check = Some(true);
                }
                Err(err) => {
                    let error_msg = format!(
                        "Failed to install prerequisites from offline archive: {}",
                        err
                    );
                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Error,
                            percentage: 0,
                            message: "Prerequisites installation failed".to_string(),
                            detail: Some(error_msg.clone()),
                            version: None,
                        },
                    );
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            }
        } else {
            // Check prerequisites on non-Windows systems
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Prerequisites,
                    percentage: ((archive_index * 90 + 25) / total_archives) as u32,
                    message: "Checking system prerequisites...".to_string(),
                    detail: Some("Verifying required system components".to_string()),
                    version: None,
                },
            );

            let prereq = check_prequisites(app_handle.clone());
            if !prereq.is_empty() {
                let error_msg = format!("Missing prerequisites: {}", prereq.join(", "));
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "Prerequisites missing".to_string(),
                        detail: Some(error_msg.clone()),
                        version: None,
                    },
                );
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }

            // Python sanity check
            let mut python_sane = true;
            for result in idf_im_lib::python_utils::python_sanity_check(None) {
                match result {
                    Ok(_) => {}
                    Err(err) => {
                        python_sane = false;
                        emit_log_message(
                            &app_handle,
                            MessageLevel::Warning,
                            format!("Python sanity check failed: {}", err),
                        );
                        warn!("{:?}", err);
                    }
                }
            }
            if !python_sane {
                let error_msg = "Python sanity check failed".to_string();
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: error_msg.clone(),
                        detail: Some("Python environment is not properly configured".to_string()),
                        version: None,
                    },
                );
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }

            emit_log_message(
                &app_handle,
                MessageLevel::Success,
                "All prerequisites verified successfully".to_string(),
            );
        }

        // Copy ESP-IDF from archive
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Download,
                percentage: ((archive_index * 90 + 35) / total_archives) as u32,
                message: "Installing ESP-IDF from archive...".to_string(),
                detail: Some("Copying ESP-IDF files to installation directory".to_string()),
                version: None,
            },
        );

        match copy_idf_from_offline_archive(&offline_archive_dir, &settings) {
            Ok(_) => {
                emit_log_message(
                    &app_handle,
                    MessageLevel::Success,
                    "ESP-IDF copied successfully from offline archive".to_string(),
                );
            }
            Err(err) => {
                let error_msg = format!("Failed to copy ESP-IDF from offline archive: {}", err);
                error!("{}", error_msg);
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "ESP-IDF installation failed".to_string(),
                        detail: Some(error_msg.clone()),
                        version: None,
                    },
                );
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        }

        // Process each IDF version
        let versions = settings.idf_versions.clone().unwrap_or_default();
        for (version_index, idf_version) in versions.iter().enumerate() {
            let version_progress_start = ((archive_index * 90 + 40) / total_archives) as u32;
            let version_progress_end = ((archive_index * 90 + 85) / total_archives) as u32;
            let version_progress_range = version_progress_end - version_progress_start;

            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: version_progress_start
                        + (version_index as u32 * version_progress_range / versions.len() as u32),
                    message: format!("Processing ESP-IDF version: {}", idf_version),
                    detail: Some(format!(
                        "Setting up version {} of {}",
                        version_index + 1,
                        versions.len()
                    )),
                    version: Some(idf_version.clone()),
                },
            );

            let paths = match settings.get_version_paths(idf_version) {
                Ok(paths) => {
                    emit_log_message(
                        &app_handle,
                        MessageLevel::Info,
                        format!(
                            "Version paths configured for {}: {}",
                            idf_version,
                            paths.idf_path.display()
                        ),
                    );
                    paths
                }
                Err(err) => {
                    let error_msg = format!("Failed to get version paths: {}", err);
                    error!("{}", error_msg);
                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Error,
                            percentage: 0,
                            message: "Path configuration failed".to_string(),
                            detail: Some(error_msg.clone()),
                            version: Some(idf_version.clone()),
                        },
                    );
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            };

            settings.idf_path = Some(paths.idf_path.clone());
            idf_im_lib::add_path_to_path(paths.idf_path.to_str().unwrap());

            // Copy tools
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: version_progress_start
                        + ((version_index + 1) as u32 * version_progress_range
                            / (versions.len() as u32 * 3)),
                    message: "Installing development tools...".to_string(),
                    detail: Some("Copying tools from offline archive".to_string()),
                    version: Some(idf_version.clone()),
                },
            );

            match copy_dir_contents(
                &offline_archive_dir.path().join("dist"),
                &paths.tool_download_directory,
            ) {
                Ok(_) => {
                    emit_log_message(
                        &app_handle,
                        MessageLevel::Success,
                        "Development tools copied successfully".to_string(),
                    );
                }
                Err(err) => {
                    let error_msg = format!("Failed to copy tool directory: {}", err);
                    error!("{}", error_msg);
                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Error,
                            percentage: 0,
                            message: "Tools installation failed".to_string(),
                            detail: Some(error_msg.clone()),
                            version: Some(idf_version.clone()),
                        },
                    );
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            }

            idf_im_lib::add_path_to_path(paths.tool_install_directory.to_str().unwrap());

            // Setup tools
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: version_progress_start
                        + ((version_index + 1) as u32 * version_progress_range * 2
                            / (versions.len() as u32 * 3)),
                    message: "Configuring development tools...".to_string(),
                    detail: Some("Setting up tool environment".to_string()),
                    version: Some(idf_version.clone()),
                },
            );

            let export_vars = match setup_tools(
                &app_handle,
                &settings,
                &paths.idf_path,
                &paths.actual_version,
            )
            .await
            {
                Ok(vars) => {
                    emit_log_message(
                        &app_handle,
                        MessageLevel::Success,
                        "Development tools configured successfully".to_string(),
                    );
                    vars
                }
                Err(err) => {
                    let error_msg = format!("Failed to setup tools: {}", err);
                    error!("{}", error_msg);
                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Error,
                            percentage: 0,
                            message: "Tools configuration failed".to_string(),
                            detail: Some(error_msg.clone()),
                            version: Some(idf_version.clone()),
                        },
                    );
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            };

            // Install Python environment
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Python,
                    percentage: version_progress_start
                        + ((version_index + 1) as u32 * version_progress_range * 3
                            / (versions.len() as u32 * 3))
                        - 5,
                    message: "Setting up Python environment...".to_string(),
                    detail: Some("Installing Python dependencies".to_string()),
                    version: Some(idf_version.clone()),
                },
            );

            match idf_im_lib::python_utils::install_python_env(
                &paths.actual_version,
                &paths.tool_install_directory,
                true,
                &paths.idf_path,
                &settings.idf_features.clone().unwrap_or_default(),
                Some(offline_archive_dir.path()),
            )
            .await
            {
                Ok(_) => {
                    emit_log_message(
                        &app_handle,
                        MessageLevel::Success,
                        "Python environment installed successfully".to_string(),
                    );
                }
                Err(err) => {
                    let error_msg = format!("Failed to install Python environment: {}", err);
                    error!("{}", error_msg);
                    emit_installation_event(
                        &app_handle,
                        InstallationProgress {
                            stage: InstallationStage::Error,
                            percentage: 0,
                            message: "Python environment installation failed".to_string(),
                            detail: Some(error_msg.clone()),
                            version: Some(idf_version.clone()),
                        },
                    );
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            }

            // Post-install configuration
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Configure,
                    percentage: version_progress_end - 5,
                    message: "Finalizing installation...".to_string(),
                    detail: Some("Completing setup and configuration".to_string()),
                    version: Some(idf_version.clone()),
                },
            );

            idf_im_lib::single_version_post_install(
                &paths.activation_script_path.to_str().unwrap(),
                paths.idf_path.to_str().unwrap(),
                &paths.actual_version,
                paths.tool_install_directory.to_str().unwrap(),
                export_vars,
                paths.python_venv_path.to_str(),
                None,
            );

            emit_log_message(
                &app_handle,
                MessageLevel::Success,
                format!("ESP-IDF version {} configured successfully", idf_version),
            );
        }

        // Save IDE configuration
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Configure,
                percentage: ((archive_index * 90 + 88) / total_archives) as u32,
                message: "Saving IDE configuration...".to_string(),
                detail: Some("Updating development environment settings".to_string()),
                version: None,
            },
        );

        let ide_conf_path_tmp =
            PathBuf::from(&settings.esp_idf_json_path.clone().unwrap_or_default());
        debug!("IDE configuration path: {}", ide_conf_path_tmp.display());

        match ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
            Ok(_) => {
                emit_log_message(
                    &app_handle,
                    MessageLevel::Info,
                    "IDE configuration directory created".to_string(),
                );
            }
            Err(err) => {
                let error_msg = format!("Failed to create IDE configuration directory: {}", err);
                error!("{}", error_msg);
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "IDE configuration failed".to_string(),
                        detail: Some(error_msg.clone()),
                        version: None,
                    },
                );
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        }

        match settings.save_esp_ide_json() {
            Ok(_) => {
                emit_log_message(
                    &app_handle,
                    MessageLevel::Success,
                    "IDE configuration saved successfully".to_string(),
                );
                debug!("IDE configuration saved.");
            }
            Err(err) => {
                let error_msg = format!("Failed to save IDE configuration: {}", err);
                error!("{}", error_msg);
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "IDE configuration save failed".to_string(),
                        detail: Some(error_msg.clone()),
                        version: None,
                    },
                );
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        }

        emit_log_message(
            &app_handle,
            MessageLevel::Success,
            format!(
                "Archive {} processed successfully ({}/{})",
                get_file_name(archive),
                archive_index + 1,
                total_archives
            ),
        );
    }

    // Final completion
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Complete,
            percentage: 100,
            message: "Offline installation completed successfully!".to_string(),
            detail: Some(format!("Processed {} archive(s)", total_archives)),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Success,
        "All offline installations completed successfully".to_string(),
    );

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}
