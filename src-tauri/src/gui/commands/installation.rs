use tauri::{AppHandle, Emitter, Manager};
use crate::gui::{app_state::{self, update_settings}, commands::idf_tools::setup_tools, ui::{emit_installation_event, emit_log_message, InstallationProgress, InstallationStage, MessageLevel}, utils::is_path_empty_or_nonexistent};
use std::{
  fs,
  io::{BufRead, BufReader},
  path::PathBuf,
  process::{Command, Stdio},
  sync::mpsc,
  thread,
};

use anyhow::{anyhow, Context, Result};
use idf_im_lib::{ensure_path, expand_tilde, utils::{is_valid_idf_directory, parse_cmake_version}, ProgressMessage};
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
                    let submodule_progress = 10 + (completed_submodules * 55 / total_estimated_submodules.max(completed_submodules));

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
async fn install_single_version(
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

  if !is_path_empty_or_nonexistent(settings_clone.path.clone().unwrap().to_str().unwrap(), &settings_clone.clone().idf_versions.unwrap()) {
    log::error!("Installation path not avalible: {:?}", settings_clone.path.clone().unwrap());
    send_simple_setup_message(
      &app_handle,
      12,
      format!("Installation path not avalible"),
    );
    return Err(format!("Installation path not avalible: {:?}", settings_clone.path.clone().unwrap()));
  }

  // Save settings to temp file
  if let Err(e) = settings_clone.save() {
      log::error!("Failed to save temporary config: {}", e);
      return Err(format!("Failed to save temporary config: {}", e));
  }

  log::info!("Saved temporary config to {}", config_path.display());

  // Get current executable path
  let current_exe = std::env::current_exe()
      .map_err(|e| format!("Failed to get current executable path: {}", e))?;

  send_message(
      &app_handle,
      "Starting installation in separate process...".to_string(),
      "info".to_string(),
  );

  // Start the process with piped stdout and stderr
  let mut child = Command::new(current_exe)
      .arg("install")
      .arg("-n").arg("true")             // Non-interactive mode
      .arg("-a").arg("true")             // Install prerequisites
      .arg("-c").arg(config_path.clone())    // Path to config file
      .stdout(Stdio::piped())            // Capture stdout
      .stderr(Stdio::piped())            // Capture stderr
      .spawn()
      .map_err(|e| format!("Failed to start installer: {}", e))?;

  // Set up monitor thread to read output and send to frontend
  let monitor_handle = app_handle.clone();
  let cfg_path = config_path.clone();
  std::thread::spawn(move || {
      let pid = child.id();

      // Get stdout and stderr
      let mut child = child; // Take ownership of child to wait on it
      let stdout = child.stdout.take().expect("Failed to capture stdout");
      let stderr = child.stderr.take().expect("Failed to capture stderr");

      // Monitor stdout in a separate thread
      let stdout_monitor = {
          let handle = monitor_handle.clone();
          std::thread::spawn(move || {
              let stdout_reader = BufReader::new(stdout);
              for line in stdout_reader.lines() {
                  if let Ok(line) = line {
                      // Send output to frontend
                      let _ = handle.emit("installation_output", json!({
                          "type": "stdout",
                          "message": line
                      }));

                      // Also log it
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
                      // Send output to frontend
                      let _ = handle.emit("installation_output", json!({
                          "type": "stderr",
                          "message": line
                      }));

                      // Also log it
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
              // Wait a bit to ensure we've processed output
              std::thread::sleep(std::time::Duration::from_secs(2));
              return;
          }
      };

      // Wait for stdout/stderr monitors to finish
      let _ = stdout_monitor.join();
      let _ = stderr_monitor.join();

      // Clean up
      if let Err(e) = set_installation_status(&monitor_handle, false) {
          log::error!("Failed to update installation status: {}", e);
      }

      // Let the frontend know installation is complete
      let success = status.success();
      log::info!("Emitting installation_complete event with success={}", success);
      let _ = monitor_handle.emit("installation_complete", json!({
          "success": success,
          "message": if success {
              "Installation process has completed successfully".to_string()
          } else {
              format!("Installation process failed with exit code: {}", status.code().unwrap_or(-1))
          }
      }));

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
