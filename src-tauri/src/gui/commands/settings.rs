use tauri::{AppHandle, Manager};
use idf_im_lib::settings;
use crate::gui::{
  app_state::{self, get_locked_settings, get_settings_non_blocking, update_settings, AppState},
  ui::send_message,
  utils::is_path_empty_or_nonexistent,
};

use log::info;
use serde_json::{json, Value};
use std::{
  fs::File,
  io::Read,
  path::{Path, PathBuf},
};

/// Gets the current settings
#[tauri::command]
pub fn get_settings(app_handle: tauri::AppHandle) -> settings::Settings {
  get_settings_non_blocking(&app_handle).unwrap_or_default()
}


/// Loads settings from a file
#[tauri::command]
pub fn load_settings(app_handle: AppHandle, path: &str) {
  update_settings(&app_handle, |settings| {
      log::debug!("settings before load {:?}", settings);
      settings.load(path)
          .map_err(|_| {
              send_message(
                  &app_handle,
                  format!("Failed to load settings from file: {}", path),
                  "warning".to_string(),
              )
          })
          .expect("Failed to load settings");
        log::debug!("settings after load {:?}", settings);
  });
  send_message(
      &app_handle,
      format!("Settings loaded from {}", path),
      "info".to_string(),
  );
}

/// Saves the current config to a file
#[tauri::command]
pub fn save_config(app_handle: tauri::AppHandle, path: String) {
  let mut settings = match get_locked_settings(&app_handle) {
      Ok(s) => s,
      Err(_) => {
          return send_message(
              &app_handle,
              "Installation config can not be saved. Please try again later.".to_string(),
              "error".to_string(),
          )
      }
  };

  settings.config_file_save_path = Some(PathBuf::from(path));
  let _ = settings.save();
}

/// Gets the installation path
#[tauri::command]
pub fn get_installation_path(app_handle: AppHandle) -> String {
  let settings = match get_settings_non_blocking(&app_handle) {
      Ok(s) => s,
      Err(e) => {
          send_message(
              &app_handle,
              e,
              "error".to_string()
          );
          return String::new();
      }
  };

  let path = settings.path.clone().unwrap_or_default();
  path.to_str().unwrap_or_default().to_string()
}

/// Sets the installation path
#[tauri::command]
pub fn set_installation_path(app_handle: AppHandle, path: String) -> Result<(), String> {
  info!("Setting installation path: {}", path);
  update_settings(&app_handle, |settings| {
      settings.path = Some(PathBuf::from(path));
  })?;

  send_message(
      &app_handle,
      "Installation path updated successfully".to_string(),
      "info".to_string(),
  );
  Ok(())
}

/// Gets the list of available IDF targets
#[tauri::command]
pub async fn get_available_targets(app_handle: AppHandle) -> Vec<Value> {
  let settings = match get_settings_non_blocking(&app_handle) {
      Ok(s) => s,
      Err(e) => {
          send_message(&app_handle, e, "error".to_string());
          return Vec::new();
      }
  };

  let targets = settings.target.clone().unwrap_or_default();
  let available_targets = match idf_im_lib::idf_versions::get_avalible_targets().await {
      Ok(targets) => targets,
      Err(_) => Vec::new(),
  };

  available_targets
      .into_iter()
      .map(|t| {
          json!({
            "name": t,
            "selected": targets.contains(&t),
          })
      })
      .collect()
}

/// Sets the selected targets
#[tauri::command]
pub fn set_targets(app_handle: AppHandle, targets: Vec<String>) -> Result<(), String> {
  info!("Setting targets: {:?}", targets);
  update_settings(&app_handle, |settings| {
      settings.target = Some(targets);
  })?;
  send_message(
      &app_handle,
      "Targets updated successfully".to_string(),
      "info".to_string(),
  );
  Ok(())
}

