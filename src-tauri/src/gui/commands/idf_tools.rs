use crate::gui::ui::{emit_installation_event, emit_log_message, send_message, send_tools_message, InstallationProgress, InstallationStage, MessageLevel, ProgressBar};
use anyhow::{anyhow, Context, Result};

use idf_im_lib::{
  add_path_to_path,ensure_path,
  idf_tools::{self, get_tools_export_paths},
  DownloadProgress,
  settings::Settings,
};
use log::{ error, info};
use std::{
  path::{Path, PathBuf}, sync::{Arc, Mutex},
};
use tauri::AppHandle;
use rust_i18n::t;


/// Represents the tool setup configuration
#[derive(Debug)]
struct ToolSetup {
  download_dir: String,
  install_dir: String,
  tools_json_path: String,
}

impl ToolSetup {
  /// Creates a new tool setup based on settings and version path
  fn new(settings: &Settings, version_path: &PathBuf) -> Result<Self, String> {
      let p = version_path;
      let tools_json_path = p
          .join("esp-idf")
          .join(settings.tools_json_file.clone().unwrap_or_default());
      let download_dir = p.join(
          settings
              .tool_download_folder_name
              .clone()
              .unwrap_or_default(),
      );
      let install_dir = p.join(
          settings
              .tool_install_folder_name
              .clone()
              .unwrap_or_default(),
      );
      Ok(Self {
          download_dir: download_dir.to_str().unwrap().to_string(),
          install_dir: install_dir.to_str().unwrap().to_string(),
          tools_json_path: tools_json_path.to_str().unwrap().to_string(),
      })
  }

  /// Creates necessary directories for tool installation
  fn create_directories(&self, app_handle: &AppHandle) -> Result<(), String> {
      // Create download directory
      ensure_path(&self.download_dir).map_err(|e| {
          send_message(
              app_handle,
              t!("gui.setup_tools.dir_create_failed", error = e.to_string()).to_string(),
              "error".to_string(),
          );
          e.to_string()
      })?;

      // Create installation directory
      ensure_path(&self.install_dir).map_err(|e| {
          send_message(
              app_handle,
              t!("gui.setup_tools.install_dir_create_failed", error = e.to_string()).to_string(),
              "error".to_string(),
          );
          e.to_string()
      })?;

      // Add installation directory to PATH
      add_path_to_path(&self.install_dir);

      Ok(())
  }

  /// Validates that the tools.json file exists
  fn validate_tools_json(&self) -> Result<(), String> {
      if std::fs::metadata(&self.tools_json_path).is_err() {
          return Err(t!("gui.setup_tools.tools_json_not_found", path = self.tools_json_path.clone()).to_string());
      }
      Ok(())
  }
}

