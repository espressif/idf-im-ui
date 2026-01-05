use tauri::{AppHandle, Emitter, Manager};
use tempfile::TempDir;
use crate::gui::{app_state::{self, update_settings}, commands::idf_tools::setup_tools, get_installed_versions, ui::{emit_installation_event, emit_log_message, InstallationProgress, InstallationStage, MessageLevel}, utils::{is_path_empty_or_nonexistent, MirrorType, get_mirror_to_use}};
use std::{
  fs,
  io::{BufRead, BufReader},
  path::PathBuf,
  process::{Command, Stdio},
  sync::mpsc,
  thread,
};

use anyhow::{anyhow, Context, Result};
use idf_im_lib::{
  ensure_path,
  expand_tilde,
  idf_config::IdfConfig,
  offline_installer::{copy_idf_from_offline_archive, install_prerequisites_offline, use_offline_archive},
  utils::{copy_dir_contents, extract_zst_archive, is_valid_idf_directory, parse_cmake_version},
  version_manager::{get_default_config_path, prepare_settings_for_fix_idf_installation},
  git_tools::ProgressMessage};
use log::{debug, error, info, warn};
use serde_json::json;

use idf_im_lib::settings::Settings;
use crate::gui::{
  app_state::{get_locked_settings, get_settings_non_blocking, set_installation_status, set_is_simple_installation},
  commands,
  ui::{
      send_install_progress_message, send_message, send_simple_setup_message,
      ProgressBar,
  },
};

use super::{prequisites::{check_prequisites, install_prerequisites, python_install, python_sanity_check}, settings};

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallationPlan {
    pub total_versions: usize,
    pub versions: Vec<String>,
    pub current_version_index: Option<usize>,
}

pub fn emit_installation_plan(app_handle: &AppHandle, plan: InstallationPlan) {
    let _ = app_handle.emit("installation-plan", plan);
}

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
      rust_i18n::t!("gui.installation.folder_created", path = version_path.display().to_string()).to_string(),
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
      let progress = ProgressBar::new(app_handle.clone(), &format!("{} {}", rust_i18n::t!("gui.installation.progress.installing_idf"), version));

      while let Ok(message) = rx.recv() {
          match message {
              ProgressMessage::Finish => {
                  progress.update(100, None);
              }
              ProgressMessage::Update(value) => {
                  progress.update(value, Some(&format!("{} {}...", rust_i18n::t!("gui.installation.progress.downloading_idf"), version)));
              }
              ProgressMessage::SubmoduleUpdate((name, value)) => {
                  progress.update(
                      value,
                      Some(&format!("{} {}... {}%", rust_i18n::t!("gui.installation.progress.submodules"), name, value))
                  );
              }
              ProgressMessage::SubmoduleFinish(_name) => {
                  progress.update(100, None);
              }
          }
      }
  })
}

