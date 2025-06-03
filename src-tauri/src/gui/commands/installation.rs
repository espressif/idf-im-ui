use tauri::{AppHandle, Emitter, Manager};
use crate::gui::{app_state::{self, update_settings}, commands::idf_tools::setup_tools, utils::is_path_empty_or_nonexistent};
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
use log::{error, info, warn};
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
  let mut version_path = expand_tilde(settings.path.as_ref().unwrap().as_path());
  version_path.push(version);

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

/// Downloads the ESP-IDF for a specific version
async fn download_idf(
  app_handle: &AppHandle,
  settings: &Settings,
  version: &str,
  idf_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
  let (tx, rx) = mpsc::channel();
  let progress = ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", version));

  let handle = spawn_progress_monitor(app_handle.clone(), version.to_string(), rx);

  match idf_im_lib::get_esp_idf(
      idf_path.to_str().unwrap(),
      settings.repo_stub.as_deref(),
      version,
      settings.idf_mirror.as_deref(),
      settings.recurse_submodules.unwrap_or_default(),
      tx,
  ) {
      Ok(_) => {
          send_message(
              app_handle,
              format!(
                  "IDF {} installed successfully at: {}",
                  version,
                  idf_path.display()
              ),
              "info".to_string(),
          );
          progress.finish();
      }
      Err(e) => {
          send_message(
              app_handle,
              format!("Failed to install IDF {}. Reason: {}", version, e),
              "error".to_string(),
          );
          progress.finish();
          return Err(e.into());
      }
  }

  handle.join().unwrap();
  Ok(())
}

/// Installs a single ESP-IDF version
async fn install_single_version(
  app_handle: AppHandle,
  settings: &Settings,
  version: String,
) -> Result<(), Box<dyn std::error::Error>> {
  info!("Installing IDF version: {}", version);

  let mut using_existing_idf = false;

  let p = idf_im_lib::expand_tilde(settings.path.as_ref().unwrap().as_path());
  let mut idf_path;
  let mut version_path = PathBuf::from(p.clone());
  if is_valid_idf_directory(p.to_str().unwrap()) {
    info!("Using existing IDF directory: {}", p.display());
    send_message(
        &app_handle,
        format!("Using existing IDF directory: {}", p.display()),
        "info".to_string(),
    );
    using_existing_idf = true;
    idf_path = p;
  } else {
    version_path = prepare_installation_directories(&app_handle.clone(), settings, &version)?;
    idf_path = version_path.clone().join("esp-idf");
    download_idf(&app_handle, settings, &version, &idf_path).await?;
  }


  let export_vars = setup_tools(&app_handle, settings, &idf_path, &version).await?;

  let tools_install_path = PathBuf::from(settings
          .tool_install_folder_name
          .clone()
          .unwrap_or_default());

  let idf_python_env_path = tools_install_path.clone().join("python").join(&version).join("venv");
  let activation_script_path = settings.esp_idf_json_path.clone().unwrap_or_default();
  let constrains_idf_version = match parse_cmake_version(idf_path.to_str().unwrap()) {
    Ok((maj,min)) => format!("v{}.{}", maj, min),
    Err(e) => {
      warn!("Failed to parse CMake version: {}", e);
      version.to_string()
    }
  };
  idf_im_lib::single_version_post_install(
      &activation_script_path,
      idf_path.to_str().unwrap(),
      &constrains_idf_version,
      tools_install_path.to_str().unwrap(),
      export_vars,
      Some(idf_python_env_path.to_str().unwrap()),
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

  if let Some(versions) = &settings.idf_versions {
      for version in versions {
          send_install_progress_message(&app_handle, version.clone(), "started".to_string());
          if let Err(e) =
              install_single_version(app_handle.clone(), &settings, version.clone()).await
          {
              send_install_progress_message(&app_handle, version.clone(), "failed".to_string());

              error!("Failed to install version {}: {}", version, e);
              set_installation_status(&app_handle, false)?;
              return Err(format!("Installation failed: {}", e));
          } else {
              send_install_progress_message(&app_handle, version.clone(), "finished".to_string());
          }
      }
  } else {
      send_message(
          &app_handle,
          "No IDF versions were selected".to_string(),
          "warning".to_string(),
      );
      set_installation_status(&app_handle, false)?;
      return Err("No IDF versions were selected".to_string());
  }

  // Save IDE JSON configuration
  let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
  let _ = ensure_path(&ide_json_path);
  match settings.save_esp_ide_json() {
      Ok(_) => {
          send_message(
              &app_handle,
              format!("IDE JSON file saved to: {}", ide_json_path),
              "info".to_string(),
          );
      }
      Err(e) => {
          send_message(
              &app_handle,
              format!("Failed to save IDE JSON file: {}", e),
              "error".to_string(),
          );
      }
  }

  send_simple_setup_message(&app_handle, 11, "Installation finished successfully".to_string());
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
          send_message(&app_handle, e, "error".to_string());
          return;
      }
  };

  send_simple_setup_message(&app_handle, 1, "started".to_string());

  // Check prerequisites
  let mut prerequisites = check_prequisites(app_handle.clone());
  let os = std::env::consts::OS.to_lowercase();

  // Install prerequisites on Windows if needed
  if !prerequisites.is_empty() && os == "windows" {
      send_simple_setup_message(&app_handle, 2, "installing prerequisites".to_string());

      if !install_prerequisites(app_handle.clone()) {
          prerequisites = check_prequisites(app_handle.clone());
          send_simple_setup_message(&app_handle, 3, prerequisites.join(", "));
          return;
      }

      prerequisites = check_prequisites(app_handle.clone());
  }

  // Check if any prerequisites are still missing
  if !prerequisites.is_empty() {
      send_simple_setup_message(&app_handle, 4, prerequisites.join(", "));
      return;
  }

  // Check for Python
  let mut python_found = python_sanity_check(app_handle.clone(), None);

  // Install Python on Windows if needed
  if !python_found && os == "windows" {
      send_simple_setup_message(&app_handle, 5, "Installing Python".to_string());

      if !python_install(app_handle.clone()) {
          send_simple_setup_message(&app_handle, 6, "Failed to install Python".to_string());
          return;
      }

      python_found = python_sanity_check(app_handle.clone(), None);
  }

  // Check if Python is still not found
  if !python_found {
      send_simple_setup_message(
          &app_handle,
          7,
          "Python not found. Please install it manually".to_string(),
      );
      return;
  }

  // Check for IDF versions
  if settings.idf_versions.is_none() {
      send_simple_setup_message(&app_handle, 8, "Getting IDF versions".to_string());

      let versions = settings::get_idf_versions(app_handle.clone()).await;

      if versions.is_empty() {
          send_simple_setup_message(&app_handle, 9, "Failed to get IDF versions".to_string());
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
              send_simple_setup_message(
                  &app_handle,
                  10,
                  format!("IDF version {} selected", version),
              );
          }
          Err(e) => {
              send_simple_setup_message(
                  &app_handle,
                  11,
                  format!("Failed to set IDF version: {}", e),
              );
              return;
          }
      }
      // if settings::set_versions(app_handle.clone(), vec![version]).is_err() {
      //     send_simple_setup_message(&app_handle, 9, "Failed to set IDF versions".to_string());
      //     return;
      // }
  }

  // Start installation
  send_simple_setup_message(&app_handle, 10, "Installing IDF".to_string());
  let _res = start_installation(app_handle.clone()).await;
}
