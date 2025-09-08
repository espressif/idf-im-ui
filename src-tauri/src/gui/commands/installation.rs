use tauri::{AppHandle, Emitter, Manager};
use tempfile::TempDir;
use crate::gui::{app_state::{self, update_settings}, commands::idf_tools::setup_tools, get_installed_versions, ui::{emit_installation_event, emit_log_message, InstallationProgress, InstallationStage, MessageLevel}, utils::is_path_empty_or_nonexistent};
use std::{
  fs,
  io::{BufRead, BufReader},
  path::PathBuf,
  process::{Command, Stdio},
  sync::mpsc,
  thread,
};

use anyhow::{anyhow, Context, Result};
use idf_im_lib::{ensure_path, expand_tilde, idf_config::IdfConfig, offline_installer::{copy_idf_from_offline_archive, install_prerequisites_offline, use_offline_archive}, utils::{copy_dir_contents, extract_zst_archive, is_valid_idf_directory, parse_cmake_version}, version_manager::{get_default_config_path, prepare_settings_for_fix_idf_installation}, ProgressMessage};
use log::{debug, error, info, warn};
use serde_json::json;

use idf_im_lib::settings::Settings;
use crate::gui::{
  app_state::{get_locked_settings, get_settings_non_blocking, set_installation_status},
  commands,
  ui::{
      send_install_progress_message, send_message, send_simple_setup_message,
      ProgressBar,
  },
};

use super::{prequisites::{check_prequisites, install_prerequisites, python_install, python_sanity_check}, settings};

// Checks if an installation is currently in progress
#[tauri::command]
pub fn is_installing(app_handle: AppHandle) -> bool {
    app_state::is_installation_in_progress(&app_handle)
}

/// Prepares installation directories for a specific version
fn prepare_installation_directories(
  app_handle: &AppHandle,
  settings: &Settings,
  version: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
  let version_path = settings.path.as_ref().unwrap().as_path().join(version);

  ensure_path(version_path.to_str().unwrap())?;
  send_message(
      &app_handle,
      format!(
          "IDF installation folder created at: {}",
          version_path.display()
      ),
      "info".to_string(),
  );

  Ok(version_path)
}

/// Spawns a progress monitor thread for installation
pub fn spawn_progress_monitor(
  app_handle: AppHandle,
  version: String,
  rx: mpsc::Receiver<ProgressMessage>,
) -> thread::JoinHandle<()> {
  thread::spawn(move || {
      let progress = ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", version));

      while let Ok(message) = rx.recv() {
          match message {
              ProgressMessage::Finish => {
                  progress.update(100, None);
              }
              ProgressMessage::Update(value) => {
                  progress.update(value, Some(&format!("Downloading IDF {}...", version)));
              }
              ProgressMessage::SubmoduleUpdate((name, value)) => {
                  progress.update(
                      value,
                      Some(&format!("Downloading submodule {}... {}%", name, value))
                  );
              }
              ProgressMessage::SubmoduleFinish(_name) => {
                  progress.update(100, None);
              }
          }
      }
  })
}

