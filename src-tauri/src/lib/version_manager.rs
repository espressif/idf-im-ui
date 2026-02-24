use anyhow::anyhow;
use anyhow::Result;
use gix::command;
use log::debug;
use log::error;
use log::info;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::Output;

use log::warn;
use lnk::ShellLink;
use lnk::encoding::WINDOWS_1252;


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

/// Searches the user's desktop for a shortcut whose arguments contain the specified custom profile filename.
///
/// # Arguments
///
/// * `custom_profile_filename` - A string slice that holds the name of the custom profile script to search for.
///
/// # Returns
///
/// * `Ok(Some(String))` - The name of the shortcut (without the .lnk extension) if found.
/// * `Ok(None)` - If no matching shortcut is found.
/// * `Err(String)` - If an error occurs during the search process.
pub fn find_shortcut_by_profile(custom_profile_filename: &str) -> anyhow::Result<Option<String>> {
    let desktop_path = dirs::desktop_dir().ok_or(anyhow!("Failed to get desktop directory"))?;

    // Read all entries in the desktop directory
    let entries = fs::read_dir(&desktop_path)
        .map_err(|e| anyhow!("Failed to read desktop directory: {}", e))?;

    // Iterate over each entry in the desktop directory
    for entry in entries {
        let entry = entry.map_err(|e| anyhow!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        // Check if the entry is a .lnk file
        if path.extension().map_or(false, |ext| ext == "lnk") {
            // Open and parse the .lnk file
            let link = ShellLink::open(&path, WINDOWS_1252)
                .map_err(|e| anyhow!("Failed to open or parse shortcut file '{}': {}", path.display(), e))?;

            // The arguments are stored in the `string_data` field.
            // We access it via the `string_data()` method on the ShellLink struct.
            if let Some(arguments) = link.string_data().command_line_arguments() {
                // Check if the arguments contain the custom profile filename
                if arguments.contains(custom_profile_filename) {
                    // Extract just the filename without the .lnk extension
                    if let Some(name) = path.file_name() {
                        return Ok(Some(name.to_string_lossy().into_owned()));
                    }
                }
            }
        }
    }

    // If no matching shortcut was found
    Ok(None)
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
pub fn remove_single_idf_version(identifier: &str, keep_idf_folder: bool) -> Result<String> {
    //TODO: remove also from path
    let config_path = get_default_config_path();
    let mut ide_config = IdfConfig::from_file(&config_path)?;
    if let Some(installation) = ide_config
        .idf_installed
        .iter()
        .find(|install| install.id == identifier || install.name == identifier).cloned()
    {
        let installation_folder_path = PathBuf::from(installation.path.clone());
        let installation_folder = installation_folder_path.parent().ok_or_else(|| {
            anyhow!(
                "Installation path '{}' has no parent directory",
                installation_folder_path.display()
            )
        })?;
        if !keep_idf_folder {
            match remove_directory_all(installation_folder) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!("Failed to remove installation folder: {}", e));
                }
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
        if std::env::consts::OS == "windows" {
            // On Windows, also remove the desktop icon associated with the installation
            match find_shortcut_by_profile(&installation.activation_script) {
                Ok(Some(shortcut_name)) => {
                    let desktop_path = match dirs::desktop_dir().ok_or("Failed to get desktop directory") {
                        Ok(path) => path,
                        Err(e) => {
                            error!("{}", e);
                            return Ok(format!("Version {} removed, but failed to remove desktop shortcut", identifier));
                        }
                    };
                    let shortcut_path = desktop_path.join(shortcut_name);
                    if shortcut_path.exists() {
                        match fs::remove_file(&shortcut_path) {
                            Ok(_) => {
                                info!("Removed desktop shortcut: {}", shortcut_path.display());
                            }
                            Err(e) => {
                                error!("Failed to remove desktop shortcut {}: {}", shortcut_path.display(), e);
                            }
                        }
                    }
                }
                Ok(None) => {
                    info!("No desktop shortcut found for profile: {}", installation.activation_script);
                }
                Err(e) => {
                    error!("Error searching for desktop shortcut: {}", e);
                }
            }
        }
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
    let mut dirs = crate::utils::find_directories_by_name(path, "esp-idf");
    dirs.sort();
    dirs.reverse();
    let filtered_dirs = crate::utils::filter_duplicate_paths(dirs.clone());
    filtered_dirs
        .iter()
        .filter(|p| crate::utils::is_valid_idf_directory(p))
        .cloned()
        .collect()
}

pub fn run_command_in_context(identifier: &str, command: &str) -> anyhow::Result<ExitStatus> {
    let installation = match list_installed_versions() {
        Ok(versions) => versions.into_iter().find(|v| {
            v.id == identifier
                || v.name == identifier
                || { let normalized_identifier = crate::utils::normalize_path_for_comparison(identifier);
                     normalized_identifier.is_some() && normalized_identifier == crate::utils::normalize_path_for_comparison(&v.path)
                }
        }),
        Err(e) => {
            return Err(anyhow!("Failed to list installed versions: {}", e));
        }
    };

    let installation = match installation {
        Some(install) => install,
        None => {
            return Err(anyhow!("Version {} not installed", identifier));
        }
    };

    let activation_script = &installation.activation_script;

    #[cfg(not(target_os = "windows"))]
    let script = format!(
        "source \"{}\"\nshopt -s expand_aliases\n{}",
        activation_script,
        command
    );

    #[cfg(target_os = "windows")]
    let script = format!(
        ". \"{}\"\n{}",
        activation_script,
        command
    );

    println!("Running command in context of IDF version {}", identifier);

    let executor = crate::command_executor::get_executor();
    match executor.run_script_from_string_streaming(&script) {
        Ok(status) => Ok(status),
        Err(e) => Err(anyhow!("Failed to execute command: {}", e)),
    }
}


pub async fn prepare_settings_for_fix_idf_installation(path_to_fix: PathBuf) -> anyhow::Result<Settings> {
    info!("Fixing IDF installation at path: {}", path_to_fix.display());
    // The fix logic is just instalation with use of existing repository
    let mut version_name = None;
    match list_installed_versions() {
        Ok(versions) => {
            for v in versions {
                if v.path == path_to_fix.to_str().unwrap() {
                    info!("Found existing IDF version: {}", v.name);
                    // Remove the existing activation script and eim_idf.json entry
                    match remove_single_idf_version(&v.name, true) {
                        Ok(_) => {
                            info!("Removed existing IDF version from eim_idf.json: {}", v.name);
                            version_name = Some(v.name.clone());
                        }
                        Err(err) => {
                            error!("Failed to remove existing IDF version {}: {}", v.name, err);
                        }
                    }
                }
            }
        }
        Err(_) => {
            info!("Failed to list installed versions. Using default naming.");
        }
    }

    let mut settings = Settings::default();
    settings.path = Some(path_to_fix.clone());
    settings.non_interactive = Some(true);
    settings.version_name = version_name;
    settings.install_all_prerequisites = Some(true);
    settings.config_file_save_path = None;
    return Ok(settings);

}
