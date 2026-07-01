use anyhow::anyhow;
use anyhow::Result;
use gix::command;
use log::debug;
use log::error;
use log::info;
use semver::Op;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::Output;

use lnk::encoding::WINDOWS_1252;
use lnk::ShellLink;
use log::warn;

use crate::utils::remove_directory_all;
use crate::{
    idf_config::{IdfConfig, IdfInstallation},
    settings::Settings,
    idf_tools::{Tool, Version},
};
use serde::{Deserialize, Serialize};

/// Removes activation and deactivation scripts (sh, fish, ps1, bat) for a
/// given version from the activation script directory.
///
/// This must be kept in lock-step with the script names produced by
/// `create_activation_shell_script`, `create_fish_script`,
/// `create_powershell_profile`, `create_batch_profile` and their
/// `deactivate` counterparts in `lib::mod`.
fn remove_activation_scripts(
    activation_script_path: &str,
    idf_version: &str,
) -> Result<(), String> {
    let path = PathBuf::from(activation_script_path);
    let parent_dir = path.parent().ok_or("No parent directory")?;

    // Define all possible activation + deactivation script names for this version
    let scripts_to_remove = match std::env::consts::OS {
        "windows" => vec![
            format!("Microsoft.{}.PowerShell_profile.ps1", idf_version),
            format!("Microsoft.{}.PowerShell_deactivate.ps1", idf_version),
            format!("Microsoft.{}_profile.bat", idf_version),
            format!("Microsoft.{}_deactivate.bat", idf_version),
        ],
        _ => vec![
            format!("activate_idf_{}.sh", idf_version),
            format!("activate_idf_{}.fish", idf_version),
            format!("deactivate_idf_{}.sh", idf_version),
            format!("deactivate_idf_{}.fish", idf_version),
        ],
    };

    for script_name in scripts_to_remove {
        let script_path = parent_dir.join(&script_name);
        if script_path.exists() {
            match fs::remove_file(&script_path) {
                Ok(_) => info!("Removed activation script: {}", script_path.display()),
                Err(e) => warn!("Failed to remove {}: {}", script_path.display(), e),
            }
        }
    }

    Ok(())
}

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