/// Downloads the ESP-IDF for a specific version without the detailed progress reporting
/// sadly the new gix library used in idf_im_lib for git operations does not provide progress
/// updates when used in an async context like Tauri GUI app.
async fn download_idf(
    app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    idf_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, _rx) = mpsc::channel();
    idf_im_lib::ensure_path(idf_path.to_str().unwrap())?;

    emit_installation_event(app_handle, InstallationProgress {
        stage: InstallationStage::Download,
        percentage: 0,
        message: rust_i18n::t!("gui.installation.download_starting", version = version).to_string(),
        detail: Some(rust_i18n::t!("gui.installation.download_preparing").to_string()),
        version: Some(version.to_string()),
    });
    
    let is_simple_installation = app_state::is_simple_installation(&app_handle);
    let mirror_to_use = get_mirror_to_use(&app_handle, MirrorType::IDF, settings, is_simple_installation).await;

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!("gui.installation.cloning_from_mirror",
            version = version,
            mirror = mirror_to_use.as_str()).to_string(),
    );

    // Clone values needed for the blocking task
    let idf_path_str = idf_path.to_str().unwrap().to_string();
    let repo_stub = settings.repo_stub.clone();
    let version_owned = version.to_string();
    let recurse_submodules = settings.recurse_submodules.unwrap_or_default();

    let result = match std::thread::spawn(move || {
      idf_im_lib::git_tools::get_esp_idf(
        &idf_path_str,
        repo_stub.as_deref(),
        &version_owned,
        Some(&mirror_to_use),
        recurse_submodules,
        tx,
      )
    }).join(){
        Ok(res) => res,
        Err(e) => Err(rust_i18n::t!("gui.installation.thread_panic", error = format!("{:?}", e)).to_string()),
    };

    match result {
        Ok(_) => {
            emit_installation_event(app_handle, InstallationProgress {
                stage: InstallationStage::Extract,
                percentage: 70,
                message: rust_i18n::t!("gui.installation.ready_for_tools", version = version).to_string(),
                detail: Some(rust_i18n::t!("gui.installation.location", path = idf_path.display().to_string()).to_string()),
                version: Some(version.to_string()),
            });

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                rust_i18n::t!("gui.installation.downloaded_successfully",
                    version = version,
                    path = idf_path.display().to_string()).to_string(),
            );
            Ok(())
        }
        Err(e) => {
            emit_installation_event(app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.installation.download_failed", version = version).to_string(),
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
        rust_i18n::t!("gui.installation.using_existing", path = paths.idf_path.display().to_string()).to_string(),
        "info".to_string(),
    );

    debug!("Using IDF version: {}", paths.actual_version);
  } else {
    download_idf(&app_handle, settings, &version, &paths.idf_path).await?;
  }


  let export_vars = setup_tools(&app_handle, settings, &paths.idf_path, &paths.actual_version, None).await?;

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
    let settings = get_locked_settings(&app_handle)?;
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
            message: rust_i18n::t!("gui.installation.path_not_available").to_string(),
            detail: Some(rust_i18n::t!("gui.installation.path_detail", path = format!("{:?}", settings_clone.path.clone().unwrap())).to_string()),
            version: None,
        });

        return Err(rust_i18n::t!("gui.installation.path_not_available").to_string());
    }

    // Save settings to temp file
    if let Err(e) = settings_clone.save() {
        log::error!("Failed to save temporary config: {}", e);

        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: rust_i18n::t!("gui.installation.config_save_failed").to_string(),
            detail: Some(e.to_string()),
            version: None,
        });

        return Err(rust_i18n::t!("gui.installation.config_save_failed").to_string());
    }

    log::info!("Saved temporary config to {}", config_path.display());

    // Get current executable path
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // Emit initial progress
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: rust_i18n::t!("gui.installation.starting_process").to_string(),
        detail: Some(rust_i18n::t!("gui.installation.launching_subprocess").to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        rust_i18n::t!("gui.installation.starting_separate_process").to_string());

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
                message: rust_i18n::t!("gui.installation.installer_process_failed").to_string(),
                detail: Some(e.to_string()),
                version: None,
            });
            format!("Failed to start installer: {}", e)
        })?;

    // Set up monitor thread to read output and send to frontend
    let monitor_handle = app_handle.clone();
    let cfg_path = config_path.clone();
    let versions = settings_clone.idf_versions.clone().unwrap_or_default();

    emit_installation_plan(&app_handle, InstallationPlan {
        total_versions: versions.len(),
        versions: versions.clone(),
        current_version_index: None,
    });

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
        let parse_and_emit_progress = move |handle: &AppHandle, line: &str,
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

                        if let Some(version_index) = version_clone.iter().position(|v| v == &version) {
                            emit_installation_plan(&handle, InstallationPlan {
                                total_versions: version_clone.len(),
                                versions: version_clone.clone(),
                                current_version_index: Some(version_index),
                            });
                        }

                        emit_installation_event(handle, InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: 10,
                            message: rust_i18n::t!("gui.installation.starting_version", version = version.clone()).to_string(),
                            detail: Some(rust_i18n::t!("gui.installation.preparing_download").to_string()),
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
                    message: rust_i18n::t!("gui.installation.checking_prerequisites").to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.verifying_requirements").to_string()),
                    version: current_ver.clone(),
                });
                *stage = InstallationStage::Prerequisites;
                *percentage = 8;

            } else if line.contains("Python sanity check") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Prerequisites,
                    percentage: 12,
                    message: rust_i18n::t!("gui.installation.verifying_python").to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.checking_python").to_string()),
                    version: current_ver.clone(),
                });
                *percentage = 12;

            } else if line.contains("Cloning ESP-IDF") || line.contains("git clone") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Download,
                    percentage: 15,
                    message: rust_i18n::t!("gui.installation.downloading_repository").to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.cloning_main").to_string()),
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
                    message: rust_i18n::t!("gui.installation.downloading_submodules").to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.processing_submodules").to_string()),
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
                            message: rust_i18n::t!("gui.installation.installing_tools", count = *total).to_string(),
                            detail: Some(rust_i18n::t!("gui.installation.preparing_tools").to_string()),
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
                        message: rust_i18n::t!("gui.installation.downloading_tool", name = tool_name).to_string(),
                        detail: Some(rust_i18n::t!("gui.installation.tool_number", number = *completed + 1).to_string()),
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
                    message: rust_i18n::t!("gui.installation.installed_tool", number = *completed).to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.tool_completed").to_string()),
                    version: current_ver.clone(),
                });
                *percentage = tool_progress.min(85);

            } else if line.contains("Python environment") || line.contains("Installing python") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Python,
                    percentage: 90,
                    message: rust_i18n::t!("gui.installation.python_environment").to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.configuring_python").to_string()),
                    version: current_ver.clone(),
                });
                *stage = InstallationStage::Python;
                *percentage = 90;

            } else if line.contains("Successfully installed IDF") || line.contains("Installation complete") {
                emit_installation_event(handle, InstallationProgress {
                    stage: InstallationStage::Complete,
                    percentage: 100,
                    message: rust_i18n::t!("gui.installation.completed_successfully").to_string(),
                    detail: Some(rust_i18n::t!("gui.installation.finished").to_string()),
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
                    message: rust_i18n::t!("gui.installation.process_failed").to_string(),
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
                message: rust_i18n::t!("gui.installation.all_completed").to_string(),
                detail: Some(rust_i18n::t!("gui.installation.all_versions", versions = versions.join(", ")).to_string()),
                version: None,
            });

            emit_log_message(&monitor_handle, MessageLevel::Success,
                rust_i18n::t!("gui.installation.success_message").to_string());
        } else {
            let error_msg = rust_i18n::t!("gui.installation.failed_exit_code", code = status.code().unwrap_or(-1)).to_string();

            emit_installation_event(&monitor_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.installation.process_failed_detail").to_string(),
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
    let app_state = app_handle.state::<crate::gui::app_state::AppState>();
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
                message: rust_i18n::t!("gui.installation.no_versions_selected").to_string(),
                detail: Some(rust_i18n::t!("gui.installation.select_version").to_string()),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Warning,
                rust_i18n::t!("gui.installation.no_versions_warning").to_string());

            set_installation_status(&app_handle, false)?;
            return Err(rust_i18n::t!("gui.installation.no_versions_warning").to_string());
        }
    };

    emit_installation_plan(&app_handle, InstallationPlan {
      total_versions: versions.len(),
      versions: versions.clone(),
      current_version_index: None,
    });

    let total_versions = versions.len();
    let plural = if total_versions == 1 { "" } else { "s" };
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: rust_i18n::t!("gui.installation.starting_batch",
            count = total_versions,
            plural = plural).to_string(),
        detail: Some(rust_i18n::t!("gui.installation.versions_list", versions = versions.join(", ")).to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        rust_i18n::t!("gui.installation.batch_log",
            count = total_versions,
            versions = versions.join(", ")).to_string());

    // Install each version with progress tracking
    for (index, version) in versions.iter().enumerate() {
        emit_installation_plan(&app_handle, InstallationPlan {
          total_versions: versions.len(),
          versions: versions.clone(),
          current_version_index: Some(index),
        });

        let version_start_percentage = (index * 90) / total_versions; // Each version gets equal share of 0-90%
        let version_end_percentage = ((index + 1) * 90) / total_versions;

        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Download,
            percentage: version_start_percentage as u32,
            message: rust_i18n::t!("gui.installation.starting_version", version = version).to_string(),
            detail: Some(rust_i18n::t!("gui.installation.version_detail",
                current = index + 1,
                total = total_versions,
                version = version).to_string()),
            version: Some(version.clone()),
        });

        emit_log_message(&app_handle, MessageLevel::Info,
            rust_i18n::t!("gui.installation.starting_version_log",
                version = version,
                current = index + 1,
                total = total_versions).to_string());

        // Install single version
        match install_single_version(app_handle.clone(), &settings, version.clone()).await {
            Ok(_) => {
                emit_installation_event(&app_handle, InstallationProgress {
                  stage: if index < versions.len() - 1 { InstallationStage::Configure } else { InstallationStage::Complete },
                  percentage: version_end_percentage as u32,
                  message: rust_i18n::t!("gui.installation.version_success", version = version).to_string(),
                  detail: Some(rust_i18n::t!("gui.installation.completed_versions",
                      current = index + 1,
                      total = total_versions).to_string()),
                  version: Some(version.clone()),
                });

                emit_log_message(&app_handle, MessageLevel::Success,
                    rust_i18n::t!("gui.installation.version_success_log",
                        version = version,
                        current = index + 1,
                        total = total_versions).to_string());
            }
            Err(e) => {
                error!("Failed to install version {}: {}", version, e);

                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.installation.version_failed", version = version).to_string(),
                    detail: Some(e.to_string()),
                    version: Some(version.clone()),
                });

                emit_log_message(&app_handle, MessageLevel::Error,
                    rust_i18n::t!("gui.installation.version_failed_log",
                        version = version,
                        error = e.to_string()).to_string());

                set_installation_status(&app_handle, false)?;
                return Err(rust_i18n::t!("gui.installation.failed_for_version",
                    version = version,
                    error = e.to_string()).to_string());
            }
        }
    }

    // Configuration phase - saving IDE JSON (90-95%)
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Configure,
        percentage: 90,
        message: rust_i18n::t!("gui.installation.configuring_environment").to_string(),
        detail: Some(rust_i18n::t!("gui.installation.saving_config").to_string()),
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
                message: rust_i18n::t!("gui.installation.config_saved").to_string(),
                detail: Some(rust_i18n::t!("gui.installation.config_saved_to", path = ide_json_path.clone()).to_string()),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Success,
                rust_i18n::t!("gui.installation.ide_json_saved", path = ide_json_path).to_string());
        }
        Err(e) => {
            // Don't fail the entire installation for IDE config save failure
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Configure,
                percentage: 93,
                message: rust_i18n::t!("gui.installation.config_save_warning").to_string(),
                detail: Some(e.to_string()),
                version: None,
            });

            emit_log_message(&app_handle, MessageLevel::Warning,
                rust_i18n::t!("gui.installation.ide_json_failed", error = e.to_string()).to_string());
        }
    }

    // Final completion (95-100%)
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Configure,
        percentage: 97,
        message: rust_i18n::t!("gui.installation.finalizing").to_string(),
        detail: Some(rust_i18n::t!("gui.installation.completing_setup").to_string()),
        version: None,
    });

    // Small delay to show finalization
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Complete!
    let plural = if total_versions == 1 { "" } else { "s" };
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Complete,
        percentage: 100,
        message: rust_i18n::t!("gui.installation.all_versions_success",
            count = total_versions,
            plural = plural).to_string(),
        detail: Some(rust_i18n::t!("gui.installation.completed_list", versions = versions.join(", ")).to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Success,
        rust_i18n::t!("gui.installation.batch_completed", count = total_versions).to_string());

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

