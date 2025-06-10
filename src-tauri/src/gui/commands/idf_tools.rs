use crate::gui::ui::{send_message, send_tools_message, ProgressBar};
use anyhow::{anyhow, Context, Result};

use idf_im_lib::{
  add_path_to_path,ensure_path,
  idf_tools::{self, get_tools_export_paths},
  DownloadProgress,
  settings::Settings,
};
use log::{ error, info};
use std::{
  path::{Path, PathBuf},
};
use tauri::AppHandle;


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
              format!("Failed to create download directory: {}", e),
              "error".to_string(),
          );
          e.to_string()
      })?;

      // Create installation directory
      ensure_path(&self.install_dir).map_err(|e| {
          send_message(
              app_handle,
              format!("Failed to create installation directory: {}", e),
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
          return Err(format!(
              "tools.json file not found at: {}",
              self.tools_json_path
          ));
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
          send_message(
              app_handle,
              format!("Failed to parse tools.json: {}", e),
              "error".to_string(),
          );
          anyhow!("Failed to parse tools.json: {}", e)
      })?;

  // Setup progress callback for the library function
  let app_handle_clone = app_handle.clone();
  let progress_callback = move |progress: DownloadProgress| {
    match progress {
        DownloadProgress::Progress(current, total) => {
            let percentage = current * 100 / total;
            let progress = ProgressBar::new(app_handle_clone.clone(), "Installing tools");
            progress.update(percentage, Some(&format!("Downloading... {}%", percentage)));
        }
        DownloadProgress::Start(url) => {
            // Extract filename from URL for tool status updates
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                send_tools_message(&app_handle_clone, filename.to_string(), "start".to_string());
            }
        }
        DownloadProgress::Complete => {
            let progress = ProgressBar::new(app_handle_clone.clone(), "Installing tools");
            progress.finish();
        }
        DownloadProgress::Downloaded(url) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                send_tools_message(&app_handle_clone, filename.to_string(), "downloaded".to_string());
            }
        }
        DownloadProgress::Verified(url) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                send_tools_message(&app_handle_clone, filename.to_string(), "download_verified".to_string());
                send_tools_message(&app_handle_clone, filename.to_string(), "match".to_string());
            }
        }
        DownloadProgress::Extracted(url) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                send_tools_message(&app_handle_clone, filename.to_string(), "extracted".to_string());
            }
        }
        DownloadProgress::Error(err) => {
            send_message(
                &app_handle_clone,
                format!("Download error: {}", err),
                "error".to_string(),
            );
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
  .map_err(|e| anyhow!("Failed to setup tools: {}", e))?;

  let tools_install_folder = &PathBuf::from(&tool_setup.install_dir);

  info!("Setting up tools... to directory: {}", tools_install_folder.display());

  // Get and validate IDF tools path
  let mut idf_tools_path = idf_path.clone();
  idf_tools_path.push(settings.idf_tools_path.clone().unwrap_or_default());

  if std::fs::metadata(&idf_tools_path).is_err() {
      error!("IDF tools path does not exist");
      return Err(anyhow!("Failed to setup environment variables: IDF tools path does not exist"));
  }

  match idf_im_lib::python_utils::install_python_env(
    &idf_version,
    &tools_install_folder,
    true, //TODO: actually read from config
    &idf_path,
    &settings.idf_features.clone().unwrap_or_default(),
  ).await {
      Ok(_) => {
        info!("Python environment installed");
        send_message(
          app_handle,
          "Python environment installed successfully".to_string(),
          "info".to_string(),
        );
      }
      Err(err) => {
          error!("Failed to install Python environment: {}", err);
          send_message(
            app_handle,
            "Failed to install Python environment".to_string(),
            "error".to_string(),
          );
          return Err(anyhow!("Failed to install Python environment: {}", err));
      }
  };

  send_message(
      app_handle,
      "IDF tools setup completed successfully".to_string(),
      "info".to_string(),
  );

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

  send_message(
      app_handle,
      "Tools setup completed successfully".to_string(),
      "info".to_string(),
  );

  Ok(export_paths)
}
