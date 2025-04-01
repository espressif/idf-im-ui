use anyhow::anyhow;
use anyhow::Result;
use log::debug;
use std::path::Path;
use std::path::PathBuf;

use log::warn;

use crate::idf_config;
use crate::utils::remove_directory_all;
use crate::{
    idf_config::{IdfConfig, IdfInstallation},
    settings::Settings,
};

/// Returns the default path to the ESP-IDF configuration file.
///
/// The default path is constructed by joining the `esp_idf_json_path` setting from the `Settings` struct
/// with the filename "eim_idf.json". If `esp_idf_json_path` is not set, the default path will be
/// constructed using the default settings.
///
/// # Returns
///
/// A `PathBuf` representing the default path to the ESP-IDF configuration file.
pub fn get_default_config_path() -> PathBuf {
    let default_settings = Settings::default();
    PathBuf::from(default_settings.esp_idf_json_path.unwrap_or_default()).join("eim_idf.json")
}

// todo: add optional path parameter enabling the user to specify a custom config file
// or to search for it in a different location ( or whole filesystem)
pub fn list_installed_versions() -> Result<Vec<IdfInstallation>> {
    let config_path = get_default_config_path();
    get_installed_versions_from_config_file(&config_path)
}

/// Retrieves a list of installed ESP-IDF versions from the specified configuration file.
///
/// # Parameters
///
/// * `config_path` - A reference to a `PathBuf` representing the path to the ESP-IDF configuration file.
///
/// # Returns
///
/// * `Result<Vec<IdfInstallation>, anyhow::Error>` - On success, returns a `Result` containing a vector of
///   `IdfInstallation` structs representing the installed ESP-IDF versions. On error, returns an `anyhow::Error`
///   with a description of the error.
pub fn get_installed_versions_from_config_file(
    config_path: &PathBuf,
) -> Result<Vec<IdfInstallation>> {
    if config_path.is_file() {
        let ide_config = IdfConfig::from_file(config_path)?;
        return Ok(ide_config.idf_installed);
    }
    Err(anyhow!("Config file not found"))
}

/// Retrieves the selected ESP-IDF installation from the configuration file.
///
/// This function reads the ESP-IDF configuration from the default location specified by the
/// `get_default_config_path` function and returns the selected installation. If no installation is
/// selected, it logs a warning and returns `None`.
///
/// # Parameters
///
/// None.
///
/// # Returns
///
/// * `Option<IdfInstallation>` - Returns `Some(IdfInstallation)` if a selected installation is found in the
///   configuration file. Returns `None` if no installation is selected or if an error occurs while reading
///   the configuration file.
pub fn get_selected_version() -> Option<IdfInstallation> {
    let config_path = get_default_config_path();
    let ide_config = IdfConfig::from_file(config_path).ok();
    if let Some(config) = ide_config {
        match config.get_selected_installation() {
            Some(selected) => return Some(selected.clone()),
            None => {
                warn!("No selected version found in config file");
                return None;
            }
        }
    }
    None
}
/// Retrieves the ESP-IDF configuration from the default location.
///
/// This function reads the ESP-IDF configuration from the default location specified by the
/// `get_default_config_path` function. The configuration is then returned as an `IdfConfig` struct.
///
/// # Parameters
///
/// None.
///
/// # Returns
///
/// * `Result<IdfConfig, anyhow::Error>` - On success, returns a `Result` containing the `IdfConfig` struct
///   representing the ESP-IDF configuration. On error, returns an `anyhow::Error` with a description of the error.
pub fn get_esp_ide_config() -> Result<IdfConfig> {
    let config_path = get_default_config_path();
    IdfConfig::from_file(&config_path)
}

/// Selects the specified ESP-IDF version by updating the configuration file.
///
/// This function reads the ESP-IDF configuration from the default location, selects the installation
/// with the given identifier, and updates the configuration file. If the installation is successfully
/// selected, the function returns a `Result` containing a success message. If the installation is not
/// found in the configuration file, the function returns an error.
///
/// # Parameters
///
/// * `identifier` - A reference to a string representing the identifier of the ESP-IDF version to select.
///   The identifier can be either the version number or the name of the installation.
///
/// # Returns
///
/// * `Result<String, anyhow::Error>` - On success, returns a `Result` containing a string message indicating
///   that the version has been selected. On error, returns an `anyhow::Error` with a description of the error.
pub fn select_idf_version(identifier: &str) -> Result<String> {
    let config_path = get_default_config_path();
    let mut ide_config = IdfConfig::from_file(&config_path)?;
    if ide_config.select_installation(identifier) {
        ide_config.to_file(config_path, true, false)?;
        return Ok(format!("Version {} selected", identifier));
    }
    Err(anyhow!("Version {} not installed", identifier))
}