/// Downloads the ESP-IDF for a specific version with detailed progress reporting
async fn download_idf(
app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    idf_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();

    emit_installation_event(app_handle, InstallationProgress {
        stage: InstallationStage::Download,
        percentage: 0,
        message: format!("Starting ESP-IDF {} download...", version),
        detail: Some("Preparing to clone repository".to_string()),
        version: Some(version.to_string()),
    });

    let app_handle_clone = app_handle.clone();
    let version_clone = version.to_string();

    let handle = thread::spawn(move || {
        let mut last_percentage = 0;
        let mut submodule_count = 0;
        let mut completed_submodules = 0;
        let mut total_estimated_submodules = 35; // ESP-IDF typically has ~35 submodules
        let mut main_repo_finished = false;
        let mut has_submodules = true;

        loop {
            match rx.recv() {
                Ok(ProgressMessage::Update(value)) => {
                    // Main repository clone progress (0-10%)
                    if value != last_percentage && (value - last_percentage) >= 10 {
                        last_percentage = value;
                        emit_installation_event(&app_handle_clone, InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: (value * 10 / 100) as u32, // Main clone: 0-10%
                            message: format!("Cloning ESP-IDF {} repository", version_clone),
                            detail: Some(format!("Repository: {}%", value)),
                            version: Some(version_clone.clone()),
                        });
                    }
                }

                Ok(ProgressMessage::SubmoduleUpdate((name, value))) => {
                    has_submodules = true;

                    // Update estimated total if we're seeing more submodules
                    if submodule_count > total_estimated_submodules {
                        total_estimated_submodules = submodule_count + 10;
                    }

                    // Submodules progress: 10-65% (55% total for submodules)
                    let submodule_base_progress = 10;
                    let submodule_range = 55;
                    let current_submodule_progress = (completed_submodules as f32 / total_estimated_submodules as f32) * submodule_range as f32;
                    let individual_progress = (value as f32 / 100.0) * (submodule_range as f32 / total_estimated_submodules as f32);
                    let total_progress = submodule_base_progress + current_submodule_progress as u32 + individual_progress as u32;

                    emit_installation_event(&app_handle_clone, InstallationProgress {
                        stage: InstallationStage::Download,
                        percentage: total_progress.min(65),
                        message: format!("Downloading submodule: {}",
                            name),
                        detail: Some(format!("Submodule {}/{} (est.) - {}%",
                            completed_submodules + 1, total_estimated_submodules, value)),
                        version: Some(version_clone.clone()),
                    });
                }

                Ok(ProgressMessage::SubmoduleFinish(name)) => {
                    has_submodules = true;
                    completed_submodules += 1;
                    submodule_count = completed_submodules; // Track actual count

                    // More frequent updates for submodule completion since it's the main work
                    let effective_total = total_estimated_submodules.max(completed_submodules);
                    let submodule_progress = 10 + (completed_submodules * 55 / effective_total);
                    // let submodule_progress = 10 + (completed_submodules * 55 / total_estimated_submodules.max(completed_submodules));

                    emit_installation_event(&app_handle_clone, InstallationProgress {
                        stage: InstallationStage::Download,
                        percentage: submodule_progress.min(65) as u32,
                        message: format!("Completed submodule: {}",
                            name.split('/').last().unwrap_or(&name).replace("_", " ")),
                        detail: Some(format!("Progress: {}/{} submodules", completed_submodules, total_estimated_submodules)),
                        version: Some(version_clone.clone()),
                    });

                    emit_log_message(&app_handle_clone, MessageLevel::Info,
                        format!("Submodule '{}' completed ({}/{})",
                            name, completed_submodules, total_estimated_submodules));
                }

                Ok(ProgressMessage::Finish) => {
                    main_repo_finished = true;

                    // If no submodules were processed, this means we're done with everything
                    if !has_submodules {
                        emit_installation_event(&app_handle_clone, InstallationProgress {
                            stage: InstallationStage::Extract,
                            percentage: 65,
                            message: "ESP-IDF download completed (no submodules)".to_string(),
                            detail: Some("Repository cloned successfully".to_string()),
                            version: Some(version_clone.clone()),
                        });
                        break;
                    }
                    // If submodules were processed, check if we're actually done
                    else if completed_submodules > 0 {
                        // We have processed some submodules, likely we're done
                        emit_installation_event(&app_handle_clone, InstallationProgress {
                            stage: InstallationStage::Extract,
                            percentage: 65,
                            message: "ESP-IDF download completed".to_string(),
                            detail: Some(format!("Repository and {} submodules ready", completed_submodules)),
                            version: Some(version_clone.clone()),
                        });
                        break;
                    }
                    // If we got Finish but haven't seen submodules yet, just note main repo is done
                    else {
                        emit_installation_event(&app_handle_clone, InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: 10,
                            message: "Main repository cloned, preparing submodules...".to_string(),
                            detail: Some("Waiting for submodule updates".to_string()),
                            version: Some(version_clone.clone()),
                        });
                        // Don't break - keep waiting for submodules
                    }
                }

                Err(_) => {
                    // Channel closed - check our state and finish appropriately
                    if main_repo_finished {
                        let final_percentage = if has_submodules { 65 } else { 65 };
                        emit_installation_event(&app_handle_clone, InstallationProgress {
                            stage: InstallationStage::Extract,
                            percentage: final_percentage,
                            message: "ESP-IDF download completed".to_string(),
                            detail: Some(if has_submodules {
                                format!("Repository and {} submodules processed", completed_submodules)
                            } else {
                                "Repository cloned successfully".to_string()
                            }),
                            version: Some(version_clone.clone()),
                        });
                    }
                    break;
                }
            }
        }
    });

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        format!("Cloning ESP-IDF {} repository from {}",
            version,
            settings.idf_mirror.as_deref().unwrap_or("default mirror")
        ),
    );

    let result = idf_im_lib::get_esp_idf(
        idf_path.to_str().unwrap(),
        settings.repo_stub.as_deref(),
        version,
        settings.idf_mirror.as_deref(),
        settings.recurse_submodules.unwrap_or_default(),
        tx,
    );

    handle.join().unwrap();

    match result {
        Ok(_) => {
            emit_installation_event(app_handle, InstallationProgress {
                stage: InstallationStage::Extract,
                percentage: 70,
                message: format!("ESP-IDF {} ready for tools installation", version),
                detail: Some(format!("Location: {}", idf_path.display())),
                version: Some(version.to_string()),
            });

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                format!("ESP-IDF {} downloaded successfully: {}", version, idf_path.display()),
            );
            Ok(())
        }
        Err(e) => {
            emit_installation_event(app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: format!("Failed to download ESP-IDF {}", version),
                detail: Some(e.to_string()),
                version: Some(version.to_string()),
            });
            Err(e.into())
        }
    }
}

