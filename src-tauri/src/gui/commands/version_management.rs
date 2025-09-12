use idf_im_lib::idf_config::IdfInstallation;
use log::{debug, error, info};


#[tauri::command]
pub fn get_installed_versions() -> Vec<IdfInstallation>{
  match idf_im_lib::version_manager::get_esp_ide_config() {
    Ok(config) => {
      if config.idf_installed.is_empty() {
        debug!(
          "No versions found. Use eim install to install a new ESP-IDF version."
        );
        vec![]
      } else {
        config.idf_installed
      }
    }
    Err(err) => {
      debug!("No versions found. Use eim install to install a new ESP-IDF version.");
      debug!("Error: {}", err);
      vec![]
    }
  }
}

#[tauri::command]
pub fn rename_installation(id: String, new_name: String) -> bool {
  debug!("Renaming installation with id {} to {}", id, new_name);

  match idf_im_lib::version_manager::rename_idf_version(&id, new_name) {
    Ok(_) => {
        debug!("Successfully renamed installation {}", id);
        true
    }
    Err(e) => {
      error!("Failed to rename installation: {}", e);
      false
    }
  }
}
#[tauri::command]
pub fn remove_installation(id: String) -> bool {
  debug!("Removing installation with id {}", id);

  match idf_im_lib::version_manager::remove_single_idf_version(&id, false) {
    Ok(_) => {
        debug!("Successfully removed installation {}", id);
        true
    }
    Err(e) => {
      error!("Failed to remove installation: {}", e);
      false
    }
  }
}

#[tauri::command]
pub fn purge_all_installations() -> bool {
  debug!("Purging all installations");

  match idf_im_lib::version_manager::list_installed_versions() {
      Ok(versions) => {
        if versions.is_empty() {
          info!("No versions installed");
          true
        } else {
          let mut failed = false;
          for version in versions {
            info!("Removing version: {}", version.name);
            match idf_im_lib::version_manager::remove_single_idf_version(&version.name, false) {
              Ok(_) => {
                info!("Removed version: {}", version.name);
              }
              Err(err) => {
                error!("Failed to remove version {}: {}", version.name, err);
                failed = true;
              }
            }
          }
          if failed {
            error!("Some versions failed to remove. Check logs for details.");
            false
          } else {
            info!("All versions removed successfully.");
            true
          }
        }
      }
      Err(err) => {
        error!("Failed to list installed versions: {}", err);
        false
      }
  }
}