/// Starts a simple setup process that automates the installation
#[tauri::command]
pub async fn start_simple_setup(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<crate::gui::app_state::AppState>();
    app_state::set_is_simple_installation(&app_handle, true)?;
    println!("Starting simple setup");
    let settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(e) => {
            emit_log_message(&app_handle, MessageLevel::Error, e.clone());
            return Err(e);
        }
    };

    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: rust_i18n::t!("gui.simple_setup.starting").to_string(),
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
            message: rust_i18n::t!("gui.simple_setup.installing_prerequisites").to_string(),
            detail: Some(rust_i18n::t!("gui.simple_setup.missing", items = prerequisites.join(", ")).to_string()),
            version: None,
        });

        if !install_prerequisites(app_handle.clone()) {
            prerequisites = check_prequisites(app_handle.clone());
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.simple_setup.prerequisites_failed").to_string(),
                detail: Some(rust_i18n::t!("gui.simple_setup.missing", items = prerequisites.join(", ")).to_string()),
                version: None,
            });
            return Err(rust_i18n::t!("gui.simple_setup.prerequisites_failed").to_string());
        }

        prerequisites = check_prequisites(app_handle.clone());
    }

    // Check if any prerequisites are still missing
    if !prerequisites.is_empty() {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: rust_i18n::t!("gui.simple_setup.prerequisites_missing").to_string(),
            detail: Some(rust_i18n::t!("gui.simple_setup.please_install", items = prerequisites.join(", ")).to_string()),
            version: None,
        });
        return Err(rust_i18n::t!("gui.simple_setup.prerequisites_missing").to_string());
    }

    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Prerequisites,
        percentage: 10,
        message: rust_i18n::t!("gui.simple_setup.prerequisites_verified").to_string(),
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
            message: rust_i18n::t!("gui.simple_setup.installing_python").to_string(),
            detail: None,
            version: None,
        });

        if !python_install(app_handle.clone()) {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.simple_setup.python_failed").to_string(),
                detail: Some(rust_i18n::t!("gui.simple_setup.python_install_failed").to_string()),
                version: None,
            });
            return Err(rust_i18n::t!("gui.simple_setup.python_failed").to_string());
        }

        python_found = python_sanity_check(app_handle.clone(), None);
    }

    // Check if Python is still not found
    if !python_found {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: rust_i18n::t!("gui.simple_setup.python_not_found").to_string(),
            detail: Some(rust_i18n::t!("gui.simple_setup.install_python_manually").to_string()),
            version: None,
        });
        return Err(rust_i18n::t!("gui.simple_setup.python_not_found").to_string());
    }

    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Python,
        percentage: 20,
        message: rust_i18n::t!("gui.simple_setup.python_ready").to_string(),
        detail: None,
        version: None,
    });

    // Check for IDF versions
    if settings.idf_versions.is_none() {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Configure,
            percentage: 25,
            message: rust_i18n::t!("gui.simple_setup.fetching_versions").to_string(),
            detail: None,
            version: None,
        });

        let versions = settings::get_idf_versions(app_handle.clone(),false).await;

        if versions.is_empty() {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.simple_setup.fetch_failed").to_string(),
                detail: Some(rust_i18n::t!("gui.simple_setup.retrieve_failed").to_string()),
                version: None,
            });
            return Err(rust_i18n::t!("gui.simple_setup.fetch_failed").to_string());
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
                    rust_i18n::t!("gui.simple_setup.version_selected", version = version.clone()).to_string());

                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Configure,
                    percentage: 30,
                    message: rust_i18n::t!("gui.simple_setup.version_selected_event", version = version.clone()).to_string(),
                    detail: None,
                    version: Some(version.clone()),
                });
            }
            Err(e) => {
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.simple_setup.config_failed").to_string(),
                    detail: Some(e.to_string()),
                    version: None,
                });
                return Err(e);
            }
        }
    }

    // Start installation
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Download,
        percentage: 35,
        message: rust_i18n::t!("gui.simple_setup.starting_installation").to_string(),
        detail: None,
        version: settings.idf_versions.as_ref().and_then(|v| v.first().cloned()),
    });

    let res = start_installation(app_handle.clone()).await;
    app_state::set_is_simple_installation(&app_handle, false)?;
    return res;
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
        message: rust_i18n::t!("gui.fix.checking_installation").to_string(),
        detail: Some(rust_i18n::t!("gui.fix.looking_up", id = id.clone()).to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        rust_i18n::t!("gui.fix.starting_repair", id = id.clone()).to_string());

    let versions = get_installed_versions();
    let installation = match versions.iter().find(|v| v.id == id) {
        Some(inst) => {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Checking,
                percentage: 10,
                message: rust_i18n::t!("gui.fix.found_installation", name = inst.name.clone()).to_string(),
                detail: Some(rust_i18n::t!("gui.installation.path_detail", path = inst.path.clone()).to_string()),
                version: Some(inst.name.clone()),
            });

            emit_log_message(&app_handle, MessageLevel::Info,
                rust_i18n::t!("gui.fix.found_at", name = inst.name.clone(), path = inst.path.clone()).to_string());

            inst
        }
        None => {
            let error_msg = rust_i18n::t!("gui.fix.not_found_detail", id = id.clone()).to_string();
            error!("{}", error_msg);

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.fix.not_found").to_string(),
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
        message: rust_i18n::t!("gui.fix.preparing_config").to_string(),
        detail: Some(rust_i18n::t!("gui.fix.setting_up").to_string()),
        version: Some(installation.name.clone()),
    });

    let mut settings = match prepare_settings_for_fix_idf_installation(PathBuf::from(installation.path.clone())).await {
        Ok(settings) => {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Prerequisites,
                percentage: 30,
                message: rust_i18n::t!("gui.fix.config_prepared").to_string(),
                detail: Some(rust_i18n::t!("gui.fix.ready_to_repair").to_string()),
                version: Some(installation.name.clone()),
            });

            emit_log_message(&app_handle, MessageLevel::Success,
                rust_i18n::t!("gui.fix.config_prepared_log").to_string());

            settings
        }
        Err(e) => {
            let error_msg = rust_i18n::t!("gui.fix.prepare_failed_detail", error = e.to_string()).to_string();
            error!("{}", error_msg);

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.fix.prepare_failed").to_string(),
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
        message: rust_i18n::t!("gui.fix.starting_repair_version", version = installation.name.clone()).to_string(),
        detail: Some(rust_i18n::t!("gui.fix.beginning_reinstall").to_string()),
        version: Some(installation.name.clone()),
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        rust_i18n::t!("gui.fix.starting_repair_log", version = installation.name.clone()).to_string());

    // The actual repair process - this will generate detailed progress events
    match install_single_version(app_handle.clone(), &settings, installation.name.clone()).await {
        Ok(_) => {
            emit_log_message(&app_handle, MessageLevel::Success,
                rust_i18n::t!("gui.fix.repair_success", version = installation.name.clone()).to_string());

            info!("Successfully fixed installation {}", id);
        }
        Err(e) => {
            let error_msg = rust_i18n::t!("gui.fix.repair_failed_detail", error = e.to_string()).to_string();
            error!("{}", error_msg);

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.fix.repair_failed", version = installation.name.clone()).to_string(),
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
        message: rust_i18n::t!("gui.fix.updating_config").to_string(),
        detail: Some(rust_i18n::t!("gui.fix.saving_info").to_string()),
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
                    message: rust_i18n::t!("gui.fix.config_updated").to_string(),
                    detail: Some(rust_i18n::t!("gui.fix.found_in_config").to_string()),
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
                            message: rust_i18n::t!("gui.fix.config_saved_success").to_string(),
                            detail: Some(rust_i18n::t!("gui.installation.config_saved_to", path = ide_json_path.clone()).to_string()),
                            version: Some(installation.name.clone()),
                        });

                        emit_log_message(&app_handle, MessageLevel::Success,
                            rust_i18n::t!("gui.fix.ide_json_updated", path = ide_json_path.clone()).to_string());

                        info!("IDE JSON saved to {}", ide_json_path);
                    }
                    Err(e) => {
                        let error_msg = rust_i18n::t!("gui.installation.ide_config_save_failed_detail", error = e.to_string()).to_string();
                        warn!("{}", error_msg);

                        emit_installation_event(&app_handle, InstallationProgress {
                            stage: InstallationStage::Configure,
                            percentage: 95,
                            message: rust_i18n::t!("gui.fix.config_save_warning").to_string(),
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
                        message: rust_i18n::t!("gui.fix.config_saved_success").to_string(),
                        detail: Some(rust_i18n::t!("gui.installation.config_saved_to", path = ide_json_path.clone()).to_string()),
                        version: Some(installation.name.clone()),
                    });

                    emit_log_message(&app_handle, MessageLevel::Success,
                        rust_i18n::t!("gui.fix.ide_json_updated", path = ide_json_path.clone()).to_string());

                    info!("IDE JSON saved to {}", ide_json_path);
                }
                Err(e) => {
                    let error_msg = rust_i18n::t!("gui.installation.ide_config_save_failed_detail", error = e.to_string()).to_string();
                    warn!("{}", error_msg);

                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Configure,
                        percentage: 95,
                        message: rust_i18n::t!("gui.fix.config_save_warning").to_string(),
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
        message: rust_i18n::t!("gui.fix.repair_completed", version = installation.name.clone()).to_string(),
        detail: Some(rust_i18n::t!("gui.fix.repaired_at", path = installation.path.clone()).to_string()),
        version: Some(installation.name.clone()),
    });

    emit_log_message(&app_handle, MessageLevel::Success,
        rust_i18n::t!("gui.fix.completed_log", version = installation.name.clone()).to_string());

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