/// Installs a single ESP-IDF version
pub async fn install_single_version(
  app_handle: AppHandle,
  settings: &Settings,
  version: String,
) -> Result<(), Box<dyn std::error::Error>> {
  info!("Installing IDF version: {}", version);

  let paths = settings.get_version_paths(&version).map_err(|err| {
    error!("Failed to get version paths: {}", err);
    err.to_string()
  })?;


  if paths.using_existing_idf {
    info!("Using existing IDF directory: {}", paths.idf_path.display());
    send_message(
        &app_handle,
        format!("Using existing IDF directory: {}", paths.idf_path.display()),
        "info".to_string(),
    );

    debug!("Using IDF version: {}", paths.actual_version);
  } else {
    download_idf(&app_handle, settings, &version, &paths.idf_path).await?;
  }


  let export_vars = setup_tools(&app_handle, settings, &paths.idf_path, &paths.actual_version).await?;

  idf_im_lib::single_version_post_install(
      &paths.activation_script_path.to_str().unwrap(),
      paths.idf_path.to_str().unwrap(),
      &paths.actual_version,
      paths.tool_install_directory.to_str().unwrap(),
      export_vars,
      paths.python_venv_path.to_str(),
      None, // env_vars
  );

  Ok(())
}

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
    if !is_path_empty_or_nonexistent(settings_clone.path.clone().unwrap().to_str().unwrap(), &settings_clone.clone().idf_versions.unwrap()) {
        log::error!("Installation path not available: {:?}", settings_clone.path.clone().unwrap());

        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: "Installation path not available".to_string(),
            detail: Some(format!("Path: {:?}", settings_clone.path.clone().unwrap())),
            version: None,
        });

        return Err(format!("Installation path not available: {:?}", settings_clone.path.clone().unwrap()));
    }

    // Save settings to temp file
    if let Err(e) = settings_clone.save() {
        log::error!("Failed to save temporary config: {}", e);

        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: "Failed to save temporary configuration".to_string(),
            detail: Some(e.to_string()),
            version: None,
        });

        return Err(format!("Failed to save temporary config: {}", e));
    }

    log::info!("Saved temporary config to {}", config_path.display());

    // Get current executable path
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // Emit initial progress
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: "Starting installation process...".to_string(),
        detail: Some("Launching installer subprocess".to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        "Starting installation in separate process...".to_string());

    // Start the process with piped stdout and stderr
    let mut child = Command::new(current_exe)
        .arg("install")
        .arg("-n").arg("true")             // Non-interactive mode
        .arg("-a").arg("true")             // Install prerequisites
        .arg("-c").arg(config_path.clone())    // Path to config file
        .stdout(Stdio::piped())            // Capture stdout
        .stderr(Stdio::piped())            // Capture stderr
        .spawn()
        .map_err(|e| {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Failed to start installer process".to_string(),
                detail: Some(e.to_string()),
                version: None,
            });
            format!("Failed to start installer: {}", e)
        })?;

    // Set up monitor thread to read output and send to frontend
    let monitor_handle = app_handle.clone();
    let cfg_path = config_path.clone();
    let versions = settings_clone.idf_versions.clone().unwrap_or_default();

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
        let parse_and_emit_progress = |handle: &AppHandle, line: &str,
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
                        let version_str = &line[start+1..end];
                        let version = version_str.replace("\"", "").trim().to_string();
                        *current_ver = Some(version.clone());

                        emit_installation_event(handle, InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: 10,
                            message: format!("Starting ESP-IDF {} installation", version),
                            detail: Some("Preparing to download ESP-IDF".to_string()),
                            version: Some(version),
                        });

                        *stage = InstallationStage::Download;
                        *percentage = 10;
                        return;
                    }
                }
            }

            // Track major phases and estimate progress
            if line.contains("Checking for prerequisites") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Prerequisites,
                    percentage: 8,
                    message: "Checking prerequisites...".to_string(),
                    detail: Some("Verifying system requirements".to_string()),
                    version: current_ver.clone(),
                });
                *stage = InstallationStage::Prerequisites;
                *percentage = 8;

            } else if line.contains("Python sanity check") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Prerequisites,
                    percentage: 12,
                    message: "Verifying Python installation...".to_string(),
                    detail: Some("Checking Python environment".to_string()),
                    version: current_ver.clone(),
                });
                *percentage = 12;

            } else if line.contains("Cloning ESP-IDF") || line.contains("git clone") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Download,
                    percentage: 15,
                    message: "Downloading ESP-IDF repository...".to_string(),
                    detail: Some("Cloning main repository".to_string()),
                    version: current_ver.clone(),
                });
                *stage = InstallationStage::Download;
                *percentage = 15;

            } else if line.contains("Updating submodule") || line.contains("submodule update") {
                // Submodules phase - this is the long part (15-65%)
                let submodule_progress = std::cmp::min(65, *percentage + 2);
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Download,
                    percentage: submodule_progress,
                    message: "Downloading submodules...".to_string(),
                    detail: Some("Processing ESP-IDF submodules".to_string()),
                    version: current_ver.clone(),
                });
                *percentage = submodule_progress;

            } else if line.contains("Downloading tools:") {
                // Extract tools list if possible
                if let Some(start) = line.find('[') {
                    if let Some(end) = line.find(']') {
                        let tools_str = &line[start+1..end];
                        let tools: Vec<&str> = tools_str.split(',').collect();
                        *total = tools.len() as u32;

                        emit_installation_event(handle, InstallationProgress {
                            stage: InstallationStage::Tools,
                            percentage: 65,
                            message: format!("Installing {} development tools...", total),
                            detail: Some("Preparing tools installation".to_string()),
                            version: current_ver.clone(),
                        });

                        *stage = InstallationStage::Tools;
                        *percentage = 65;
                        *tools_started = true;
                    }
                }

            } else if line.contains("Downloading tool:") && *tools_started {
                if let Some(tool_start) = line.find("tool:") {
                    let tool_name = line[tool_start + 5..].trim();
                    let tool_progress = 65 + (*completed * 20 / (*total).max(1));

                    emit_installation_event(handle, InstallationProgress {
                        stage: InstallationStage::Tools,
                        percentage: tool_progress,
                        message: format!("Downloading: {}", tool_name),
                        // detail: Some(format!("Tool {} of {}", *completed + 1, *total)), on windows the total information may be not available and then the of 0 looks weird
                        detail: Some(format!("Tool {}", *completed + 1)),
                        version: current_ver.clone(),
                    });
                    *percentage = tool_progress;
                }

            } else if line.contains("extracted tool:") || line.contains("Decompression completed") {
                *completed += 1;
                let tool_progress = 65 + (*completed * 20 / (*total).max(1));

                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: tool_progress.min(85),
                    // message: format!("Installed tool ({}/{})", *completed, *total), on windows the total information may be not avalible and then the /0 looks weird
                    message: format!("Installed tool ({})", *completed),
                    detail: Some("Tool installation completed".to_string()),
                    version: current_ver.clone(),
                });
                *percentage = tool_progress.min(85);

            } else if line.contains("Python environment") || line.contains("Installing python") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Python,
                    percentage: 90,
                    message: "Setting up Python environment...".to_string(),
                    detail: Some("Configuring Python dependencies".to_string()),
                    version: current_ver.clone(),
                });
                *stage = InstallationStage::Python;
                *percentage = 90;

            } else if line.contains("Successfully installed IDF") || line.contains("Installation complete") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Complete,
                    percentage: 100,
                    message: "ESP-IDF installation completed successfully!".to_string(),
                    detail: Some("Installation finished".to_string()),
                    version: current_ver.clone(),
                });
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
                        parse_and_emit_progress(&handle, &line, &mut stage, &mut percentage,
                                              &mut current_ver, &mut tools_started, &mut completed, &mut total);

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
            },
            Err(e) => {
                log::error!("Failed to wait for install process: {}", e);

                emit_installation_event(&monitor_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Installation process failed".to_string(),
                    detail: Some(e.to_string()),
                    version: current_version.clone(),
                });

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
            emit_installation_event(&monitor_handle, InstallationProgress {
                stage: InstallationStage::Complete,
                percentage: 100,
                message: "Installation completed successfully!".to_string(),
                detail: Some(format!("All ESP-IDF versions installed: {}", versions.join(", "))),
                version: None,
            });

            emit_log_message(&monitor_handle, MessageLevel::Success,
                "Installation process completed successfully".to_string());
        } else {
            let error_msg = format!("Installation failed with exit code: {}", status.code().unwrap_or(-1));

            emit_installation_event(&monitor_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Installation process failed".to_string(),
                detail: Some(error_msg.clone()),
                version: current_version,
            });

            emit_log_message(&monitor_handle, MessageLevel::Error, error_msg);
        }

        // Clean up temporary config file
        let _ = std::fs::remove_file(&cfg_path);

        log::info!("Installation monitor thread completed");
    });

    Ok(())
}