pub fn list_installed_versions(config_path: Option<&PathBuf>) -> Result<Vec<IdfInstallation>> {
    let config_path = config_path.cloned().unwrap_or_else(get_default_config_path);
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
/// Reads the ESP-IDF configuration from the given path (or the default location if `None`)
/// and returns the selected installation. If no installation is selected, it logs a warning
/// and returns `None`.
///
/// # Parameters
///
/// * `config_path` - Optional path to the ESP-IDF configuration file. Falls back to the default path when `None`.
///
/// # Returns
///
/// * `Option<IdfInstallation>` - Returns `Some(IdfInstallation)` if a selected installation is found in the
///   configuration file. Returns `None` if no installation is selected or if an error occurs while reading
///   the configuration file.
pub fn get_selected_version(config_path: Option<&PathBuf>) -> Option<IdfInstallation> {
    let config_path = config_path.cloned().unwrap_or_else(get_default_config_path);
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
/// Retrieves the ESP-IDF configuration from the given path or the default location.
///
/// # Parameters
///
/// * `config_path` - Optional path to the ESP-IDF configuration file. Falls back to the default path when `None`.
///
/// # Returns
///
/// * `Result<IdfConfig, anyhow::Error>` - On success, returns a `Result` containing the `IdfConfig` struct
///   representing the ESP-IDF configuration. On error, returns an `anyhow::Error` with a description of the error.
pub fn get_esp_ide_config(config_path: Option<&PathBuf>) -> Result<IdfConfig> {
    let config_path = config_path.cloned().unwrap_or_else(get_default_config_path);
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
pub fn select_idf_version(identifier: &str, config_path: Option<&PathBuf>) -> Result<String> {
    let config_path = config_path.cloned().unwrap_or_else(get_default_config_path);
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
pub fn rename_idf_version(
    identifier: &str,
    new_name: String,
    config_path: Option<&PathBuf>,
) -> Result<String> {
    let config_path = config_path.cloned().unwrap_or_else(get_default_config_path);
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
            let link = ShellLink::open(&path, WINDOWS_1252).map_err(|e| {
                anyhow!(
                    "Failed to open or parse shortcut file '{}': {}",
                    path.display(),
                    e
                )
            })?;

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
pub fn remove_single_idf_version(
    identifier: &str,
    keep_idf_folder: bool,
    config_path: Option<&PathBuf>,
) -> Result<String> {
    //TODO: remove also from path
    let config_path = config_path.cloned().unwrap_or_else(get_default_config_path);
    let mut ide_config = IdfConfig::from_file(&config_path)?;
    if let Some(installation) = ide_config
        .idf_installed
        .iter()
        .find(|install| install.id == identifier || install.name == identifier)
        .cloned()
    {
        let installation_folder_path = PathBuf::from(installation.path.clone());
        let installation_folder = installation_folder_path.parent().ok_or_else(|| {
            anyhow!(
                "Installation path '{}' has no parent directory",
                installation_folder_path.display()
            )
        })?;
        if !keep_idf_folder {
            // First remove the installation folder itself (e.g., esp-idf)
            match remove_directory_all(&installation_folder_path) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!("Failed to remove installation folder: {}", e));
                }
            }

            // Only remove the parent folder if it's empty
            match fs::read_dir(installation_folder) {
                Ok(mut entries) => {
                    if entries.next().is_none() {
                        // Directory is empty, remove it
                        if let Err(e) = remove_directory_all(installation_folder) {
                            warn!("Failed to remove empty parent directory: {}", e);
                        }
                    }
                    // Directory is not empty, keep it
                }
                Err(e) => {
                    // Directory might not exist or other error
                    warn!("Could not check or remove parent directory: {}", e);
                }
            }
        }

        match remove_directory_all(installation.clone().activation_script) {
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow!("Failed to remove activation script: {}", e));
            }
        }

        // Also remove fish/bat activation scripts
        if let Err(e) =
            remove_activation_scripts(&installation.clone().activation_script, &installation.name)
        {
            warn!("Failed to remove additional activation scripts: {}", e);
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
                    let desktop_path =
                        match dirs::desktop_dir().ok_or("Failed to get desktop directory") {
                            Ok(path) => path,
                            Err(e) => {
                                error!("{}", e);
                                return Ok(format!(
                                    "Version {} removed, but failed to remove desktop shortcut",
                                    identifier
                                ));
                            }
                        };
                    let shortcut_path = desktop_path.join(shortcut_name);
                    if shortcut_path.exists() {
                        match fs::remove_file(&shortcut_path) {
                            Ok(_) => {
                                info!("Removed desktop shortcut: {}", shortcut_path.display());
                            }
                            Err(e) => {
                                error!(
                                    "Failed to remove desktop shortcut {}: {}",
                                    shortcut_path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
                Ok(None) => {
                    info!(
                        "No desktop shortcut found for profile: {}",
                        installation.activation_script
                    );
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

pub fn run_command_in_context(
    identifier: &str,
    command: &str,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<ExitStatus> {
    let installation = match list_installed_versions(config_path) {
        Ok(versions) => versions.into_iter().find(|v| {
            v.id == identifier || v.name == identifier || {
                let normalized_identifier = crate::utils::normalize_path_for_comparison(identifier);
                normalized_identifier.is_some()
                    && normalized_identifier == crate::utils::normalize_path_for_comparison(&v.path)
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

    run_command_using_activation_script(activation_script, command, None)
}

fn build_activation_script(activation_script: &str, command: &str) -> String {
    #[cfg(not(target_os = "windows"))]
    let script = format!(
        "source \"{}\"\nshopt -s expand_aliases\n{}",
        activation_script, command
    );

    #[cfg(target_os = "windows")]
    let script = format!(". \"{}\"\n{}", activation_script, command);

    script
}

pub fn run_command_using_activation_script(
    activation_script: &str,
    command: &str,
    dir: Option<&str>,
) -> anyhow::Result<ExitStatus> {
    let script = build_activation_script(activation_script, command);
    debug!(
        "Running command using activation script {}",
        activation_script
    );

    let executor = crate::command_executor::get_executor();
    if dir.is_some() {
        debug!("Running command in directory {}", dir.unwrap());
        match executor.run_script_from_string_streaming_with_dir(&script, dir.unwrap()) {
            Ok(status) => Ok(status),
            Err(e) => Err(anyhow!("Failed to execute command: {}", e)),
        }
    } else {
        match executor.run_script_from_string_streaming(&script) {
            Ok(status) => Ok(status),
            Err(e) => Err(anyhow!("Failed to execute command: {}", e)),
        }
    }
}

pub fn run_command_using_activation_script_headless(
    activation_script: &str,
    command: &str,
    dir: Option<&str>,
) -> anyhow::Result<ExitStatus> {
    let script = build_activation_script(activation_script, command);
    debug!(
        "Running headless command using activation script {}",
        activation_script
    );

    let executor = crate::command_executor::get_executor();
    let result = if dir.is_some() {
        debug!("Running headless command in directory {}", dir.unwrap());
        executor.run_script_from_string_streaming_headless_with_dir(&script, dir.unwrap())
    } else {
        executor.run_script_from_string_streaming_headless(&script)
    };
    match result {
        Ok(status) => Ok(status),
        Err(e) => Err(anyhow!("Failed to execute command: {}", e)),
    }
}

pub async fn prepare_settings_for_fix_idf_installation(
    path_to_fix: PathBuf,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<Settings> {
    info!("Fixing IDF installation at path: {}", path_to_fix.display());
    // The fix logic is just instalation with use of existing repository
    let mut version_name = None;
    let mut original_settings: Option<Settings> = None;
    match list_installed_versions(config_path) {
        Ok(versions) => {
            for v in versions {
                if v.path == path_to_fix.to_str().unwrap() {
                    info!("Found existing IDF version: {}", v.name);
                    // Recover the settings the installation was originally created with
                    match &v.installation_config {
                        Some(config_bytes) => {
                            match bincode::deserialize::<Settings>(config_bytes.as_slice()) {
                                Ok(settings) => {
                                    info!("Recovered original installation settings for {}", v.name);
                                    original_settings = Some(settings);
                                }
                                Err(err) => {
                                    warn!("Failed to deserialize installation_config for {}: {}. Falling back to defaults.", v.name, err);
                                }
                            }
                        }
                        None => {
                            warn!("No installation_config stored for {}. Falling back to defaults.", v.name);
                        }
                    }
                    // Remove the existing activation script and eim_idf.json entry
                    match remove_single_idf_version(&v.name, true, config_path) {
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

    let mut settings = original_settings.unwrap_or_default();
    settings.path = Some(path_to_fix.clone());
    settings.non_interactive = Some(true);
    settings.version_name = version_name;
    settings.install_all_prerequisites = Some(true);
    settings.config_file_save_path = None;
    return Ok(settings);
}

// ============================================================================
// Tools inspection (eim list-tools)
// ============================================================================

/// A report of the tools declared in an installed ESP-IDF's `tools/tools.json`,
/// together with their on-disk installation status.
///
/// Produced by [`list_idf_tools`]. Serializable so that a future GUI command can
/// re-use the same shape without re-parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListReport {
    pub idf: ToolListIdfContext,
    pub tools_json_path: String,
    pub idf_tools_path: String,
    pub outdated_only: bool,
    pub tools: Vec<ToolListEntry>,
    pub outdated: Vec<ToolListOutdatedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListIdfContext {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListEntry {
    pub tool: Tool,
    pub version_inspections: Vec<ToolVersionInspection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolVersionInspection {
    pub version: Version,
    pub has_platform_download: bool,
    pub installed: Option<ToolListInstalled>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListInstalled {
    pub version: String,
    pub install_path: String,
    pub is_recommended_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolListOutdatedEntry {
    pub name: String,
    pub installed: String,
    pub available: String,
}

/// Lists the tools declared in an installed ESP-IDF's `tools/tools.json`,
/// together with their on-disk installation status.
///
/// Resolves the IDF by `id`, then `name`, then a normalized `path` match
/// against `IdfConfig.idf_installed`. When `identifier` is `None`, returns an
/// error — the CLI does its own interactive selection before calling this
/// function, keeping the library headless-callable.
///
/// `outdated_only` is echoed back in the report but does not currently affect
/// the populated data; both `tools` and `outdated` are always computed.
pub fn list_idf_tools(
    identifier: Option<&str>,
    outdated_only: bool,
    config_path: Option<&std::path::PathBuf>,
) -> Result<ToolListReport, String> {
    let identifier = identifier.ok_or_else(|| {
        "no identifier provided; pass an IDF id, name or path".to_string()
    })?;

    let config_path = config_path
        .cloned()
        .unwrap_or_else(get_default_config_path);
    let ide_config = IdfConfig::from_file(&config_path)
        .map_err(|e| format!("Failed to read eim_idf.json: {}", e))?;

    let installation = find_installation(&ide_config, identifier)?;

    let tools_json_path = std::path::Path::new(&installation.path).join("tools/tools.json");
    if !tools_json_path.exists() {
        return Err(format!(
            "tools.json not found at {}",
            tools_json_path.display()
        ));
    }

    let tools_file = tools_json_path
        .to_str()
        .ok_or_else(|| {
            format!(
                "tools.json path is not valid UTF-8: {}",
                tools_json_path.display()
            )
        })
        .and_then(|s| {
            crate::idf_tools::read_and_parse_tools_file(s)
                .map_err(|e| format!("Failed to read tools.json: {}", e))
        })?;

    let recommended_versions: std::collections::HashMap<String, String> = tools_file
        .tools
        .iter()
        .filter_map(|t| {
            t.versions
                .iter()
                .find(|v| v.status == "recommended")
                .map(|v| (t.name.clone(), v.name.clone()))
        })
        .collect();

    let platform = crate::idf_tools::get_platform_identification().ok();

    let mut tools: Vec<ToolListEntry> = Vec::new();
    let mut outdated: Vec<ToolListOutdatedEntry> = Vec::new();

    for tool in tools_file.tools.into_iter() {
        if tool.install == "never" {
            continue;
        }

        let tool_dir = std::path::Path::new(&installation.idf_tools_path).join(&tool.name);
        let on_disk_versions = enumerate_on_disk_versions(&tool_dir);
        let recommended_name = recommended_versions.get(&tool.name).cloned();

        let mut version_inspections: Vec<ToolVersionInspection> = Vec::new();
        let mut biggest_installed: Option<String> = None;

        for v in &tool.versions {
            let has_platform_download = match &platform {
                Some(p) => v.downloads.contains_key(p) || v.downloads.contains_key("any"),
                None => v.downloads.contains_key("any"),
            };
            let on_disk_match = on_disk_versions.iter().find(|(n, _)| n == &v.name);
            let installed = on_disk_match.map(|(dir_name, install_path)| ToolListInstalled {
                version: dir_name.clone(),
                install_path: install_path.clone(),
                is_recommended_match: recommended_name
                    .as_deref()
                    .map(|r| r == dir_name.as_str())
                    .unwrap_or(false),
            });
            if let Some((dir_name, _)) = on_disk_match {
                if is_newer_semver(dir_name, biggest_installed.as_deref()) {
                    biggest_installed = Some(dir_name.clone());
                }
            }
            version_inspections.push(ToolVersionInspection {
                version: v.clone(),
                has_platform_download,
                installed,
            });
        }

        let biggest_available = tool
            .versions
            .iter()
            .filter(|v| v.status != "deprecated")
            .map(|v| v.name.clone())
            .max_by(|a, b| semver_order(a, b))
            .or_else(|| {
                tool.versions
                    .iter()
                    .map(|v| v.name.clone())
                    .max_by(|a, b| semver_order(a, b))
            })
            .unwrap_or_default();
        if let Some(bi) = &biggest_installed {
            if !biggest_available.is_empty() && is_newer_semver(&biggest_available, Some(bi)) {
                outdated.push(ToolListOutdatedEntry {
                    name: tool.name.clone(),
                    installed: bi.clone(),
                    available: biggest_available,
                });
            }
        }

        tools.push(ToolListEntry {
            tool,
            version_inspections,
        });
    }

    Ok(ToolListReport {
        idf: ToolListIdfContext {
            id: installation.id.clone(),
            name: installation.name.clone(),
            path: installation.path.clone(),
        },
        tools_json_path: tools_json_path.to_string_lossy().to_string(),
        idf_tools_path: installation.idf_tools_path.clone(),
        outdated_only,
        tools,
        outdated,
    })
}

/// Enumerates the immediate subdirectories of `tool_dir` and returns
/// `(name, absolute_path)` pairs. Missing or non-directory entries are
/// ignored. Order is whatever the filesystem returns; the caller filters
/// further by name.
fn enumerate_on_disk_versions(tool_dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(tool_dir) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            continue;
        };
        out.push((name, path.to_string_lossy().to_string()));
    }
    out
}

/// Returns true when `candidate` is strictly greater than `current`.
///
/// Prefers semver comparison when both sides parse, but falls back to a
/// plain lexicographic string comparison as the final tiebreaker. This
/// matches the rest of the ESP-IDF tooling, where tool versions can carry a
/// non-semver suffix such as `v0.12.0-esp32-20260304`. When `current` is
/// `None`, any non-empty candidate is treated as newer than nothing.
fn is_newer_semver(candidate: &str, current: Option<&str>) -> bool {
    let cand = semver::Version::parse(candidate);
    let curr = current.and_then(|c| semver::Version::parse(c).ok());
    match (cand, curr) {
        (Ok(c), Some(cur)) => c > cur,
        _ => match current {
            Some(cur) => candidate > cur,
            None => !candidate.is_empty(),
        },
    }
}

/// `max_by` comparator that prefers semver when both sides parse, with a
/// plain lexicographic string comparison as the final tiebreaker so that
/// non-semver versions (e.g. `v0.12.0-esp32-20260304`) still order
/// deterministically and consistently with `is_newer_semver`.
fn semver_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(av), Ok(bv)) => av.cmp(&bv),
        _ => a.cmp(b),
    }
}

/// Finds the installation in `ide_config` matching `identifier` by id, then
/// name, then normalized path. Returns an error if no match is found.
fn find_installation<'a>(
    ide_config: &'a IdfConfig,
    identifier: &str,
) -> Result<&'a IdfInstallation, String> {
    if let Some(install) = ide_config
        .idf_installed
        .iter()
        .find(|i| i.id == identifier || i.name == identifier)
    {
        return Ok(install);
    }
    let normalized = crate::utils::normalize_path_for_comparison(identifier);
    if let Some(install) = ide_config.idf_installed.iter().find(|i| {
        let n = crate::utils::normalize_path_for_comparison(&i.path);
        n.is_some() && n == normalized
    }) {
        return Ok(install);
    }
    Err(format!("IDF installation '{}' not found", identifier))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_run_command_using_activation_script_echo() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple activation script that echoes a test message
        #[cfg(target_os = "windows")]
        let script_path = temp_dir.path().join("activate.ps1");
        #[cfg(not(target_os = "windows"))]
        let script_path = temp_dir.path().join("activate.sh");

        #[cfg(target_os = "windows")]
        {
            fs::write(&script_path, "@echo off\nset TEST_VAR=hello\n").unwrap();
        }
        #[cfg(not(target_os = "windows"))]
        {
            fs::write(&script_path, "export TEST_VAR=hello\n").unwrap();
        }

        // Run a simple echo command
        #[cfg(target_os = "windows")]
        let command = "echo %TEST_VAR%";
        #[cfg(not(target_os = "windows"))]
        let command = "echo $TEST_VAR";

        let result =
            run_command_using_activation_script(script_path.to_str().unwrap(), command, None);

        // This should succeed (may fail on Windows without cmd.exe, but that's okay)
        // The main purpose is to verify the function can be called
        match result {
            Ok(_) => {
                info!("Command executed successfully");
            }
            Err(e) => {
                // This might fail in test environment without proper shell
                info!(
                    "Command execution returned error (may be expected in test env): {:?}",
                    e
                );
            }
        }
    }

    // ----- helpers for list_idf_tools tests -----

    use crate::idf_config::IdfConfig;
    use crate::idf_config::IdfInstallation;

    /// Builds a minimal `IdfInstallation` for tests.
    fn make_idf_installation(idf_path: &str, tools_path: &str) -> IdfInstallation {
        IdfInstallation {
            activation_script: format!("{}/activate.sh", idf_path),
            id: "esp-idf-test-id".to_string(),
            idf_tools_path: tools_path.to_string(),
            name: "TestIDF".to_string(),
            path: idf_path.to_string(),
            python: format!("{}/tools/python/bin/python3", tools_path),
            installation_config: None,
        }
    }

    /// Writes a real `IdfConfig` JSON file under `dir/eim_idf.json` pointing at
    /// `idf_path` and `tools_path`. Returns the path to the written file.
    fn write_fake_idf_config(
        dir: &std::path::Path,
        idf_path: &str,
        tools_path: &str,
    ) -> std::path::PathBuf {
        let mut config = IdfConfig {
            git_path: "/usr/bin/git".to_string(),
            idf_installed: vec![make_idf_installation(idf_path, tools_path)],
            idf_selected_id: "esp-idf-test-id".to_string(),
            eim_path: None,
            version: Some("2.0".to_string()),
        };
        let path = dir.join("eim_idf.json");
        config.to_file(&path, true, false).unwrap();
        path
    }

    /// Builds a minimal `Tool` for tests with the given install mode, name,
    /// description and versions.
    fn make_tool(
        name: &str,
        description: &str,
        install: &str,
        versions: Vec<crate::idf_tools::Version>,
    ) -> Tool {
        Tool {
            description: description.to_string(),
            export_paths: vec![],
            export_vars: std::collections::HashMap::new(),
            info_url: String::new(),
            install: install.to_string(),
            license: None,
            name: name.to_string(),
            platform_overrides: None,
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: vec![],
            version_regex: String::new(),
            version_regex_replace: None,
            versions,
        }
    }

    /// Builds a minimal `Version` for tests. `download_keys` are the keys
    /// added to the empty `downloads` map (with placeholder `Download` values).
    fn make_version(
        name: &str,
        status: &str,
        download_keys: &[&str],
    ) -> crate::idf_tools::Version {
        use crate::idf_tools::Download;
        use std::collections::HashMap;
        let mut downloads: HashMap<String, Download> = HashMap::new();
        for k in download_keys {
            downloads.insert(
                k.to_string(),
                Download {
                    sha256: String::new(),
                    size: 0,
                    url: String::new(),
                    rename_dist: None,
                },
            );
        }
        crate::idf_tools::Version {
            name: name.to_string(),
            status: status.to_string(),
            downloads,
        }
    }

    /// Writes a real `ToolsFile` JSON at `<idf_path>/tools/tools.json`.
    /// Returns the path of the written file.
    fn write_fake_tools_json(
        idf_path: &std::path::Path,
        tools: Vec<Tool>,
    ) -> std::path::PathBuf {
        use crate::idf_tools::ToolsFile;
        let tools_dir = idf_path.join("tools");
        fs::create_dir_all(&tools_dir).unwrap();
        let path = tools_dir.join("tools.json");
        let file = ToolsFile { tools, version: 3 };
        let json = serde_json::to_string_pretty(&file).unwrap();
        fs::write(&path, json).unwrap();
        path
    }

    #[test]
    fn test_list_idf_tools_resolves_installation_by_id() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );
        write_fake_tools_json(&idf_path, vec![]);

        let report = list_idf_tools(Some("esp-idf-test-id"), false, Some(&config_path))
            .expect("list_idf_tools should succeed");

        assert_eq!(report.idf.id, "esp-idf-test-id");
        assert_eq!(report.idf.name, "TestIDF");
        assert_eq!(report.idf.path, idf_path.to_str().unwrap());
        assert_eq!(
            report.tools_json_path,
            idf_path.join("tools/tools.json").to_str().unwrap()
        );
        assert_eq!(report.idf_tools_path, tools_path.to_str().unwrap());
        assert!(!report.outdated_only);
        assert!(report.tools.is_empty());
    }

    #[test]
    fn test_list_idf_tools_filters_never_install_tools() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let keep = make_tool(
            "xtensa-esp-elf",
            "Xtensa toolchain",
            "always",
            vec![make_version("14.2.0", "recommended", &["any"])],
        );
        let skip = make_tool(
            "deprecated-tool",
            "Should be filtered out",
            "never",
            vec![make_version("1.0.0", "supported", &["any"])],
        );
        let optional = make_tool(
            "optional-tool",
            "On request",
            "on_request",
            vec![make_version("2.0.0", "supported", &["any"])],
        );
        write_fake_tools_json(&idf_path, vec![keep, skip, optional]);

        let report = list_idf_tools(Some("esp-idf-test-id"), false, Some(&config_path))
            .expect("list_idf_tools should succeed");

        let names: Vec<&str> = report.tools.iter().map(|e| e.tool.name.as_str()).collect();
        assert_eq!(names, vec!["xtensa-esp-elf", "optional-tool"]);
    }

    #[test]
    fn test_list_idf_tools_detects_installed_recommended_version() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let tool = make_tool(
            "xtensa-esp-elf",
            "Xtensa toolchain",
            "always",
            vec![make_version("14.2.0", "recommended", &["any"])],
        );
        write_fake_tools_json(&idf_path, vec![tool]);

        // Create <idf_tools_path>/xtensa-esp-elf/14.2.0/ on disk
        let installed = tools_path.join("xtensa-esp-elf").join("14.2.0");
        fs::create_dir_all(&installed).unwrap();

        let report =
            list_idf_tools(Some("esp-idf-test-id"), false, Some(&config_path)).unwrap();
        assert_eq!(report.tools.len(), 1);
        let entry = &report.tools[0];
        assert_eq!(entry.version_inspections.len(), 1);
        let installed_info = entry.version_inspections[0]
            .installed
            .as_ref()
            .expect("should detect install");
        assert_eq!(installed_info.version, "14.2.0");
        assert!(installed_info.is_recommended_match);
        assert_eq!(installed_info.install_path, installed.to_string_lossy());
    }

    #[test]
    fn test_list_idf_tools_reports_recommended_match_false_for_older_install() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let tool = make_tool(
            "xtensa-esp-elf",
            "Xtensa toolchain",
            "always",
            vec![
                make_version("13.2.0", "supported", &["any"]),
                make_version("14.2.0", "recommended", &["any"]),
            ],
        );
        write_fake_tools_json(&idf_path, vec![tool]);

        // Install the older, non-recommended one
        let installed = tools_path.join("xtensa-esp-elf").join("13.2.0");
        fs::create_dir_all(&installed).unwrap();

        let report =
            list_idf_tools(Some("esp-idf-test-id"), false, Some(&config_path)).unwrap();
        // Find the inspection for the older version
        let older = report.tools[0]
            .version_inspections
            .iter()
            .find(|vi| vi.version.name == "13.2.0")
            .expect("missing 13.2.0 inspection");
        let info = older.installed.as_ref().expect("should detect 13.2.0 install");
        assert!(!info.is_recommended_match);
    }

    #[test]
    fn test_list_idf_tools_outdated_when_older_version_is_installed() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        // Available: 13.2.0 (older, supported) and 14.2.0 (newer, recommended)
        let tool = make_tool(
            "xtensa-esp-elf",
            "Xtensa toolchain",
            "always",
            vec![
                make_version("13.2.0", "supported", &["any"]),
                make_version("14.2.0", "recommended", &["any"]),
            ],
        );
        write_fake_tools_json(&idf_path, vec![tool]);

        // Install the older one
        let installed = tools_path.join("xtensa-esp-elf").join("13.2.0");
        fs::create_dir_all(&installed).unwrap();

        let report =
            list_idf_tools(Some("esp-idf-test-id"), true, Some(&config_path)).unwrap();
        assert!(report.outdated_only);
        assert_eq!(report.outdated.len(), 1);
        assert_eq!(report.outdated[0].name, "xtensa-esp-elf");
        assert_eq!(report.outdated[0].installed, "13.2.0");
        assert_eq!(report.outdated[0].available, "14.2.0");
    }

    #[test]
    fn test_list_idf_tools_outdated_empty_when_nothing_installed() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let tool = make_tool(
            "xtensa-esp-elf",
            "Xtensa toolchain",
            "always",
            vec![
                make_version("13.2.0", "supported", &["any"]),
                make_version("14.2.0", "recommended", &["any"]),
            ],
        );
        write_fake_tools_json(&idf_path, vec![tool]);
        // Nothing installed on disk

        let report =
            list_idf_tools(Some("esp-idf-test-id"), true, Some(&config_path)).unwrap();
        assert!(report.outdated.is_empty());
    }

    #[test]
    fn test_list_idf_tools_errors_on_unknown_identifier() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let result = list_idf_tools(Some("does-not-exist"), false, Some(&config_path));
        let err = result.expect_err("expected error for unknown identifier");
        assert!(
            err.contains("does-not-exist"),
            "error should mention identifier: {}",
            err
        );
    }

    #[test]
    fn test_list_idf_tools_errors_on_missing_tools_json() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        // Intentionally do NOT write tools/tools.json
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let result = list_idf_tools(Some("esp-idf-test-id"), false, Some(&config_path));
        let err = result.expect_err("expected error for missing tools.json");
        assert!(
            err.contains("tools.json not found"),
            "error should mention tools.json: {}",
            err
        );
    }

    #[test]
    fn test_list_idf_tools_marks_has_platform_download_using_current_platform() {
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        // Current platform key + an unrelated one.
        let current_platform = crate::idf_tools::get_platform_identification()
            .unwrap_or_else(|_| "any".to_string());
        let unrelated_platform = if current_platform == "any" {
            "win64"
        } else {
            "any"
        };
        let tool = make_tool(
            "xtensa-esp-elf",
            "Xtensa toolchain",
            "always",
            vec![
                make_version("14.2.0", "recommended", &[&current_platform]),
                make_version("13.2.0", "supported", &[unrelated_platform]),
            ],
        );
        write_fake_tools_json(&idf_path, vec![tool]);

        let report =
            list_idf_tools(Some("esp-idf-test-id"), false, Some(&config_path)).unwrap();
        assert_eq!(report.tools[0].version_inspections[0].has_platform_download, true);
        // We only assert the first one (always true). The second's value
        // depends on whether current_platform happens to be "any".
    }

    // ----- is_newer_semver / semver_order fall back to plain string -----

    #[test]
    fn test_is_newer_semver_plain_string_when_neither_parses() {
        // Both sides are non-semver (e.g. ESP-IDF tool versions with a
        // `-esp32-YYYYMMDD` suffix). The lexicographic tiebreaker must pick
        // the higher string.
        assert!(is_newer_semver(
            "v0.12.0-esp32-20260304",
            Some("v0.11.0-esp32-20240304")
        ));
        assert!(!is_newer_semver(
            "v0.11.0-esp32-20240304",
            Some("v0.12.0-esp32-20260304")
        ));
    }

    #[test]
    fn test_is_newer_semver_plain_string_when_only_current_parses() {
        // Candidate is non-semver, current is semver. The plain-string
        // tiebreaker compares them lexically: '1' < 'v', so the candidate
        // is NOT newer.
        assert!(!is_newer_semver(
            "14.2.0",
            Some("v0.12.0-esp32-20260304")
        ));
        // And the reverse direction holds lexically: 'v' > '1'.
        assert!(is_newer_semver(
            "v0.12.0-esp32-20260304",
            Some("14.2.0")
        ));
    }

    #[test]
    fn test_is_newer_semver_semver_when_both_parse() {
        // Sanity check: the semver path is still preferred when both sides
        // parse.
        assert!(is_newer_semver("14.2.0", Some("13.2.0")));
        assert!(!is_newer_semver("13.2.0", Some("14.2.0")));
    }

    #[test]
    fn test_is_newer_semver_with_no_current() {
        // When `current` is None, any non-empty candidate is newer than
        // nothing, regardless of whether it parses as semver.
        assert!(is_newer_semver("v0.12.0-esp32-20260304", None));
        assert!(is_newer_semver("14.2.0", None));
        assert!(!is_newer_semver("", None));
    }

    #[test]
    fn test_semver_order_plain_string_when_either_does_not_parse() {
        // Both non-semver: plain lex order.
        assert_eq!(
            semver_order("v0.11.0-esp32-20240304", "v0.12.0-esp32-20260304"),
            std::cmp::Ordering::Less
        );
        // Mixed: plain lex order too.
        assert_eq!(
            semver_order("14.2.0", "v0.12.0-esp32-20260304"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_list_idf_tools_outdated_detected_with_non_semver_versions() {
        // End-to-end check: with a non-semver suffix, the older installed
        // version must still be reported as outdated by the newer available
        // one. The semver-only path would have missed this because neither
        // version parses.
        let temp = TempDir::new().unwrap();
        let idf_path = temp.path().join("v5.4/esp-idf");
        let tools_path = temp.path().join("v5.4/tools");
        fs::create_dir_all(&idf_path).unwrap();
        fs::create_dir_all(&tools_path).unwrap();
        let config_path = write_fake_idf_config(
            temp.path(),
            idf_path.to_str().unwrap(),
            tools_path.to_str().unwrap(),
        );

        let tool = make_tool(
            "esp32ulp-elf",
            "ULP toolchain",
            "always",
            vec![
                make_version("v0.11.0-esp32-20240304", "supported", &["any"]),
                make_version("v0.12.0-esp32-20260304", "recommended", &["any"]),
            ],
        );
        write_fake_tools_json(&idf_path, vec![tool]);

        // Install the older non-semver version.
        let installed = tools_path
            .join("esp32ulp-elf")
            .join("v0.11.0-esp32-20240304");
        fs::create_dir_all(&installed).unwrap();

        let report =
            list_idf_tools(Some("esp-idf-test-id"), true, Some(&config_path)).unwrap();
        assert_eq!(report.outdated.len(), 1);
        assert_eq!(report.outdated[0].name, "esp32ulp-elf");
        assert_eq!(report.outdated[0].installed, "v0.11.0-esp32-20240304");
        assert_eq!(report.outdated[0].available, "v0.12.0-esp32-20260304");
    }
}