/// Gets the list of available IDF versions
#[tauri::command]
pub async fn get_idf_versions(app_handle: AppHandle) -> Vec<Value> {
  let settings = match get_settings_non_blocking(&app_handle) {
      Ok(s) => s,
      Err(e) => {
          send_message(&app_handle, e, "error".to_string());
          return Vec::new();
      }
  };

  let targets = settings.target.clone().unwrap_or_default();
  let versions = settings.idf_versions.clone().unwrap_or_default();

  let targets_vec: Vec<String> = targets.to_vec();
  let mut available_versions = if targets_vec.contains(&"all".to_string()) {
      idf_im_lib::idf_versions::get_idf_names().await
  } else if !targets.is_empty() {
      // todo: handle multiple targets
      idf_im_lib::idf_versions::get_idf_name_by_target(&targets[0].to_string().to_lowercase())
          .await
  } else {
      Vec::new()
  };
  available_versions.push("master".to_string());

  available_versions
      .into_iter()
      .map(|v| {
          json!({
            "name": v,
            "selected": versions.contains(&v),
          })
      })
      .collect()
}

/// Sets the selected IDF versions
#[tauri::command]
pub fn set_versions(app_handle: AppHandle, versions: Vec<String>) -> Result<(), String> {
  info!("Setting IDF versions: {:?}", versions);
  update_settings(&app_handle, |settings| {
      settings.idf_versions = Some(versions);
  })?;

  send_message(
      &app_handle,
      "IDF versions updated successfully".to_string(),
      "info".to_string(),
  );
  Ok(())
}

/// Gets the list of available IDF mirrors
#[tauri::command]
pub fn get_idf_mirror_list(app_handle: AppHandle) -> Value {
  let settings = match get_settings_non_blocking(&app_handle) {
      Ok(s) => s,
      Err(e) => {
          send_message(&app_handle, e, "error".to_string());
          return json!({
              "mirrors": Vec::<String>::new(),
              "selected": "",
          });
      }
  };

  let mirror = settings.idf_mirror.clone().unwrap_or_default();
  let mut available_mirrors = idf_im_lib::get_idf_mirrors_list().to_vec();

  if !available_mirrors.contains(&mirror.as_str()) {
      let mut new_mirrors = vec![mirror.as_str()];
      new_mirrors.extend(available_mirrors);
      available_mirrors = new_mirrors;
  }

  json!({
    "mirrors": available_mirrors,
    "selected": mirror,
  })
}

/// Sets the selected IDF mirror
#[tauri::command]
pub fn set_idf_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
  info!("Setting IDF mirror: {}", mirror);
  update_settings(&app_handle, |settings| {
      settings.idf_mirror = Some(mirror);
  })?;

  send_message(
      &app_handle,
      "IDF mirror updated successfully".to_string(),
      "info".to_string(),
  );
  Ok(())
}

/// Gets the list of available tools mirrors
#[tauri::command]
pub fn get_tools_mirror_list(app_handle: AppHandle) -> Value {
  let settings = match get_settings_non_blocking(&app_handle) {
      Ok(s) => s,
      Err(e) => {
          send_message(&app_handle, e, "error".to_string());
          return json!({
              "mirrors": Vec::<String>::new(),
              "selected": "",
          });
      }
  };

  let mirror = settings.mirror.clone().unwrap_or_default();
  let mut available_mirrors = idf_im_lib::get_idf_tools_mirrors_list().to_vec();

  if !available_mirrors.contains(&mirror.as_str()) {
      let mut new_mirrors = vec![mirror.as_str()];
      new_mirrors.extend(available_mirrors);
      available_mirrors = new_mirrors;
  }

  json!({
    "mirrors": available_mirrors,
    "selected": mirror,
  })
}

/// Sets the selected tools mirror
#[tauri::command]
pub fn set_tools_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
  info!("Setting tools mirror: {}", mirror);
  update_settings(&app_handle, |settings| {
      settings.mirror = Some(mirror);
  })?;

  send_message(
      &app_handle,
      "Tools mirror updated successfully".to_string(),
      "info".to_string(),
  );
  Ok(())
}

/// Checks if a path is empty or doesn't exist
#[tauri::command]
pub async fn is_path_empty_or_nonexistent_command(app_handle: AppHandle, path: String) -> bool {
    let settings = match get_settings_non_blocking(&app_handle) {
        Ok(s) => s,
        Err(_) => return false,
    };
    println!("Settings: {:?}", settings);
    let versions = match &settings.idf_versions {
        Some(v) => v.clone(),
        None => {
            send_message(
                &app_handle,
                "No IDF versions selected. Please select at least one version to continue."
                    .to_string(),
                "error".to_string(),
            );
            // return false;
            [].to_vec()
        }
    };

    is_path_empty_or_nonexistent(&path, &versions)
}