// Helper function to check if a process is running on Windows
#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> bool {
    use std::process::Command;

    // Check if process exists using tasklist
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output();

    match output {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str.contains(&pid.to_string())
        },
        Err(_) => false
    }
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
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "No ESP-IDF versions selected".to_string(),
                detail: Some("Please select at least one version to install".to_string()),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Warning,
                "No IDF versions were selected".to_string());

            set_installation_status(&app_handle, false)?;
            return Err("No IDF versions were selected".to_string());
        }
    };

    let total_versions = versions.len();
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: format!("Starting installation of {} ESP-IDF version{}",
            total_versions, if total_versions == 1 { "" } else { "s" }),
        detail: Some(format!("Versions: {}", versions.join(", "))),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        format!("Starting batch installation of {} version(s): {}",
            total_versions, versions.join(", ")));

    // Install each version with progress tracking
    for (index, version) in versions.iter().enumerate() {
        let version_start_percentage = (index * 90) / total_versions; // Each version gets equal share of 0-90%
        let version_end_percentage = ((index + 1) * 90) / total_versions;

        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Download,
            percentage: version_start_percentage as u32,
            message: format!("Starting ESP-IDF {} installation", version),
            detail: Some(format!("Version {} of {} - {}", index + 1, total_versions, version)),
            version: Some(version.clone()),
        });

        emit_log_message(&app_handle, MessageLevel::Info,
            format!("Starting installation of ESP-IDF version {} ({}/{})",
                version, index + 1, total_versions));

        // Install single version
        match install_single_version(app_handle.clone(), &settings, version.clone()).await {
            Ok(_) => {
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Complete,
                    percentage: version_end_percentage as u32,
                    message: format!("ESP-IDF {} installed successfully", version),
                    detail: Some(format!("Completed {} of {} versions", index + 1, total_versions)),
                    version: Some(version.clone()),
                });

                emit_log_message(&app_handle, MessageLevel::Success,
                    format!("ESP-IDF version {} installed successfully ({}/{})",
                        version, index + 1, total_versions));
            }
            Err(e) => {
                error!("Failed to install version {}: {}", version, e);

                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: format!("Failed to install ESP-IDF {}", version),
                    detail: Some(e.to_string()),
                    version: Some(version.clone()),
                });

                emit_log_message(&app_handle, MessageLevel::Error,
                    format!("Failed to install version {}: {}", version, e));

                set_installation_status(&app_handle, false)?;
                return Err(format!("Installation failed for version {}: {}", version, e));
            }
        }
    }

    // Configuration phase - saving IDE JSON (90-95%)
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Configure,
        percentage: 90,
        message: "Configuring development environment...".to_string(),
        detail: Some("Saving IDE configuration".to_string()),
        version: None,
    });

    // Save IDE JSON configuration
    let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
    let _ = ensure_path(&ide_json_path);

    match settings.save_esp_ide_json() {
        Ok(_) => {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Configure,
                percentage: 93,
                message: "IDE configuration saved successfully".to_string(),
                detail: Some(format!("Configuration saved to: {}", ide_json_path)),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Success,
                format!("IDE JSON file saved to: {}", ide_json_path));
        }
        Err(e) => {
            // Don't fail the entire installation for IDE config save failure
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Configure,
                percentage: 93,
                message: "Warning: IDE configuration save failed".to_string(),
                detail: Some(e.to_string()),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Warning,
                format!("Failed to save IDE JSON file: {}", e));
        }
    }

    // Final completion (95-100%)
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Configure,
        percentage: 97,
        message: "Finalizing installation...".to_string(),
        detail: Some("Completing setup process".to_string()),
        version: None,
    });

    // Small delay to show finalization
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Complete!
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Complete,
        percentage: 100,
        message: format!("All {} ESP-IDF version{} installed successfully!",
            total_versions, if total_versions == 1 { "" } else { "s" }),
        detail: Some(format!("Completed installation of: {}", versions.join(", "))),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Success,
        format!("Batch installation completed successfully - {} version(s) installed", total_versions));

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

