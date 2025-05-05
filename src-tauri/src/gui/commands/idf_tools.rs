use crate::gui::ui::{send_message, send_tools_message, ProgressBar};
use anyhow::{anyhow, Context, Result};

use idf_im_lib::{
  add_path_to_path, decompress_archive, download_file, ensure_path,
  idf_tools::{self, get_tools_export_paths},
  python_utils::run_idf_tools_py, verify_file_checksum, DownloadProgress,
  settings::Settings,
};
use log::{debug, error, info};
use std::{
  path::{Path, PathBuf},
  sync::mpsc,
  thread,
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

  let tools_to_download = idf_tools::get_list_of_tools_to_download(
      tools.clone(),
      settings.target.clone().unwrap_or_default(),
      settings.mirror.as_deref(),
  );

  for (tool_name, download) in tools_to_download {
      process_tool_download(app_handle, &tool_setup, &tool_name, &download).await?;
  }

  let tools_install_folder = &PathBuf::from(&tool_setup.install_dir);
  let parent_of_tools_install_folder = tools_install_folder.parent().unwrap().to_path_buf();

  info!("Setting up tools... to directory: {}", tools_install_folder.display());

  let env_vars_python =
      idf_im_lib::setup_environment_variables(tools_install_folder, idf_path)
          .map_err(|e| {
              send_message(
                  app_handle,
                  format!("Failed to setup environment variables: {}", e),
                  "error".to_string(),
              );
              anyhow!("Failed to setup environment variables: {}", e)
          })?;

  let env_vars_install =
          idf_im_lib::setup_environment_variables(&parent_of_tools_install_folder, idf_path)
              .map_err(|e| {
                  send_message(
                      app_handle,
                      format!("Failed to setup environment variables: {}", e),
                      "error".to_string(),
                  );
                  anyhow!("Failed to setup environment variables: {}", e)
              })?;

  // Get and validate IDF tools path
  let mut idf_tools_path = idf_path.clone();
  idf_tools_path.push(settings.idf_tools_path.clone().unwrap_or_default());

  if std::fs::metadata(&idf_tools_path).is_err() {
      error!("IDF tools path does not exist");
      return Err(anyhow!("Failed to setup environment variables: IDF tools path does not exist"));
  }

  // Run IDF tools Python script
  run_idf_tools_py(
      idf_tools_path.to_str().unwrap(),
      &env_vars_install,
      &env_vars_python
  ).map_err(|e| {
      send_message(
          app_handle,
          format!("Failed to run IDF tools setup: {}", e),
          "error".to_string(),
      );
      anyhow!("Failed to run IDF tools setup: {}", e)
  })?;

  send_message(
      app_handle,
      "IDF tools setup completed successfully".to_string(),
      "info".to_string(),
  );

  let export_paths: Vec<String> = get_tools_export_paths(
      tools,
      settings.target.clone().unwrap(),
      tools_install_folder
          .to_str()
          .unwrap(),
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

/// Processes a single tool download
async fn process_tool_download(
  app_handle: &AppHandle,
  tool_setup: &ToolSetup,
  tool_name: &str,
  download: &idf_tools::Download,
) -> Result<()> {
  let (progress_tx, progress_rx) = mpsc::channel();
  let progress = ProgressBar::new(app_handle.clone(), &format!("Installing tool {}", tool_name));

  let filename = Path::new(&download.url)
      .file_name()
      .and_then(|f| f.to_str())
      .ok_or_else(|| anyhow!("Invalid download URL"))?;

  let full_path = Path::new(&tool_setup.download_dir).join(filename);
  let full_path_str = match full_path.to_str() {
      Some(s) => s,
      None => return Err(anyhow!("Invalid file path")),
  };

  send_tools_message(app_handle, filename.to_string(), "start".to_string());

  // Verify existing file checksum
  if let Ok(true) = verify_file_checksum(&download.sha256, full_path_str) {
      info!("Checksum verified for existing file: {}", full_path_str);
      send_tools_message(app_handle, filename.to_string(), "match".to_string());
      return Ok(());
  }

  // Setup progress monitoring
  let progress_handle = setup_progress_monitoring(
      app_handle.clone(),
      progress_rx,
      progress,
      tool_name.to_string(),
  );

  // Download file
  info!("Downloading {} to: {}", tool_name, full_path_str);
  match download_file(&download.url, &tool_setup.download_dir, progress_tx).await {
      Ok(_) => {
          send_tools_message(app_handle, filename.to_string(), "downloaded".to_string());
          send_message(
              app_handle,
              format!("Tool {} downloaded successfully", tool_name),
              "info".to_string(),
          );
      }
      Err(e) => return Err(anyhow!("Download failed: {}", e)),
  };

  // Verify downloaded file
  verify_download(app_handle, &download.sha256, full_path_str, filename)?;

  // Extract tool
  extract_tool(
      app_handle,
      filename,
      full_path_str,
      Path::new(&tool_setup.install_dir),
  )?;

  progress_handle
      .join()
      .map_err(|_| anyhow!("Progress monitoring thread panicked"))?;

  Ok(())
}

/// Verifies that a downloaded file matches its checksum
fn verify_download(
  app_handle: &AppHandle,
  sha256: &str,
  full_path_str: &str,
  filename: &str,
) -> Result<()> {
  match verify_file_checksum(sha256, full_path_str) {
      Ok(true) => {
          info!(
              "Checksum verified for newly downloaded file: {}",
              full_path_str
          );
          send_tools_message(
              app_handle,
              filename.to_string(),
              "download_verified".to_string(),
          );
          Ok(())
      }
      _ => {
          debug!(
              "Checksum verification of downloaded file failed: {}",
              full_path_str
          );
          send_tools_message(
              app_handle,
              filename.to_string(),
              "download_verification_failed".to_string(),
          );
          Err(anyhow!("Checksum verification failed"))
      }
  }
}

/// Extracts a downloaded tool archive
fn extract_tool(
  app_handle: &AppHandle,
  tool_name: &str,
  full_path_str: &str,
  install_dir: &Path,
) -> Result<()> {
  match decompress_archive(full_path_str, install_dir.to_str().unwrap()) {
      Ok(_) => {
          send_tools_message(app_handle, tool_name.to_string(), "extracted".to_string());
          send_message(
              app_handle,
              format!("Tool {} extracted successfully", tool_name),
              "info".to_string(),
          );
      }
      Err(e) => {
          send_tools_message(app_handle, tool_name.to_string(), "error".to_string());
          send_message(
              app_handle,
              format!("Failed to extract tool {}: {}", tool_name, e),
              "error".to_string(),
          );
          return Err(e.into());
      }
  }
  Ok(())
}

/// Sets up progress monitoring for a download
fn setup_progress_monitoring(
  app_handle: AppHandle,
  progress_rx: mpsc::Receiver<DownloadProgress>,
  progress: ProgressBar,
  tool_name: String,
) -> thread::JoinHandle<()> {
  thread::spawn(move || {
      while let Ok(progress_msg) = progress_rx.recv() {
          match progress_msg {
              DownloadProgress::Progress(current, total) => {
                  let percentage = current * 100 / total;
                  progress.update(
                      percentage,
                      Some(&format!("Downloading {}... {}%", tool_name, percentage)),
                  );
              }
              DownloadProgress::Complete => {
                  progress.finish();
                  break;
              }
              DownloadProgress::Error(err) => {
                  send_message(
                      &app_handle,
                      format!("Error downloading {}: {}", tool_name, err),
                      "error".to_string(),
                  );
                  break;
              }
          }
      }
  })
}