#[tauri::command]
pub async fn start_offline_installation(app_handle: AppHandle, archives: Vec<String>, install_path: String) -> Result<(), String> {
    // Set installation flag
    if let Err(e) = set_installation_status(&app_handle, true) {
        return Err(e);
    }

    // Validate archives
    if archives.is_empty() {
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: rust_i18n::t!("gui.offline.no_archives").to_string(),
            detail: Some(rust_i18n::t!("gui.offline.select_archive").to_string()),
            version: None,
        });
        set_installation_status(&app_handle, false)?;
        return Err(rust_i18n::t!("gui.offline.no_archives").to_string());
    }

    // Initial progress event
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Checking,
        percentage: 0,
        message: rust_i18n::t!("gui.offline.starting").to_string(),
        detail: Some(rust_i18n::t!("gui.offline.processing_archives", count = archives.len()).to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Info,
        rust_i18n::t!("gui.offline.starting_log", count = archives.len()).to_string());

    let total_archives = archives.len();

    for (archive_index, archive) in archives.iter().enumerate() {
        let archive_path = std::path::PathBuf::from(archive);

        // Check archive exists
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: (archive_index * 10 / total_archives) as u32,
            message: rust_i18n::t!("gui.offline.validating_archive", name = get_file_name(archive)).to_string(),
            detail: Some(rust_i18n::t!("gui.offline.archive_number", current = archive_index + 1, total = total_archives).to_string()),
            version: None,
        });

        if !archive_path.try_exists().unwrap_or(false) {
            let error_msg = rust_i18n::t!("gui.offline.archive_not_exist", path = archive).to_string();
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.offline.archive_not_found").to_string(),
                detail: Some(error_msg.clone()),
                version: None,
            });
            set_installation_status(&app_handle, false)?;
            return Err(error_msg);
        }

        emit_log_message(&app_handle, MessageLevel::Info,
            rust_i18n::t!("gui.offline.validated", path = archive).to_string());

        // Create temporary directory for extraction
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Extract,
            percentage: ((archive_index * 90 + 10) / total_archives) as u32,
            message: rust_i18n::t!("gui.offline.creating_workspace").to_string(),
            detail: Some(rust_i18n::t!("gui.offline.preparing_extraction").to_string()),
            version: None,
        });

        let offline_archive_dir = TempDir::new().map_err(|e| {
            let error_msg = rust_i18n::t!("gui.offline.temp_dir_failed", error = e.to_string()).to_string();
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.offline.workspace_failed").to_string(),
                detail: Some(error_msg.clone()),
                version: None,
            });
            error_msg
        })?;

        emit_log_message(&app_handle, MessageLevel::Info,
            rust_i18n::t!("gui.offline.temp_dir_created", path = offline_archive_dir.path().display().to_string()).to_string());

        // Get and configure settings
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Extract,
            percentage: ((archive_index * 90 + 15) / total_archives) as u32,
            message: rust_i18n::t!("gui.offline.configuring_settings").to_string(),
            detail: Some(rust_i18n::t!("gui.offline.preparing_config").to_string()),
            version: None,
        });

        let mut settings = get_settings_non_blocking(&app_handle)?;
        if !install_path.is_empty() && is_path_empty_or_nonexistent(&install_path, &[]) {
            settings.path = Some(PathBuf::from(install_path.clone()));
            emit_log_message(&app_handle, MessageLevel::Info,
                rust_i18n::t!("gui.offline.custom_path", path = install_path.clone()).to_string());
        }
        settings.use_local_archive = Some(archive_path);

        // Extract and configure archive
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Extract,
            percentage: ((archive_index * 90 + 20) / total_archives) as u32,
            message: rust_i18n::t!("gui.offline.extracting_archive", name = get_file_name(archive)).to_string(),
            detail: Some(rust_i18n::t!("gui.offline.processing_contents").to_string()),
            version: None,
        });

        settings = match use_offline_archive(settings, &offline_archive_dir) {
            Ok(updated_config) => {
                emit_log_message(&app_handle, MessageLevel::Success,
                    rust_i18n::t!("gui.offline.extraction_success").to_string());
                updated_config
            }
            Err(err) => {
                let error_msg = rust_i18n::t!("gui.offline.extraction_failed_detail", error = err.to_string()).to_string();
                error!("{}", error_msg);
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.offline.extraction_failed").to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                });
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        };

        // Install prerequisites on Windows
        if std::env::consts::OS == "windows" {
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Prerequisites,
                percentage: ((archive_index * 90 + 25) / total_archives) as u32,
                message: rust_i18n::t!("gui.offline.installing_prerequisites").to_string(),
                detail: Some(rust_i18n::t!("gui.offline.installing_windows_components").to_string()),
                version: None,
            });

            match install_prerequisites_offline(&offline_archive_dir) {
                Ok(_) => {
                    emit_log_message(&app_handle, MessageLevel::Success,
                        rust_i18n::t!("gui.offline.prerequisites_success").to_string());
                    settings.skip_prerequisites_check = Some(true);
                }
                Err(err) => {
                    let error_msg = rust_i18n::t!("gui.offline.prerequisites_failed_detail", error = err.to_string()).to_string();
                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: rust_i18n::t!("gui.offline.prerequisites_failed").to_string(),
                        detail: Some(error_msg.clone()),
                        version: None,
                    });
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            }
        } else {
            // Check prerequisites on non-Windows systems
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Prerequisites,
                percentage: ((archive_index * 90 + 25) / total_archives) as u32,
                message: rust_i18n::t!("gui.offline.checking_prerequisites").to_string(),
                detail: Some(rust_i18n::t!("gui.offline.verifying_components").to_string()),
                version: None,
            });

            let prereq = check_prequisites(app_handle.clone());
            if !prereq.is_empty() {
                let error_msg = rust_i18n::t!("gui.offline.missing_prerequisites", items = prereq.join(", ")).to_string();
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.offline.prerequisites_missing").to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                });
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
                        emit_log_message(&app_handle, MessageLevel::Warning,
                            rust_i18n::t!("gui.offline.python_check_warning", error = err.to_string()).to_string());
                        warn!("{:?}", err);
                    }
                }
            }
            if !python_sane {
                let error_msg = rust_i18n::t!("gui.offline.python_check_failed").to_string();
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: error_msg.clone(),
                    detail: Some(rust_i18n::t!("gui.offline.python_not_configured").to_string()),
                    version: None,
                });
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }

            emit_log_message(&app_handle, MessageLevel::Success,
                rust_i18n::t!("gui.offline.prerequisites_verified").to_string());
        }

        // Copy ESP-IDF from archive
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Download,
            percentage: ((archive_index * 90 + 35) / total_archives) as u32,
            message: rust_i18n::t!("gui.offline.installing_idf").to_string(),
            detail: Some(rust_i18n::t!("gui.offline.copying_files").to_string()),
            version: None,
        });

        match copy_idf_from_offline_archive(&offline_archive_dir, &settings) {
            Ok(_) => {
                emit_log_message(&app_handle, MessageLevel::Success,
                    rust_i18n::t!("gui.offline.idf_copy_success").to_string());
            }
            Err(err) => {
                let error_msg = rust_i18n::t!("gui.offline.idf_copy_failed", error = err.to_string()).to_string();
                error!("{}", error_msg);
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.offline.idf_install_failed").to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                });
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

            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Tools,
                percentage: version_progress_start + (version_index as u32 * version_progress_range / versions.len() as u32),
                message: rust_i18n::t!("gui.offline.processing_version", version = idf_version).to_string(),
                detail: Some(rust_i18n::t!("gui.offline.setting_up_version", current = version_index + 1, total = versions.len()).to_string()),
                version: Some(idf_version.clone()),
            });

            let paths = match settings.get_version_paths(idf_version) {
                Ok(paths) => {
                    emit_log_message(&app_handle, MessageLevel::Info,
                        rust_i18n::t!("gui.offline.version_paths_configured", version = idf_version, path = paths.idf_path.display().to_string()).to_string());
                    paths
                }
                Err(err) => {
                    let error_msg = rust_i18n::t!("gui.offline.path_config_failed_detail", error = err.to_string()).to_string();
                    error!("{}", error_msg);
                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: rust_i18n::t!("gui.offline.path_config_failed").to_string(),
                        detail: Some(error_msg.clone()),
                        version: Some(idf_version.clone()),
                    });
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            };

            settings.idf_path = Some(paths.idf_path.clone());
            idf_im_lib::add_path_to_path(paths.idf_path.to_str().unwrap());

            // Copy tools
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Tools,
                percentage: version_progress_start + ((version_index + 1) as u32 * version_progress_range / (versions.len() as u32 * 3)),
                message: rust_i18n::t!("gui.offline.installing_tools").to_string(),
                detail: Some(rust_i18n::t!("gui.offline.copying_tools").to_string()),
                version: Some(idf_version.clone()),
            });

            match copy_dir_contents(
                &offline_archive_dir.path().join("dist"),
                &paths.tool_download_directory,
            ) {
                Ok(_) => {
                    emit_log_message(&app_handle, MessageLevel::Success,
                        rust_i18n::t!("gui.offline.tools_copy_success").to_string());
                }
                Err(err) => {
                    let error_msg = rust_i18n::t!("gui.offline.tools_copy_failed", error = err.to_string()).to_string();
                    error!("{}", error_msg);
                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: rust_i18n::t!("gui.offline.tools_install_failed").to_string(),
                        detail: Some(error_msg.clone()),
                        version: Some(idf_version.clone()),
                    });
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            }

            idf_im_lib::add_path_to_path(paths.tool_install_directory.to_str().unwrap());

            // Setup tools
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Tools,
                percentage: version_progress_start + ((version_index + 1) as u32 * version_progress_range * 2 / (versions.len() as u32 * 3)),
                message: rust_i18n::t!("gui.offline.configuring_tools").to_string(),
                detail: Some(rust_i18n::t!("gui.offline.setting_up_environment").to_string()),
                version: Some(idf_version.clone()),
            });

            let export_vars = match setup_tools(&app_handle, &settings, &paths.idf_path, &paths.actual_version, Some(offline_archive_dir.path())).await {
                Ok(vars) => {
                    emit_log_message(&app_handle, MessageLevel::Success,
                        rust_i18n::t!("gui.offline.tools_configured").to_string());
                    vars
                }
                Err(err) => {
                    let error_msg = rust_i18n::t!("gui.offline.tools_setup_failed", error = err.to_string()).to_string();
                    error!("{}", error_msg);
                    emit_installation_event(&app_handle, InstallationProgress {
                        stage: InstallationStage::Error,
                        percentage: 0,
                        message: rust_i18n::t!("gui.offline.tools_config_failed").to_string(),
                        detail: Some(error_msg.clone()),
                        version: Some(idf_version.clone()),
                    });
                    set_installation_status(&app_handle, false)?;
                    return Err(error_msg);
                }
            };

            // Post-install configuration
            emit_installation_event(&app_handle, InstallationProgress {
                stage: InstallationStage::Configure,
                percentage: version_progress_end - 5,
                message: rust_i18n::t!("gui.offline.finalizing").to_string(),
                detail: Some(rust_i18n::t!("gui.offline.completing_setup").to_string()),
                version: Some(idf_version.clone()),
            });

            idf_im_lib::single_version_post_install(
                &paths.activation_script_path.to_str().unwrap(),
                paths.idf_path.to_str().unwrap(),
                &paths.actual_version,
                paths.tool_install_directory.to_str().unwrap(),
                export_vars,
                paths.python_venv_path.to_str(),
                None,
            );

            emit_log_message(&app_handle, MessageLevel::Success,
                rust_i18n::t!("gui.offline.version_configured", version = idf_version).to_string());
        }

        // Save IDE configuration
        emit_installation_event(&app_handle, InstallationProgress {
            stage: InstallationStage::Configure,
            percentage: ((archive_index * 90 + 88) / total_archives) as u32,
            message: rust_i18n::t!("gui.offline.saving_ide_config").to_string(),
            detail: Some(rust_i18n::t!("gui.offline.updating_settings").to_string()),
            version: None,
        });

        let ide_conf_path_tmp = PathBuf::from(&settings.esp_idf_json_path.clone().unwrap_or_default());
        debug!("IDE configuration path: {}", ide_conf_path_tmp.display());

        match ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
            Ok(_) => {
                emit_log_message(&app_handle, MessageLevel::Info,
                    rust_i18n::t!("gui.offline.ide_dir_created").to_string());
            }
            Err(err) => {
                let error_msg = rust_i18n::t!("gui.offline.ide_dir_failed", error = err.to_string()).to_string();
                error!("{}", error_msg);
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.offline.ide_config_failed").to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                });
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        }

        match settings.save_esp_ide_json() {
            Ok(_) => {
                emit_log_message(&app_handle, MessageLevel::Success,
                    rust_i18n::t!("gui.offline.ide_config_saved").to_string());
                debug!("IDE configuration saved.");
            }
            Err(err) => {
                let error_msg = rust_i18n::t!("gui.offline.ide_config_save_failed_detail", error = err.to_string()).to_string();
                error!("{}", error_msg);
                emit_installation_event(&app_handle, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.offline.ide_config_save_failed").to_string(),
                    detail: Some(error_msg.clone()),
                    version: None,
                });
                set_installation_status(&app_handle, false)?;
                return Err(error_msg);
            }
        }

        emit_log_message(&app_handle, MessageLevel::Success,
            rust_i18n::t!("gui.offline.archive_processed",
                name = get_file_name(archive),
                current = archive_index + 1,
                total = total_archives).to_string());
    }

    // Final completion
    emit_installation_event(&app_handle, InstallationProgress {
        stage: InstallationStage::Complete,
        percentage: 100,
        message: rust_i18n::t!("gui.offline.completed").to_string(),
        detail: Some(rust_i18n::t!("gui.offline.processed_archives", count = total_archives).to_string()),
        version: None,
    });

    emit_log_message(&app_handle, MessageLevel::Success,
        rust_i18n::t!("gui.offline.all_completed").to_string());

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

// Helper function to extract filename from path
fn get_file_name(path: &str) -> &str {
    path.split(&['/', '\\'][..]).last().unwrap_or(path)
}