/// Starts a simple setup process that automates the installation
#[tauri::command]
pub async fn start_simple_setup(app_handle: tauri::AppHandle) {
    println!("Starting simple setup");
    let settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(e) => {
            emit_log_message(&app_handle, MessageLevel::Error, e);
            return;
        }
    };

    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: "Starting installation...".to_string(),
        detail: None,
        version: None,
    });

    // Check prerequisites
    let mut prerequisites = check_prequisites(app_handle.clone());
    let os = std::env::consts::OS.to_lowercase();

    // Install prerequisites on Windows if needed
    if !prerequisites.is_empty() && os == "windows" {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Prerequisites,
            percentage: 5,
            message: "Installing prerequisites...".to_string(),
            detail: Some(format!("Missing: {}", prerequisites.join(", "))),
            version: None,
        });

        if !install_prerequisites(app_handle.clone()) {
            prerequisites = check_prequisites(app_handle.clone());
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Failed to install prerequisites".to_string(),
                detail: Some(format!("Missing: {}", prerequisites.join(", "))),
                version: None,
            });
            return;
        }

        prerequisites = check_prequisites(app_handle.clone());
    }

    // Check if any prerequisites are still missing
    if !prerequisites.is_empty() {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: "Prerequisites missing".to_string(),
            detail: Some(format!("Please install: {}", prerequisites.join(", "))),
            version: None,
        });
        return;
    }

    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Prerequisites,
        percentage: 10,
        message: "Prerequisites verified".to_string(),
        detail: None,
        version: None,
    });

    // Check for Python
    let mut python_found = python_sanity_check(app_handle.clone(), None);

    // Install Python on Windows if needed
    if !python_found && os == "windows" {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Python,
            percentage: 15,
            message: "Installing Python...".to_string(),
            detail: None,
            version: None,
        });

        if !python_install(app_handle.clone()) {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Failed to install Python".to_string(),
                detail: Some("Python installation failed".to_string()),
                version: None,
            });
            return;
        }

        python_found = python_sanity_check(app_handle.clone(), None);
    }

    // Check if Python is still not found
    if !python_found {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: "Python not found".to_string(),
            detail: Some("Please install Python 3.8 or later manually".to_string()),
            version: None,
        });
        return;
    }

    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Python,
        percentage: 20,
        message: "Python environment ready".to_string(),
        detail: None,
        version: None,
    });

    // Check for IDF versions
    if settings.idf_versions.is_none() {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Configure,
            percentage: 25,
            message: "Fetching available ESP-IDF versions...".to_string(),
            detail: None,
            version: None,
        });

        let versions = settings::get_idf_versions(app_handle.clone()).await;

        if versions.is_empty() {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Failed to fetch ESP-IDF versions".to_string(),
                detail: Some("Could not retrieve version list from server".to_string()),
                version: None,
            });
            return;
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
                emit_log_message(&app_handle, MessageLevel::Info,
                    format!("Selected ESP-IDF version: {}", version));

                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Configure,
                    percentage: 30,
                    message: format!("ESP-IDF {} selected", version),
                    detail: None,
                    version: Some(version.clone()),
                });
            }
            Err(e) => {
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: "Failed to configure ESP-IDF version".to_string(),
                    detail: Some(e.to_string()),
                    version: None,
                });
                return;
            }
        }
    }

    // Start installation
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Download,
        percentage: 35,
        message: "Starting ESP-IDF installation...".to_string(),
        detail: None,
        version: settings.idf_versions.as_ref().and_then(|v| v.first().cloned()),
    });

    let _res = start_installation(app_handle.clone()).await;
}