/// Sets up ESP-IDF tools based on settings and IDF path
pub async fn setup_tools(
    app_handle: &AppHandle,
    settings: &Settings,
    idf_path: &PathBuf,
    idf_version: &str,
    offline_archive_dir: Option<&Path>,
) -> Result<Vec<String>> {
    info!("Setting up tools...");

    let version_path = idf_path
        .parent()
        .context("Failed to get parent directory of IDF path")?;

    // Initialize tool setup
    let tool_setup = ToolSetup::new(settings, &PathBuf::from(version_path))
        .map_err(|e| anyhow!("Failed to initialize tool setup: {}", e))?;

    // Create necessary directories
    tool_setup
        .create_directories(app_handle)
        .map_err(|e| anyhow!("Failed to create tool directories: {}", e))?;

    // Validate tools.json exists
    tool_setup
        .validate_tools_json()
        .map_err(|e| anyhow!("Failed to validate tools.json: {}", e))?;

    // Parse tools.json and get list of tools to download
    let tools = idf_tools::read_and_parse_tools_file(&tool_setup.tools_json_path)
        .map_err(|e| {
            emit_log_message(
                app_handle,
                MessageLevel::Error,
                t!("gui.setup_tools.tools_json_parse_failed", error = e.to_string()).to_string(),
            );
            anyhow!(t!("gui.setup_tools.tools_json_parse_failed", error = e.to_string()).to_string())
        })?;

    // Start tools installation phase (65% of total progress)
    emit_installation_event(app_handle, InstallationProgress {
        stage: InstallationStage::Tools,
        percentage: 65,
        message: t!("gui.setup_tools.installation_starting").to_string(),
        detail: Some(t!("gui.setup_tools.preparing_tools", count = tools.tools.len()).to_string()),
        version: Some(idf_version.to_string()),
    });

    // Progress tracking with interior mutability
    let total_tools = tools.tools.len() as f32;
    let completed_tools = Arc::new(Mutex::new(0u32));
    let current_tool_name = Arc::new(Mutex::new(String::new()));
    let base_percentage = 65u32; // Tools start at 65%
    let tools_range = 25u32; // Tools take 65-90% (25% range)

    // Setup progress callback for the library function
    let app_handle_clone = app_handle.clone();
    let idf_version_clone = idf_version.to_string();
    let completed_tools_clone = completed_tools.clone();
    let current_tool_name_clone = current_tool_name.clone();

    let progress_callback = move |progress: DownloadProgress| {
        match progress {
            DownloadProgress::Progress(current, total) => {
                if total > 0 {
                    let tool_progress = current * 100 / total;
                    let completed = *completed_tools_clone.lock().unwrap();
                    let tool_name = current_tool_name_clone.lock().unwrap().clone();

                    let overall_tool_progress = (completed as f32 / total_tools) * tools_range as f32;
                    let current_tool_contribution = (tool_progress as f32 / 100.0) * (tools_range as f32 / total_tools);
                    let overall_percentage = base_percentage + overall_tool_progress as u32 + current_tool_contribution as u32;

                    emit_installation_event(&app_handle_clone, InstallationProgress {
                        stage: InstallationStage::Tools,
                        percentage: overall_percentage.min(89), // Cap at 89% to leave room for completion
                        message: t!("gui.setup_tools.downloading",
                            tool_name = tool_name.split('/').last()
                                .unwrap_or(&tool_name)
                                .replace("-", " ")
                        ).to_string(),
                        detail: Some(t!("gui.setup_tools.tool_progress",
                            current = completed + 1,
                            total = total_tools as u32,
                            percentage = tool_progress
                        ).to_string()),
                        version: Some(idf_version_clone.clone()),
                    });
                }
            }

            DownloadProgress::Start(url) => {
                // Extract tool name from URL
                let tool_name = if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                    // Try to extract tool name from filename (remove version/platform info)
                    let clean_name = filename
                        .split('-')
                        .take(3) // Take first few parts before version numbers
                        .collect::<Vec<_>>()
                        .join("-")
                        .replace(".tar", "")
                        .replace(".zip", "");
                    clean_name
                } else {
                    t!("gui.setup_tools.unknown_tool").to_string()
                };

                *current_tool_name_clone.lock().unwrap() = tool_name.clone();
                let completed = *completed_tools_clone.lock().unwrap();
                let overall_percentage = base_percentage + ((completed as f32 / total_tools) * tools_range as f32) as u32;

                emit_installation_event(&app_handle_clone, InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: overall_percentage.min(89),
                    message: t!("gui.setup_tools.preparing",
                        tool_name = tool_name.replace("-", " ")
                    ).to_string(),
                    detail: Some(t!("gui.setup_tools.starting_tool",
                        current = completed + 1,
                        total = total_tools as u32
                    ).to_string()),
                    version: Some(idf_version_clone.clone()),
                });

                emit_log_message(&app_handle_clone, MessageLevel::Info,
                    t!("gui.setup_tools.starting_download", tool_name = tool_name).to_string());
            }

            DownloadProgress::Downloaded(url) => {
                let completed = *completed_tools_clone.lock().unwrap();
                let tool_name = current_tool_name_clone.lock().unwrap().clone();

                emit_installation_event(&app_handle_clone, InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: (base_percentage + ((completed as f32 / total_tools) * tools_range as f32) as u32 + 1).min(89),
                    message: t!("gui.setup_tools.verifying",
                        tool_name = tool_name.replace("-", " ")
                    ).to_string(),
                    detail: Some(t!("gui.setup_tools.downloaded_tool",
                        current = completed + 1,
                        total = total_tools as u32
                    ).to_string()),
                    version: Some(idf_version_clone.clone()),
                });

                emit_log_message(&app_handle_clone, MessageLevel::Info,
                    t!("gui.setup_tools.downloaded", tool_name = tool_name).to_string());
            }

            DownloadProgress::Verified(url) => {
                let completed = *completed_tools_clone.lock().unwrap();
                let tool_name = current_tool_name_clone.lock().unwrap().clone();

                emit_installation_event(&app_handle_clone, InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: (base_percentage + ((completed as f32 / total_tools) * tools_range as f32) as u32 + 2).min(89),
                    message: t!("gui.setup_tools.extracting",
                        tool_name = tool_name.replace("-", " ")
                    ).to_string(),
                    detail: Some(t!("gui.setup_tools.verified_tool",
                        current = completed + 1,
                        total = total_tools as u32
                    ).to_string()),
                    version: Some(idf_version_clone.clone()),
                });

                emit_log_message(&app_handle_clone, MessageLevel::Success,
                    t!("gui.setup_tools.verified", tool_name = tool_name).to_string());
            }

            DownloadProgress::Extracted(url, _dest) => {
                let mut completed = completed_tools_clone.lock().unwrap();
                *completed += 1;
                let completed_count = *completed;
                let tool_name = current_tool_name_clone.lock().unwrap().clone();

                let overall_percentage = base_percentage + ((completed_count as f32 / total_tools) * tools_range as f32) as u32;

                emit_installation_event(&app_handle_clone, InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: overall_percentage.min(89),
                    message: t!("gui.setup_tools.installed",
                        tool_name = tool_name.replace("-", " ")
                    ).to_string(),
                    detail: Some(t!("gui.setup_tools.completed_tools",
                        current = completed_count,
                        total = total_tools as u32
                    ).to_string()),
                    version: Some(idf_version_clone.clone()),
                });

                emit_log_message(&app_handle_clone, MessageLevel::Success,
                    t!("gui.setup_tools.installed_tool",
                        tool_name = tool_name,
                        current = completed_count,
                        total = total_tools as u32
                    ).to_string());
            }

            DownloadProgress::Complete => {
                emit_installation_event(&app_handle_clone, InstallationProgress {
                    stage: InstallationStage::Tools,
                    percentage: 89,
                    message: t!("gui.setup_tools.all_downloaded").to_string(),
                    detail: Some(t!("gui.setup_tools.completed_installation", count = total_tools as u32).to_string()),
                    version: Some(idf_version_clone.clone()),
                });
            }

            DownloadProgress::Error(err) => {
                let tool_name = current_tool_name_clone.lock().unwrap().clone();

                emit_installation_event(&app_handle_clone, InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: t!("gui.setup_tools.tool_failed", tool_name = tool_name).to_string(),
                    detail: Some(err.to_string()),
                    version: Some(idf_version_clone.clone()),
                });

                emit_log_message(&app_handle_clone, MessageLevel::Error,
                    t!("gui.setup_tools.tool_error", error = err.to_string()).to_string());
            }
        }
    };

    // Use the library's setup_tools function
    let installed_tools_list = idf_tools::setup_tools(
        &tools,
        settings.target.clone().unwrap_or_default(),
        &PathBuf::from(&tool_setup.download_dir),
        &PathBuf::from(&tool_setup.install_dir),
        settings.mirror.as_deref(),
        progress_callback,
    )
    .await
    .map_err(|e| {
        emit_installation_event(app_handle, InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: t!("gui.setup_tools.setup_failed").to_string(),
            detail: Some(e.to_string()),
            version: Some(idf_version.to_string()),
        });
        anyhow!("Failed to setup tools: {}", e)
    })?;

    let tools_install_folder = &PathBuf::from(&tool_setup.install_dir);

    info!("Setting up tools... to directory: {}", tools_install_folder.display());

    // Transition to Python setup phase (90%)
    emit_installation_event(app_handle, InstallationProgress {
        stage: InstallationStage::Python,
        percentage: 90,
        message: t!("gui.setup_tools.python_setup_starting").to_string(),
        detail: Some(t!("gui.setup_tools.python_installing").to_string()),
        version: Some(idf_version.to_string()),
    });

    let paths = match settings.get_version_paths(&idf_version) {
      Ok(paths) => paths,
      Err(err) => {
        return Err(anyhow!("Failed to setup environment paths for idf versions"));
      }
    };

    // Install Python environment
    match idf_im_lib::python_utils::install_python_env(
        &paths,
        &paths.actual_version,
        &paths.tool_install_directory,
        true, //TODO: actually read from config
        &settings.idf_features.clone().unwrap_or_default(),
        offline_archive_dir, // Offline archive directory
        &settings.pypi_mirror, // PyPI mirror
    ).await {
        Ok(_) => {
            info!("Python environment installed");
            emit_installation_event(app_handle, InstallationProgress {
                stage: InstallationStage::Python,
                percentage: 93,
                message: t!("gui.setup_tools.python_configured").to_string(),
                detail: Some(t!("gui.setup_tools.python_deps_installed").to_string()),
                version: Some(idf_version.to_string()),
            });

            emit_log_message(app_handle, MessageLevel::Success,
                t!("gui.setup_tools.python_installed").to_string());
        }
        Err(err) => {
            error!("Failed to install Python environment: {}", err);
            emit_installation_event(app_handle, InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: t!("gui.setup_tools.python_setup_failed").to_string(),
                detail: Some(err.to_string()),
                version: Some(idf_version.to_string()),
            });
            return Err(anyhow!("Failed to install Python environment: {}", err));
        }
    };

    // Generate export paths
    let export_paths = idf_im_lib::idf_tools::get_tools_export_paths_from_list(
        tools,
        installed_tools_list,
        tools_install_folder.to_str().unwrap(),
    )
    .into_iter()
    .map(|p| {
        if std::env::consts::OS == "windows" {
            idf_im_lib::replace_unescaped_spaces_win(&p)
        } else {
            p
        }
    })
    .collect();

    // Configuration phase (95%)
    emit_installation_event(app_handle, InstallationProgress {
        stage: InstallationStage::Configure,
        percentage: 95,
        message: t!("gui.setup_tools.configuring_env").to_string(),
        detail: Some(t!("gui.setup_tools.configuring_dev_env").to_string()),
        version: Some(idf_version.to_string()),
    });

    emit_log_message(app_handle, MessageLevel::Success,
        t!("gui.setup_tools.setup_completed").to_string());

    Ok(export_paths)
}