/// Renames the specified ESP-IDF version in the configuration file.
///
/// This function reads the ESP-IDF configuration from the default location, updates the name of the
/// installation with the given identifier, and saves the updated configuration file. If the installation
/// is successfully renamed, the function returns a `Result` containing a success message. If the
/// installation is not found in the configuration file, the function returns an error.
///
/// # Parameters
///
/// * `identifier` - A reference to a string representing the identifier of the ESP-IDF version to rename.
///   The identifier can be either the version number or the name of the installation.
///
/// * `new_name` - A string representing the new name for the ESP-IDF version.
///
/// # Returns
///
/// * `Result<String, anyhow::Error>` - On success, returns a `Result` containing a string message indicating
///   that the version has been renamed. On error, returns an `anyhow::Error` with a description of the error.
pub fn rename_idf_version(identifier: &str, new_name: String) -> Result<String> {
    let config_path = get_default_config_path();
    let mut ide_config = IdfConfig::from_file(&config_path)?;
    let res = ide_config.update_installation_name(identifier, new_name.to_string());
    if res {
        ide_config.to_file(config_path, true, false)?;
        Ok(format!("Version {} renamed to {}", identifier, new_name))
    } else {
        Err(anyhow!("Version {} not installed", identifier))
    }
}

/// Removes a single ESP-IDF version from the configuration file and its associated directories.
///
/// This function reads the ESP-IDF configuration from the default location, removes the installation
/// with the given identifier, and purges the installation directory and activation script. If the
/// installation is successfully removed, the function returns a `Result` containing a success message.
/// If the installation is not found in the configuration file, the function returns an error.
///
/// # Parameters
///
/// * `identifier` - A reference to a string representing the identifier of the ESP-IDF version to remove.
///   The identifier can be either the version number or the name of the installation.
///
/// # Returns
///
/// * `Result<String, anyhow::Error>` - On success, returns a `Result` containing a string message indicating
///   that the version has been removed. On error, returns an `anyhow::Error` with a description of the error.
pub fn remove_single_idf_version(identifier: &str) -> Result<String> {
    //TODO: remove also from path
    let config_path = get_default_config_path();
    let mut ide_config = IdfConfig::from_file(&config_path)?;
    if let Some(installation) = ide_config
        .idf_installed
        .iter()
        .find(|install| install.id == identifier || install.name == identifier)
    {
        let installation_folder_path = PathBuf::from(installation.path.clone());
        let installation_folder = installation_folder_path.parent().unwrap();
        match remove_directory_all(&installation_folder) {
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow!("Failed to remove installation folder: {}", e));
            }
        }
        match remove_directory_all(installation.clone().activation_script) {
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow!("Failed to remove activation script: {}", e));
            }
        }
        if ide_config.remove_installation(identifier) {
            debug!("Removed installation from config file");
        } else {
            return Err(anyhow!("Failed to remove installation from config file"));
        }
        ide_config.to_file(config_path, true, false)?;
        Ok(format!("Version {} removed", identifier))
    } else {
        Err(anyhow!("Version {} not installed", identifier))
    }
}

/// Finds ESP-IDF folders within the specified directory and its subdirectories.
///
/// This function searches for directories named "esp-idf" within the given path and its subdirectories.
/// It returns a vector of absolute paths to the found directories, sorted in descending order.
///
/// # Parameters
///
/// * `path` - A reference to a string representing the root directory to search for ESP-IDF folders.
///
/// # Returns
///
/// * `Vec<String>` - A vector of strings representing the absolute paths to the found ESP-IDF folders.
///   The vector is sorted in descending order.
pub fn find_esp_idf_folders(path: &str) -> Vec<String> {
    let path = Path::new(path);
    let mut dirs = crate::utils::find_directories_by_name(&path, "esp-idf");
    dirs.sort();
    dirs.reverse();
    let filtered_dirs = crate::utils::filter_duplicate_paths(dirs.clone());
    filtered_dirs
        .iter()
        .filter(|p| crate::utils::is_valid_idf_directory(p))
        .cloned()
        .collect()
}