#[tauri::command]
pub async fn fix_installation(app_handle: AppHandle, id: String) -> Result<(), String> {
    debug!("Fixing installation with id {}", id);

    // Set installation flag to indicate installation is running
    set_installation_status(&app_handle, true)?;

    // Initial progress - checking installation
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: "Checking installation to repair...".to_string(),
        detail: Some(format!("Looking up installation ID: {}", id)),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        format!("Starting repair process for installation: {}", id));

    let versions = get_installed_versions();
    let installation = match versions.iter().find(|v| v.id == id) {
        Some(inst) => {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Checking,
                percentage: 10,
                message: format!("Found installation: ESP-IDF {}", inst.name),
                detail: Some(format!("Path: {}", inst.path)),
                version: Some(inst.name.clone()),
            });

            emit_log_message(&app_handle, MessageLevel::Info,
                format!("Found installation {} at: {}", inst.name, inst.path));

            inst
        }
        None => {
            let error_msg = format!("Installation with ID {} not found", id);
            error!("{}", error_msg);

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Installation not found".to_string(),
                detail: Some(error_msg.clone()),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Error, error_msg.clone());
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }
    };

    // Preparing settings
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 20,
        message: "Preparing repair configuration...".to_string(),
        detail: Some("Setting up installation parameters".to_string()),
        version: Some(installation.name.clone()),
    });

    let mut settings = match prepare_settings_for_fix_idf_installation(PathBuf::from(installation.path.clone())).await {
        Ok(settings) => {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Prerequisites,
                percentage: 30,
                message: "Configuration prepared successfully".to_string(),
                detail: Some("Ready to begin repair process".to_string()),
                version: Some(installation.name.clone()),
            });

            emit_log_message(&app_handle, MessageLevel::Success,
                "Repair configuration prepared successfully".to_string());

            settings
        }
        Err(e) => {
            let error_msg = format!("Failed to prepare settings for repair: {}", e);
            error!("{}", error_msg);

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: "Failed to prepare repair configuration".to_string(),
                detail: Some(e.to_string()),
                version: Some(installation.name.clone()),
            });

            emit_log_message(&app_handle, MessageLevel::Error, error_msg.clone());
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }
    };

    // Get the config path for IDE configuration
    let config_path = get_default_config_path();

    // Starting actual repair (reinstallation)
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Download,
        percentage: 35,
        message: format!("Starting ESP-IDF {} repair...", installation.name),
        detail: Some("Beginning reinstallation process".to_string()),
        version: Some(installation.name.clone()),
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        format!("Starting repair installation for ESP-IDF {}", installation.name));

    // The actual repair process - this will generate detailed progress events
    match install_single_version(app_handle.clone(), &settings, installation.name.clone()).await {
        Ok(_) => {
            emit_log_message(&app_handle, MessageLevel::Success,
                format!("Successfully repaired ESP-IDF {} installation", installation.name));

            info!("Successfully fixed installation {}", id);
        }
        Err(e) => {
            let error_msg = format!("Failed to repair installation: {}", e);
            error!("{}", error_msg);

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: format!("ESP-IDF {} repair failed", installation.name),
                detail: Some(e.to_string()),
                version: Some(installation.name.clone()),
            });

            emit_log_message(&app_handle, MessageLevel::Error, error_msg.clone());
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }
    }

    // Configure IDE configuration update
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Configure,
        percentage: 90,
        message: "Updating IDE configuration...".to_string(),
        detail: Some("Saving installation information".to_string()),
        version: Some(installation.name.clone()),
    });

    // The installation should have been added back by single_version_post_install()
    // but let's ensure the IDE configuration is properly saved by reconstructing
    // the settings with the right configuration
    let paths = settings.get_version_paths(&installation.name).map_err(|err| {
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
    let ide_json_path = updated_settings.esp_idf_json_path.clone().unwrap_or_default();

    // Try to read the current IDE config to see what's in it
    match IdfConfig::from_file(&config_path) {
        Ok(current_config) => {
            info!("Current IDE config has {} installed versions", current_config.idf_installed.len());
            for installed in &current_config.idf_installed {
                info!("Installed version: {} (ID: {})", installed.name, installed.id);
            }

            // Check if our repaired installation is already there
            let found_installation = current_config.idf_installed.iter()
                .find(|inst| inst.name == installation.name && inst.path == installation.path);

            if found_installation.is_some() {
                info!("Repaired installation already found in IDE config - no need to save again");
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Configure,
                    percentage: 95,
                    message: "IDE configuration already updated".to_string(),
                    detail: Some("Installation found in configuration".to_string()),
                    version: Some(installation.name.clone()),
                });
            } else {
                info!("Repaired installation not found in IDE config - trying to save");
                // Try to save the configuration
                match updated_settings.save_esp_ide_json() {
                    Ok(_) => {
                        emit_installation_event(&app_handle, InstallationProgress {
                            stage: InstallationStage::Configure,
                            percentage: 95,
                            message: "IDE configuration saved successfully".to_string(),
                            detail: Some(format!("Configuration saved to: {}", ide_json_path)),
                            version: Some(installation.name.clone()),
                        });

                        emit_log_message(&app_handle, MessageLevel::Success,
                            format!("IDE JSON file updated at: {}", ide_json_path));

                        info!("IDE JSON saved to {}", ide_json_path);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to save IDE configuration: {}", e);
                        warn!("{}", error_msg);

                        emit_installation_event(&app_handle, InstallationProgress {
                            stage: InstallationStage::Configure,
                            percentage: 95,
                            message: "Warning: IDE configuration save failed".to_string(),
                            detail: Some(e.to_string()),
                            version: Some(installation.name.clone()),
                        });

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
                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Configure,
                        percentage: 95,
                        message: "IDE configuration saved successfully".to_string(),
                        detail: Some(format!("Configuration saved to: {}", ide_json_path)),
                        version: Some(installation.name.clone()),
                    });

                    emit_log_message(&app_handle, MessageLevel::Success,
                        format!("IDE JSON file updated at: {}", ide_json_path));

                    info!("IDE JSON saved to {}", ide_json_path);
                }
                Err(e) => {
                    let error_msg = format!("Failed to save IDE configuration: {}", e);
                    warn!("{}", error_msg);

                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Configure,
                        percentage: 95,
                        message: "Warning: IDE configuration save failed".to_string(),
                        detail: Some(e.to_string()),
                        version: Some(installation.name.clone()),
                    });

                    emit_log_message(&app_handle, MessageLevel::Warning, error_msg);
                }
            }
        }
    }

    // Final completion
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Complete,
        percentage: 100,
        message: format!("ESP-IDF {} repair completed successfully!", installation.name),
        detail: Some(format!("Installation repaired at: {}", installation.path)),
        version: Some(installation.name.clone()),
    });

    emit_log_message(&app_handle, MessageLevel::Success,
        format!("Repair process completed successfully for ESP-IDF {}", installation.name));

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

