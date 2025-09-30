use super::common::InstallationPlan;
use super::online::install_single_version;
use crate::gui::app_state::{
    get_locked_settings, get_settings_non_blocking, set_installation_status,
};
use crate::gui::emit_installation_plan;
use crate::gui::ui::{
    emit_installation_event, emit_log_message, InstallationProgress, InstallationStage,
    MessageLevel,
};
use crate::gui::utils::is_path_empty_or_nonexistent;
use idf_im_lib::ensure_path;
use tauri::AppHandle;

use log::{debug, error, info, warn};

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<crate::gui::app_state::AppState>();

    // Set installation flag
    if let Err(e) = set_installation_status(&app_handle, true) {
        return Err(e);
    }

    // Get the settings and save to a temporary config file
    let settings = get_settings_non_blocking(&app_handle)?;
    let temp_dir = std::env::temp_dir();
    let config_path = temp_dir.join(format!("eim_config_{}.toml", std::process::id()));

    // Make sure settings has proper values
    let mut settings_clone = settings.clone();
    settings_clone.config_file_save_path = Some(config_path.clone());
    settings_clone.non_interactive = Some(true);
    settings_clone.install_all_prerequisites = Some(true);

    // Validate installation path
    if !is_path_empty_or_nonexistent(
        settings_clone.path.clone().unwrap().to_str().unwrap(),
        &settings_clone.clone().idf_versions.unwrap(),
    ) {
        log::error!(
            "Installation path not available: {:?}",
            settings_clone.path.clone().unwrap()
        );

        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Installation path not available".to_string(),
                detail: Some(format!("Path: {:?}", settings_clone.path.clone().unwrap())),
                version: None,
            },
        );

        return Err(format!(
            "Installation path not available: {:?}",
            settings_clone.path.clone().unwrap()
        ));
    }

    // Save settings to temp file
    if let Err(e) = settings_clone.save() {
        log::error!("Failed to save temporary config: {}", e);

        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Failed to save temporary configuration".to_string(),
                detail: Some(e.to_string()),
                version: None,
            },
        );

        return Err(format!("Failed to save temporary config: {}", e));
    }

    log::info!("Saved temporary config to {}", config_path.display());

    // Get current executable path
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // Emit initial progress
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 0,
            message: "Starting installation process...".to_string(),
            detail: Some("Launching installer subprocess".to_string()),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        "Starting installation in separate process...".to_string(),
    );

    // Start the process with piped stdout and stderr
    let mut child = Command::new(current_exe)
        .arg("install")
        .arg("-n")
        .arg("true") // Non-interactive mode
        .arg("-a")
        .arg("true") // Install prerequisites
        .arg("-c")
        .arg(config_path.clone()) // Path to config file
        .stdout(Stdio::piped()) // Capture stdout
        .stderr(Stdio::piped()) // Capture stderr
        .spawn()
        .map_err(|e| {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Failed to start installer process".to_string(),
                    detail: Some(e.to_string()),
                    version: None,
                },
            );
            format!("Failed to start installer: {}", e)
        })?;

    // Set up monitor thread to read output and send to frontend
    let monitor_handle = app_handle.clone();
    let cfg_path = config_path.clone();
    let versions = settings_clone.idf_versions.clone().unwrap_or_default();

    emit_installation_plan(
        &app_handle,
        InstallationPlan {
            total_versions: versions.len(),
            versions: versions.clone(),
            current_version_index: None,
        },
    );

    std::thread::spawn(move || {
        let pid = child.id();

        // Progress tracking state
        let mut current_stage = InstallationStage::Checking;
        let mut current_percentage = 5u32;
        let mut current_version: Option<String> = None;
        let mut installation_started = false;
        let mut tools_phase = false;
        let mut completed_tools = 0u32;
        let mut total_tools = 0u32;

        // Get stdout and stderr
        let mut child = child; // Take ownership of child to wait on it
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        // Helper function to parse and emit progress based on log content
        let version_clone = versions.clone();
        let parse_and_emit_progress = move |handle: &AppHandle,
                                            line: &str,
                                            stage: &mut InstallationStage,
                                            percentage: &mut u32,
                                            current_ver: &mut Option<String>,
                                            tools_started: &mut bool,
                                            completed: &mut u32,
                                            total: &mut u32| {
            // Extract version information
            if line.contains("Selected idf version:") {
                if let Some(start) = line.find('[') {
                    if let Some(end) = line.find(']') {
                        let version_str = &line[start + 1..end];
                        let version = version_str.replace("\"", "").trim().to_string();
                        *current_ver = Some(version.clone());

                        if let Some(version_index) =
                            version_clone.iter().position(|v| v == &version)
                        {
                            emit_installation_plan(
                                &handle,
                                InstallationPlan {
                                    total_versions: version_clone.len(),
                                    versions: version_clone.clone(),
                                    current_version_index: Some(version_index),
                                },
                            );
                        }

                        emit_installation_event(
                            handle,
                            InstallationProgress {
                                stage: InstallationStage::Download,
                                percentage: 10,
                                message: format!("Starting ESP-IDF {} installation", version),
                                detail: Some("Preparing to download ESP-IDF".to_string()),
                                version: Some(version),
                            },
                        );

                        *stage = InstallationStage::Download;
                        *percentage = 10;
                        return;
                    }
                }
            }

            // Track major phases and estimate progress
            if line.contains("Checking for prerequisites") {
                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Prerequisites,
                        percentage: 8,
                        message: "Checking prerequisites...".to_string(),
                        detail: Some("Verifying system requirements".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *stage = InstallationStage::Prerequisites;
                *percentage = 8;
            } else if line.contains("Python sanity check") {
                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Prerequisites,
                        percentage: 12,
                        message: "Verifying Python installation...".to_string(),
                        detail: Some("Checking Python environment".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *percentage = 12;
            } else if line.contains("Cloning ESP-IDF") || line.contains("git clone") {
                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Download,
                        percentage: 15,
                        message: "Downloading ESP-IDF repository...".to_string(),
                        detail: Some("Cloning main repository".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *stage = InstallationStage::Download;
                *percentage = 15;
            } else if line.contains("Updating submodule") || line.contains("submodule update") {
                // Submodules phase - this is the long part (15-65%)
                let submodule_progress = std::cmp::min(65, *percentage + 2);
                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Download,
                        percentage: submodule_progress,
                        message: "Downloading submodules...".to_string(),
                        detail: Some("Processing ESP-IDF submodules".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *percentage = submodule_progress;
            } else if line.contains("Downloading tools:") {
                // Extract tools list if possible
                if let Some(start) = line.find('[') {
                    if let Some(end) = line.find(']') {
                        let tools_str = &line[start + 1..end];
                        let tools: Vec<&str> = tools_str.split(',').collect();
                        *total = tools.len() as u32;

                        emit_installation_event(
                            handle,
                            InstallationProgress {
                                stage: InstallationStage::Tools,
                                percentage: 65,
                                message: format!("Installing {} development tools...", total),
                                detail: Some("Preparing tools installation".to_string()),
                                version: current_ver.clone(),
                            },
                        );

                        *stage = InstallationStage::Tools;
                        *percentage = 65;
                        *tools_started = true;
                    }
                }
            } else if line.contains("Downloading tool:") && *tools_started {
                if let Some(tool_start) = line.find("tool:") {
                    let tool_name = line[tool_start + 5..].trim();
                    let tool_progress = 65 + (*completed * 20 / (*total).max(1));

                    emit_installation_event(
                        handle,
                        InstallationProgress {
                            stage: InstallationStage::Tools,
                            percentage: tool_progress,
                            message: format!("Downloading: {}", tool_name),
                            // detail: Some(format!("Tool {} of {}", *completed + 1, *total)), on windows the total information may be not available and then the of 0 looks weird
                            detail: Some(format!("Tool {}", *completed + 1)),
                            version: current_ver.clone(),
                        },
                    );
                    *percentage = tool_progress;
                }
            } else if line.contains("extracted tool:") || line.contains("Decompression completed") {
                *completed += 1;
                let tool_progress = 65 + (*completed * 20 / (*total).max(1));

                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Tools,
                        percentage: tool_progress.min(85),
                        // message: format!("Installed tool ({}/{})", *completed, *total), on windows the total information may be not avalible and then the /0 looks weird
                        message: format!("Installed tool ({})", *completed),
                        detail: Some("Tool installation completed".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *percentage = tool_progress.min(85);
            } else if line.contains("Python environment") || line.contains("Installing python") {
                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Python,
                        percentage: 90,
                        message: "Setting up Python environment...".to_string(),
                        detail: Some("Configuring Python dependencies".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *stage = InstallationStage::Python;
                *percentage = 90;
            } else if line.contains("Successfully installed IDF")
                || line.contains("Installation complete")
            {
                emit_installation_event(
                    handle,
                    InstallationProgress {
                        stage: InstallationStage::Complete,
                        percentage: 100,
                        message: "ESP-IDF installation completed successfully!".to_string(),
                        detail: Some("Installation finished".to_string()),
                        version: current_ver.clone(),
                    },
                );
                *stage = InstallationStage::Complete;
                *percentage = 100;
            }
        };

        // Monitor stdout in a separate thread
        let stdout_monitor = {
            let handle = monitor_handle.clone();
            let mut stage = current_stage.clone();
            let mut percentage = current_percentage;
            let mut current_ver = current_version.clone();
            let mut tools_started = tools_phase;
            let mut completed = completed_tools;
            let mut total = total_tools;

            std::thread::spawn(move || {
                let stdout_reader = BufReader::new(stdout);
                for line in stdout_reader.lines() {
                    if let Ok(line) = line {
                        // Parse progress and emit structured events
                        parse_and_emit_progress(
                            &handle,
                            &line,
                            &mut stage,
                            &mut percentage,
                            &mut current_ver,
                            &mut tools_started,
                            &mut completed,
                            &mut total,
                        );

                        // Skip debug/trace messages from logs
                        if line.contains("DEBUG") || line.contains("TRACE") {
                            continue;
                        }

                        // Clean up log message and emit
                        let clean_message = if let Some(pos) = line.find(" - ") {
                            let parts: Vec<&str> = line.splitn(2, " - ").collect();
                            if parts.len() > 1 {
                                parts[1].to_string()
                            } else {
                                line.clone()
                            }
                        } else {
                            line.clone()
                        };

                        emit_log_message(&handle, MessageLevel::Info, clean_message);
                        log::info!("Install process stdout: {}", line);
                    }
                }
            })
        };

        // Monitor stderr in a separate thread
        let stderr_monitor = {
            let handle = monitor_handle.clone();
            std::thread::spawn(move || {
                let stderr_reader = BufReader::new(stderr);
                for line in stderr_reader.lines() {
                    if let Ok(line) = line {
                        emit_log_message(&handle, MessageLevel::Error, line.clone());
                        log::error!("Install process stderr: {}", line);
                    }
                }
            })
        };

        // Wait for the child process to complete
        let status = match child.wait() {
            Ok(status) => {
                log::info!("Install process completed with status: {:?}", status);
                status
            }
            Err(e) => {
                log::error!("Failed to wait for install process: {}", e);

                emit_installation_event(
                    &monitor_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: "Installation process failed".to_string(),
                        detail: Some(e.to_string()),
                        version: current_version.clone(),
                    },
                );

                std::thread::sleep(std::time::Duration::from_secs(2));
                return;
            }
        };

        // Wait for stdout/stderr monitors to finish
        let _ = stdout_monitor.join();
        let _ = stderr_monitor.join();

        // Clean up installation status
        if let Err(e) = set_installation_status(&monitor_handle, false) {
            log::error!("Failed to update installation status: {}", e);
        }

        // Emit final completion or error event
        let success = status.success();
        log::info!("Installation completed with success={}", success);

        if success {
            emit_installation_event(
                &monitor_handle,
                InstallationProgress {
                    stage: InstallationStage::Complete,
                    percentage: 100,
                    message: "Installation completed successfully!".to_string(),
                    detail: Some(format!(
                        "All ESP-IDF versions installed: {}",
                        versions.join(", ")
                    )),
                    version: None,
                },
            );

            emit_log_message(
                &monitor_handle,
                MessageLevel::Success,
                "Installation process completed successfully".to_string(),
            );
        } else {
            let error_msg = format!(
                "Installation failed with exit code: {}",
                status.code().unwrap_or(-1)
            );

            emit_installation_event(
                &monitor_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Installation process failed".to_string(),
                    detail: Some(error_msg.clone()),
                    version: current_version,
                },
            );

            emit_log_message(&monitor_handle, MessageLevel::Error, error_msg);
        }

        // Clean up temporary config file
        let _ = std::fs::remove_file(&cfg_path);

        log::info!("Installation monitor thread completed");
    });

    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
    info!("Starting installation");

    // Set installation flag
    if let Err(e) = set_installation_status(&app_handle, true) {
        return Err(e);
    }

    let settings = get_locked_settings(&app_handle)?;

    // Check if versions are selected
    let versions = match &settings.idf_versions {
        Some(versions) if !versions.is_empty() => versions,
        _ => {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "No ESP-IDF versions selected".to_string(),
                    detail: Some("Please select at least one version to install".to_string()),
                    version: None,
                },
            );

            emit_log_message(
                &app_handle,
                MessageLevel::Warning,
                "No IDF versions were selected".to_string(),
            );

            set_installation_status(&app_handle, false)?;
            return Err("No IDF versions were selected".to_string());
        }
    };

    emit_installation_plan(
        &app_handle,
        InstallationPlan {
            total_versions: versions.len(),
            versions: versions.clone(),
            current_version_index: None,
        },
    );

    let total_versions = versions.len();
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 0,
            message: format!(
                "Starting installation of {} ESP-IDF version{}",
                total_versions,
                if total_versions == 1 { "" } else { "s" }
            ),
            detail: Some(format!("Versions: {}", versions.join(", "))),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        format!(
            "Starting batch installation of {} version(s): {}",
            total_versions,
            versions.join(", ")
        ),
    );

    // Install each version with progress tracking
    for (index, version) in versions.iter().enumerate() {
        emit_installation_plan(
            &app_handle,
            InstallationPlan {
                total_versions: versions.len(),
                versions: versions.clone(),
                current_version_index: Some(index),
            },
        );

        let version_start_percentage = (index * 90) / total_versions; // Each version gets equal share of 0-90%
        let version_end_percentage = ((index + 1) * 90) / total_versions;

        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Download,
                percentage: version_start_percentage as u32,
                message: format!("Starting ESP-IDF {} installation", version),
                detail: Some(format!(
                    "Version {} of {} - {}",
                    index + 1,
                    total_versions,
                    version
                )),
                version: Some(version.clone()),
            },
        );

        emit_log_message(
            &app_handle,
            MessageLevel::Info,
            format!(
                "Starting installation of ESP-IDF version {} ({}/{})",
                version,
                index + 1,
                total_versions
            ),
        );

        // Install single version
        match install_single_version(app_handle.clone(), &settings, version.clone()).await {
            Ok(_) => {
                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: if index < versions.len() - 1 {
                            InstallationStage::Configure
                        } else {
                            InstallationStage::Complete
                        },
                        percentage: version_end_percentage as u32,
                        message: format!("ESP-IDF {} installed successfully", version),
                        detail: Some(format!(
                            "Completed {} of {} versions, continuing...",
                            index + 1,
                            total_versions
                        )),
                        version: Some(version.clone()),
                    },
                );

                emit_log_message(
                    &app_handle,
                    MessageLevel::Success,
                    format!(
                        "ESP-IDF version {} installed successfully ({}/{})",
                        version,
                        index + 1,
                        total_versions
                    ),
                );
            }
            Err(e) => {
                error!("Failed to install version {}: {}", version, e);

                emit_installation_event(
                    &app_handle,
                    InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: format!("Failed to install ESP-IDF {}", version),
                        detail: Some(e.to_string()),
                        version: Some(version.clone()),
                    },
                );

                emit_log_message(
                    &app_handle,
                    MessageLevel::Error,
                    format!("Failed to install version {}: {}", version, e),
                );

                set_installation_status(&app_handle, false)?;
                return Err(format!(
                    "Installation failed for version {}: {}",
                    version, e
                ));
            }
        }
    }

    // Configuration phase - saving IDE JSON (90-95%)
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Configure,
            percentage: 90,
            message: "Configuring development environment...".to_string(),
            detail: Some("Saving IDE configuration".to_string()),
            version: None,
        },
    );

    // Save IDE JSON configuration
    let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
    let _ = ensure_path(&ide_json_path);

    match settings.save_esp_ide_json() {
        Ok(_) => {
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Configure,
                    percentage: 93,
                    message: "IDE configuration saved successfully".to_string(),
                    detail: Some(format!("Configuration saved to: {}", ide_json_path)),
                    version: None,
                },
            );

            emit_log_message(
                &app_handle,
                MessageLevel::Success,
                format!("IDE JSON file saved to: {}", ide_json_path),
            );
        }
        Err(e) => {
            // Don't fail the entire installation for IDE config save failure
            emit_installation_event(
                &app_handle,
                InstallationProgress {
                    stage: InstallationStage::Configure,
                    percentage: 93,
                    message: "Warning: IDE configuration save failed".to_string(),
                    detail: Some(e.to_string()),
                    version: None,
                },
            );

            emit_log_message(
                &app_handle,
                MessageLevel::Warning,
                format!("Failed to save IDE JSON file: {}", e),
            );
        }
    }

    // Final completion (95-100%)
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Configure,
            percentage: 97,
            message: "Finalizing installation...".to_string(),
            detail: Some("Completing setup process".to_string()),
            version: None,
        },
    );

    // Small delay to show finalization
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Complete!
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Complete,
            percentage: 100,
            message: format!(
                "All {} ESP-IDF version{} installed successfully!",
                total_versions,
                if total_versions == 1 { "" } else { "s" }
            ),
            detail: Some(format!(
                "Completed installation of: {}",
                versions.join(", ")
            )),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Success,
        format!(
            "Batch installation completed successfully - {} version(s) installed",
            total_versions
        ),
    );

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}