#[tauri::command]
pub async fn start_offline_installation(app_handle: AppHandle, archives: Vec<String>, install_path: String) -> Result<(), String> {
    // Implementation goes here
    if archives.len() == 0 {
        return Err("No archives provided for offline installation".to_string());
    }
    for archive in &archives {
        let archive_path = std::path::PathBuf::from(archive);
        if !archive_path.try_exists().unwrap_or(false) {
            return Err(format!("Archive file does not exist: {}", archive));
        }
        let offline_archive_dir = TempDir::new().expect("Failed to create temporary directory");

        let mut settings = get_settings_non_blocking(&app_handle)?;
        if install_path != "" && is_path_empty_or_nonexistent(&install_path, &[]) { //TODO: parse version from archive name
            settings.path = Some(PathBuf::from(install_path.clone()));
        }
        settings.use_local_archive = Some(archive_path);

        settings = match use_offline_archive(settings, &offline_archive_dir){
          Ok(updated_config) => updated_config,
          Err(err) => {
            error!("Failed to use offline archive: {}", err);
            return Err(err);
          }
        };

        // install prerequisites offline
        if std::env::consts::OS == "windows" {
          match install_prerequisites_offline(&offline_archive_dir){
            Ok(_) => {
                info!("Successfully installed prerequisites from offline archive.");
                settings.skip_prerequisites_check = Some(true);
            }
            Err(err) => {
                return Err(format!("Failed to install prerequisites from offline archive: {}", err));
            }
          }
        } else {
          let prereq =  check_prequisites(app_handle.clone());
          if prereq.len() > 0 {
              return Err(format!("Missing prerequisites: {}", prereq.join(", ")));
          }
          let mut python_sane = true;
          for result in idf_im_lib::python_utils::python_sanity_check(None) {
              match result {
                  Ok(_) => {}
                  Err(err) => {
                      python_sane = false;
                      send_message(
                          &app_handle,
                          format!("Python sanity check failed: {}", err),
                          "warning".to_string(),
                      );
                      warn!("{:?}", err)
                  }
              }
          }
          if !python_sane {
              return Err("Python sanity check failed".to_string());
          }
      }
      match copy_idf_from_offline_archive(&offline_archive_dir, &settings) {
          Ok(_) => {
              info!("Successfully copied ESP-IDF from offline archive.");
          }
          Err(err) => {
              error!("Failed to copy ESP-IDF from offline archive: {}", err);
              return Err(err);
          }
      }
      for idf_version in settings.idf_versions.clone().unwrap() {
        let paths = match settings.get_version_paths(&idf_version) {
            Ok(paths) => paths,
            Err(err) => {
                error!("Failed to get version paths: {}", err);
                return Err(err.to_string());
            }
        };
        settings.idf_path = Some(paths.idf_path.clone());
        idf_im_lib::add_path_to_path(paths.idf_path.to_str().unwrap());

        match copy_dir_contents(
            &offline_archive_dir.path().join("dist"),
          &paths.tool_download_directory,
        ) {
          Ok(_) => {}
          Err(err) => {
              error!("Failed to copy tool directory: {}", err);
              return Err(err.to_string());
          }
        }
        idf_im_lib::add_path_to_path(paths.tool_install_directory.to_str().unwrap());

        let export_vars = match setup_tools(&app_handle, &settings, &paths.idf_path, &paths.actual_version).await {
          Ok(vars) => vars,
          Err(err) => {
              error!("Failed to setup tools: {}", err);
              return Err(err.to_string());
          }
        };

        match idf_im_lib::python_utils::install_python_env(
            &paths.actual_version,
            &paths.tool_install_directory,
            true, //TODO: actually read from config
            &paths.idf_path,
            &settings.idf_features.clone().unwrap_or_default(),
            Some(offline_archive_dir.path())
        )
        .await
        {
            Ok(_) => {
                info!("Python environment installed");
            }
            Err(err) => {
                error!("Failed to install Python environment: {}", err);
                return Err(err.to_string());
            }
        };

        idf_im_lib::single_version_post_install(
            &paths.activation_script_path.to_str().unwrap(),
            paths.idf_path.to_str().unwrap(),
            &paths.actual_version,
            paths.tool_install_directory.to_str().unwrap(),
            export_vars,
            paths.python_venv_path.to_str(),
            None, // env_vars
        );

        let ide_conf_path_tmp = PathBuf::from(&settings.esp_idf_json_path.clone().unwrap_or_default());
        debug!("IDE configuration path: {}", ide_conf_path_tmp.display());
        match ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to create IDE configuration directory: {}", err);
                return Err(err.to_string());
            }
        }
        match settings.save_esp_ide_json() {
            Ok(_) => debug!("IDE configuration saved."),
            Err(err) => {
                error!("Failed to save IDE configuration: {}", err);
                return Err(err.to_string());
            }
        };
      }
    }

    Ok(())
}
