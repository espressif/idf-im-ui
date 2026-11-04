use anyhow::{anyhow, Result};
use log::debug;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use sysinfo::System;

use crate::command_executor::execute_command_with_env;
use crate::utils::{find_by_name_and_extension, find_directories_by_name, versions_match};
use crate::{decompress_archive, download_file, verify_file_checksum, DownloadProgress};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Tool {
    pub description: String,
    pub export_paths: Vec<Vec<String>>,
    pub export_vars: HashMap<String, String>,
    pub info_url: String,
    pub install: String,
    #[serde(default)]
    pub license: Option<String>,
    pub name: String,
    #[serde(default)]
    pub platform_overrides: Option<Vec<PlatformOverride>>,
    #[serde(default)]
    pub supported_targets: Option<Vec<String>>,
    #[serde(default)]
    pub strip_container_dirs: Option<u8>,
    pub version_cmd: Vec<String>,
    pub version_regex: String,
    #[serde(default)]
    pub version_regex_replace: Option<String>,
    pub versions: Vec<Version>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct PlatformOverride {
    #[serde(default)]
    pub install: Option<String>,
    pub platforms: Vec<String>,
    #[serde(default)]
    pub export_paths: Option<Vec<Vec<String>>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Version {
    pub name: String,
    pub status: String,
    #[serde(flatten)]
    pub downloads: HashMap<String, Download>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Download {
    pub sha256: String,
    pub size: u64,
    pub url: String,
    #[serde(default)]
    pub rename_dist: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ToolsFile {
    pub tools: Vec<Tool>,
    pub version: u8,
}

#[derive(Debug, PartialEq)]
pub enum ToolStatus {
    Missing,
    DifferentVersion { installed: String, expected: String },
    Correct { version: String },
}

/// Reads and parses the tools file from the given path.
///
/// # Arguments
///
/// * `path` - A string slice representing the path to the tools file.
///
/// # Returns
///
/// * `Result<ToolsFile, Box<dyn std::error::Error>>` - On success, returns a `ToolsFile` instance.
///   On error, returns a `Box<dyn std::error::Error>` containing the error details.
pub fn read_and_parse_tools_file(path: &str) -> Result<ToolsFile, Box<dyn std::error::Error>> {
    let path = Path::new(path);
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let tools_file: ToolsFile = serde_json::from_str(&contents)?;
    let platform = get_platform_identification()?;

    Ok(apply_platform_overrides(tools_file, &platform))
}

/// Applies platform-specific overrides to the tools within a `ToolsFile`.
///
/// This function iterates through each tool in the provided `ToolsFile` and checks
/// if it has any `platform_overrides` defined. If overrides exist, it searches
/// for an override entry that matches the given `platform` string.
///
/// Upon finding a matching override, it applies the specified `install` and
/// `export_paths` from the override entry to the tool. Only the first matching
/// override for a tool is applied.
///
/// After processing, the `platform_overrides` field for each tool is set to `None`
/// to ensure the function is idempotent (running it multiple times with the same
/// inputs will produce the same result).
///
/// # Arguments
///
/// * `tools_file` - A `ToolsFile` struct, which will be mutated to apply overrides.
/// * `platform` - A string slice representing the current platform (e.g., "win64", "linux-amd64", "macos-arm64").
///
/// # Returns
///
/// The modified `ToolsFile` with platform-specific overrides applied and
/// `platform_overrides` fields cleared.
pub fn apply_platform_overrides(mut tools_file: ToolsFile, platform: &str) -> ToolsFile {
    for tool in &mut tools_file.tools {
        if let Some(platform_overrides) = &tool.platform_overrides {
            let mut override_applied = false;

            for override_entry in platform_overrides {
                if override_entry.platforms.iter().any(|p| p == platform) {
                    debug!(
                        "Applying platform override for tool '{}' on platform '{}'",
                        tool.name, platform
                    );

                    // Apply install override if present
                    if let Some(install) = &override_entry.install {
                        debug!(
                            "  - Overriding install: '{}' -> '{}'",
                            tool.install, install
                        );
                        tool.install = install.clone();
                    }

                    // Apply export_paths override if present
                    if let Some(export_paths) = &override_entry.export_paths {
                        debug!("  - Overriding export_paths ({} paths)", export_paths.len());
                        tool.export_paths = export_paths.clone();
                    }

                    override_applied = true;
                    break; // Apply only the first matching override
                }
            }

            if !override_applied {
                debug!(
                    "No matching platform override for tool '{}' on platform '{}' (has {} override(s) defined)",
                    tool.name,
                    platform,
                    platform_overrides.len()
                );
            }
        }

        // Remove platform_overrides to make the function idempotent
        tool.platform_overrides = None;
    }

    tools_file
}

/// Removes the specified number of top directory levels when extracting an archive.
/// If the operation fails, the original directory is restored.
/// E.g. if levels=2, archive path a/b/c/d.txt will be extracted as c/d.txt.
///
/// # Arguments
///
/// * `path` - The path to the extracted archive directory
/// * `levels` - The number of directory levels to strip
///
/// # Returns
///
/// * `Result<()>` - Ok if successful, Err otherwise
fn do_strip_container_dirs(path: &Path, levels: u8) -> Result<()> {
    if levels == 0 {
        return Ok(());
    }

    let tmp_path = path.with_extension("tmp");

    // Clean up any existing tmp directory and move current path to tmp
    if tmp_path.exists() {
        std::fs::remove_dir_all(&tmp_path)?;
    }
    std::fs::rename(path, &tmp_path)?;

    // Define rollback function
    let rollback = || {
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            log::error!("Failed to rollback after strip_container_dirs error: {}", e);
        }
    };

    // Navigate down through the specified levels
    let base_path = match (0..levels).try_fold(tmp_path.clone(), |current_path, level| {
        let mut entries = std::fs::read_dir(current_path)?;

        let entry = entries
            .next()
            .ok_or_else(|| anyhow!("at level {}, directory is empty", level))??;

        // Check if there's only one entry
        if entries.next().is_some() {
            return Err(anyhow!(
                "at level {}, expected 1 entry, found multiple",
                level
            ));
        }

        let next_path = entry.path();
        if !next_path.is_dir() {
            return Err(anyhow!(
                "at level {}, '{}' is not a directory",
                level,
                entry.file_name().to_string_lossy()
            ));
        }

        Ok(next_path)
    }) {
        Ok(path) => path,
        Err(e) => {
            rollback();
            return Err(e);
        }
    };

    // Recreate the original directory and move contents
    if let Err(e) = std::fs::create_dir(path) {
        rollback();
        return Err(e.into());
    }

    for entry in std::fs::read_dir(base_path)? {
        let entry = entry?;
        if let Err(e) = std::fs::rename(entry.path(), path.join(entry.file_name())) {
            // Try to clean up the partially created directory
            let _ = std::fs::remove_dir_all(path);
            rollback();
            return Err(e.into());
        }
    }

    // Clean up temporary directory
    std::fs::remove_dir_all(&tmp_path)?;

    Ok(())
}

/// Filters a list of tools based on the given target platform.
///
/// # Arguments
///
/// * `tools` - A vector of `Tool` instances to be filtered. Each `Tool` contains information about a tool,
///   such as its supported targets and other relevant details.
///
/// * `target` - A reference to a vector of strings representing the target platforms. The function will
///   filter the tools based on whether they support any of the specified target platforms.
///
/// # Returns
///
/// * A vector of `Tool` instances that match at least one of the given target platforms. If no matching tools
///   are found, an empty vector is returned.
///
pub fn filter_tools_by_target(tools: Vec<Tool>, target: &[String]) -> Vec<Tool> {
    tools
        .into_iter()
        .filter(|tool| {
            if target.contains(&"all".to_string()) {
                return true;
            }
            if let Some(supported_targets) = &tool.supported_targets {
                target.iter().any(|t| supported_targets.contains(t))
                    || supported_targets.contains(&"all".to_string())
            } else {
                true
            }
        })
        .collect()
}

/// Returns a standardized platform identifier string based on the current system.
///
/// This function maps the system's OS and architecture to a more common and standardized
/// identifier (e.g., "win64", "macos-arm64").
///
/// Uses `sysinfo` to detect the actual running system at runtime, which provides better
/// compatibility and accurate detection (e.g., properly detecting Windows 11).
///
/// # Errors
///
/// Returns an `Err` containing a `String` if the current platform is not
/// explicitly supported and mapped within the function.
///
/// # Examples
///
/// ```
/// match get_platform_identification() {
///     Ok(platform_id) => {
///         // On a 64-bit Windows system, platform_id might be "win64"
///         // On a macOS M1 system, platform_id might be "macos-arm64"
///         println!("Platform ID: {}", platform_id);
///     },
///     Err(e) => {
///         eprintln!("Error getting platform ID: {}", e);
///     }
/// }
/// ```
///
/// # Returns
///
/// A `Result` which is:
/// - `Ok(String)`: A `String` representing the standardized platform identifier.
/// - `Err(String)`: An error message if the platform is unsupported.
pub fn get_platform_identification() -> Result<String, String> {
    let mut platform_from_name = HashMap::new();

    // Windows identifiers
    platform_from_name.insert("windows-x86", "win32");
    platform_from_name.insert("windows-x86_64", "win64");
    platform_from_name.insert("windows-aarch64", "win64");

    // macOS identifiers
    platform_from_name.insert("macos-x86_64", "macos");
    platform_from_name.insert("macos-aarch64", "macos-arm64");

    // Linux identifiers
    platform_from_name.insert("linux-x86", "linux-i686");
    platform_from_name.insert("linux-x86_64", "linux-amd64");
    platform_from_name.insert("linux-aarch64", "linux-arm64");
    platform_from_name.insert("linux-arm", "linux-armel");
    platform_from_name.insert("linux-armv7", "linux-armhf");

    // FreeBSD identifiers
    platform_from_name.insert("freebsd-x86_64", "linux-amd64");
    platform_from_name.insert("freebsd-x86", "linux-i686");

    let platform_string = get_platform_definition();

    let platform = match platform_from_name.get(&platform_string.as_str()) {
        Some(platform) => platform,
        None => return Err(format!("Unsupported platform: {}", platform_string)),
    };
    Ok(platform.to_string())
}

/// Returns a string representing the current platform in the format "os-arch".
///
/// This function retrieves the operating system and architecture information
/// at runtime using `sysinfo`, with fallback to compile-time constants.
/// This provides better accuracy for OS detection (e.g., Windows 11 vs Windows 10).
///
/// # Examples
///
/// ```
/// let platform = get_platform_definition();
/// // On a 64-bit Linux system, platform might be "linux-x86_64"
/// // On a macOS M1 system, platform might be "macos-aarch64"
/// println!("{}", platform);
/// ```
///
/// # Returns
///
/// A `String` in the format "os-arch" (e.g., "linux-x86_64", "macos-aarch64").
pub fn get_platform_definition() -> String {
    let os = get_os_name();
    let arch = get_arch_name();

    format!("{}-{}", os, arch)
}

/// Gets the operating system name using sysinfo with fallback to compile-time constant.
///
/// Returns a normalized OS name compatible with the existing platform identification system.
fn get_os_name() -> String {
    // Try runtime detection first
    if let Some(name) = System::name() {
        let name_lower = name.to_lowercase();

        // Normalize OS names to match our existing convention
        if name_lower.contains("windows") {
            return "windows".to_string();
        } else if name_lower.contains("darwin")
            || name_lower.contains("macos")
            || name_lower.contains("mac os")
        {
            return "macos".to_string();
        } else if name_lower.contains("linux") {
            return "linux".to_string();
        } else if name_lower.contains("freebsd") {
            return "freebsd".to_string();
        }
    }

    // Fallback to compile-time constant
    std::env::consts::OS.to_string()
}

/// Gets the architecture name using sysinfo with fallback to compile-time constant.
///
/// Returns a normalized architecture name compatible with the existing platform identification system.
fn get_arch_name() -> String {
    // Try runtime detection first
    let arch_lower = System::cpu_arch().to_lowercase();

    // Normalize architecture names to match our existing convention
    if arch_lower.contains("x86_64") || arch_lower.contains("amd64") {
        return "x86_64".to_string();
    } else if arch_lower.contains("aarch64") || arch_lower.contains("arm64") {
        return "aarch64".to_string();
    } else if arch_lower.contains("i686")
        || arch_lower.contains("x86") && !arch_lower.contains("x86_64")
    {
        return "x86".to_string();
    } else if arch_lower.contains("arm") && !arch_lower.contains("aarch64") {
        // Check for hard-float (armhf) vs soft-float (armel)
        #[cfg(target_arch = "arm")]
        {
            // For ARM, check target features or target string for hard-float
            // armv7-unknown-linux-musleabihf uses hard-float ABI
            if cfg!(target_feature = "vfp")
                || cfg!(target_feature = "vfpv3")
                || cfg!(target_feature = "vfpv4")
            {
                return "armv7".to_string();
            }
        }
        return "arm".to_string();
    }

    // For cross-compilation, check target to distinguish armhf from armel
    #[cfg(target_arch = "arm")]
    {
        if cfg!(any(target_os = "linux", target_os = "nuttx")) {
            let target = option_env!("TARGET").unwrap_or("");
            if target.contains("eabihf") || target.contains("gnueabihf") {
                return "armv7".to_string();
            }
        }
    }

    std::env::consts::ARCH.to_string()
}

/// Given a list of `Tool` structs and a target platform, this function returns a HashMap
/// that maps tool names to a tuple containing their preferred version name and the
/// corresponding `Download` link for that platform.
///
/// For each tool, the function determines the preferred version based on the following logic:
/// - If there's a version with status "recommended", it's chosen.
/// - If no "recommended" version exists but multiple versions are present, the first one in the list is used.
/// - If only one version exists, that version is used.
/// - If a tool has no versions, a warning is logged and the tool is skipped.
///
/// After determining the preferred version, the function attempts to find a download link
/// specific to the provided `platform`. If a download link for the platform is found,
/// it's added to the result HashMap along with the preferred version's name.
///
/// # Arguments
///
/// * `tools` - A `Vec` of `Tool` structs, each potentially containing multiple versions and download links.
/// * `platform` - A string slice representing the target platform (e.g., "windows", "linux", "macos").
///
/// # Returns
///
/// A `HashMap<String, (String, Download)>` where keys are tool names and values are a tuple
/// containing the preferred version's name (String) and the `Download` struct relevant
/// to the specified `platform` for that preferred tool version.
pub fn get_download_link_by_platform(
    tools: Vec<Tool>,
    platform: &String,
) -> HashMap<String, (String, Download)> {
    let mut tool_links = HashMap::new();
    for tool in tools {
        let preferred_version = if tool.versions.len() > 1 {
            log::info!(
                "Tool {} has multiple versions, using the recommended or the first one",
                tool.name
            );
            tool.versions
                .iter()
                .find(|v| v.status == "recommended")
                .unwrap_or(&tool.versions[0])
        } else if tool.versions.is_empty() {
            log::warn!("Tool {} has no versions", tool.name);
            continue;
        } else {
            tool.versions.first().unwrap()
        };
        if let Some(download) = preferred_version
            .downloads
            .get(platform)
            .or_else(|| preferred_version.downloads.get("any"))
        {
            tool_links.insert(
                tool.name.clone(),
                (preferred_version.name.clone(), download.clone()),
            );
        } else {
            log::warn!(
                "Tool {} does not have a download link for platform {}",
                tool.name,
                platform
            );
        }
    }
    tool_links
}

/// Changes the download links of tools to use a specified mirror.
///
/// # Arguments
///
/// * `tools` - A HashMap where keys are tool names (String) and values are
///   tuples containing the tool's version (String) and its corresponding
///   Download instance.
/// * `mirror` - An optional reference to a string representing the mirror URL.
///   If `None`, the original URLs are used.
///
/// # Returns
///
/// * A new HashMap with the same keys and version strings as the input `tools`,
///   but with updated Download instances. The URLs within these Download instances
///   are replaced with the mirror URL if provided, specifically by
///   replacing "https://github.com" with the mirror URL.
///
pub fn change_links_donwanload_mirror(
    tools: HashMap<String, (String, Download)>,
    mirror: Option<&str>,
) -> HashMap<String, (String, Download)> {
    let new_tools: HashMap<String, (String, Download)> = tools
        .iter()
        .map(|(name, (version, link))| {
            let new_link = match mirror {
                Some(mirror) => Download {
                    sha256: link.sha256.clone(),
                    size: link.size,
                    url: link.url.replace("https://github.com", mirror),
                    rename_dist: link.rename_dist.clone(),
                },
                None => link.clone(),
            };
            (name.to_string(), (version.clone(), new_link))
        })
        .collect();
    new_tools
}

/// Retrieves a HashMap of tool names and their corresponding Download instances based on the given platform.
///
/// # Parameters
///
/// * `tools_file`: A `ToolsFile` instance containing the list of tools and their versions.
/// * `selected_chips`: A vector of strings representing the selected chips.
/// * `mirror`: An optional reference to a string representing the mirror URL. If `None`, the original URLs are used.
///
/// # Return
///
/// * A HashMap where the keys are tool names and the values are Download instances.
///   If a tool does not have a download for the given platform, it is not included in the HashMap.
///
pub fn get_list_of_tools_to_download(
    tools_file: ToolsFile,
    selected_chips: Vec<String>,
    mirror: Option<&str>,
) -> HashMap<String, (String, Download)> {
    let list = filter_tools_by_target(tools_file.tools, &selected_chips);
    let platform = match get_platform_identification() {
        Ok(platform) => platform,
        Err(err) => {
            panic!("Unable to identify platform: {}", err);
        }
    };
    change_links_donwanload_mirror(get_download_link_by_platform(list, &platform), mirror)
}

/// Retrieves a vector of strings representing the export paths for the tools.
///
/// This function creates export paths for the tools based on their `export_paths` and the `tools_install_path`.
/// It also checks for duplicate export paths and logs them accordingly.
///
/// # Parameters
///
/// * `tools_file`: A `ToolsFile` instance containing the list of tools and their versions.
/// * `selected_chip`: A vector of strings representing the selected chips.
/// * `tools_install_path`: A reference to a string representing the installation path for the tools.
///
/// # Return
///
/// * A vector of strings representing the export paths for the tools.
///
pub fn get_tools_export_paths(
    tools_file: ToolsFile,
    selected_chip: Vec<String>,
    tools_install_path: &str,
) -> Vec<String> {
    let bin_dirs = find_bin_directories(Path::new(tools_install_path));
    log::debug!("Bin directories: {:?}", bin_dirs);

    let list = filter_tools_by_target(tools_file.tools, &selected_chip);
    debug!("Creating export paths for: {:?}", list);
    debug!("Creating export paths from path: {:?}", tools_install_path);
    let mut paths_set: HashSet<String> = HashSet::new();
    for tool in &list {
        tool.export_paths.iter().for_each(|path| {
            let mut p = PathBuf::new();
            p.push(tools_install_path);
            for level in path {
                p.push(level);
            }
            if p.try_exists().is_ok() {
                paths_set.insert(p.to_str().unwrap().to_string());
            } else {
                log::warn!("Export path does not exist: {}", p.to_str().unwrap());
            }
        });
    }
    for bin_dir in bin_dirs {
        if Path::new(&bin_dir).try_exists().is_ok() {
            let str_p = bin_dir;
            paths_set.insert(str_p);
        } else {
            log::warn!("Bin directory does not exist: {}", bin_dir);
        }
    }
    let mut paths: Vec<String> = paths_set.into_iter().collect();
    paths.sort();
    // move clang to the end of the list if it exists
    if let Some(index) = paths.iter().position(|path| path.contains("clang")) {
        let clang_path = paths.remove(index);
        paths.push(clang_path);
    }
    log::debug!("Export paths: {:?}", paths);
    paths
}

/// Gathers unique export paths for a given set of installed tools.
///
/// This function iterates through a map of installed tools, constructs their installation
/// paths, and then identifies specific export paths based on the `ToolsFile` definition.
/// For paths containing "bin", it dynamically finds all "bin" directories within the
/// tool's installation. Otherwise, it constructs the path as specified.
/// It logs warnings for non-existent paths and errors for access issues.
/// All unique, existing paths are collected, sorted, and returned.
///
/// # Arguments
///
/// * `tools_file` - A `ToolsFile` struct containing the definitions of tools,
///   including their `export_paths`.
/// * `installed_tools` - A `HashMap` where keys are tool names (`String`) and values
///   are tuples containing the tool's installed version (`String`)
///   and its `Download` information.
/// * `tools_install_path` - A string slice representing the base directory where tools are installed.
///
/// # Returns
///
/// A `Vec<String>` containing a sorted list of unique, absolute export paths for the
/// specified installed tools that actually exist on the filesystem.
///
pub fn get_tools_export_paths_from_list(
    tools_file: ToolsFile,
    installed_tools: HashMap<String, (String, Download)>,
    tools_install_path: &str,
) -> Vec<String> {
    let mut paths_set: HashSet<String> = HashSet::new();
    for (tool_name, (version, _download)) in installed_tools {
        let mut p = PathBuf::from(tools_install_path);
        p.push(tool_name.clone());
        p.push(version);
        if let Some(tool) = tools_file.tools.iter().find(|tool| tool.name == tool_name) {
            for path in &tool.export_paths {
                collect_export_path(&p, path, &mut paths_set);
            }
        }
    }
    let mut paths: Vec<String> = paths_set.into_iter().collect();
    paths.sort();
    log::debug!("Export paths from list: {:?}", paths);
    paths
}

/// Adds the existing directories for one `export_paths` entry of an installed tool.
/// Entries containing a `bin` level are resolved by searching `tool_dir` for every
/// `bin` directory; other entries are joined onto `tool_dir` as-is.
fn collect_export_path(tool_dir: &Path, export_path: &[String], paths_set: &mut HashSet<String>) {
    if export_path.iter().any(|level| level == "bin") {
        for bin_dir in find_bin_directories(tool_dir) {
            insert_if_exists(
                paths_set,
                Path::new(&bin_dir),
                "Bin directory does not exist",
                "Error checking bin directory",
            );
        }
    } else {
        let mut full_path = tool_dir.to_path_buf();
        for level in export_path {
            full_path.push(level);
        }
        insert_if_exists(
            paths_set,
            &full_path,
            "Export path does not exist",
            "Error checking export path",
        );
    }
}

fn insert_if_exists(
    paths_set: &mut HashSet<String>,
    path: &Path,
    missing_msg: &str,
    error_msg: &str,
) {
    match path.try_exists() {
        Ok(true) => {
            paths_set.insert(path.to_str().unwrap().to_string());
        }
        Ok(false) => {
            log::warn!("{}: {}", missing_msg, path.to_str().unwrap());
        }
        Err(e) => {
            log::error!("{}: {}", error_msg, e);
        }
    }
}

/// Returns a `Vec<(String, String)>` of export variables for the installed tools.
///
/// This function iterates through a map of installed tools, finds their export variables
/// from the `ToolsFile` definition, and replaces any `${TOOL_PATH}` placeholder in the
/// values with the actual `tools_install_path`. All collected key-value pairs are returned
/// as a vector of tuples.
///
/// # Arguments
///
/// * `tools_file` - A `ToolsFile` struct containing the definitions of tools,
///   including their `export_vars`.
/// * `installed_tools` - A `HashMap` where keys are tool names (`String`) and values
///   are tuples containing the tool's installed version (`String`)
///   and its `Download` information.
/// * `tools_install_path` - A string slice representing the base directory where tools are installed.
///
/// # Returns
///
/// A `Vec<(String, String)>` containing the export variable names and their values
/// for the specified installed tools.
///
pub fn get_tools_export_vars_from_list(
    _tools_file: ToolsFile,
    installed_tools: HashMap<String, (String, Download)>,
    tools_install_path: &str,
) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = Vec::new();

    for (tool_name, (_version, _download)) in installed_tools {
        if let Some(tool) = _tools_file.tools.iter().find(|t| t.name == tool_name) {
            for (var_name, var_value) in &tool.export_vars {
                let single_tool_install_path = PathBuf::from(tools_install_path)
                    .join(tool.name.clone())
                    .join(tool.versions.first().unwrap().name.clone());
                let processed_value =
                    var_value.replace("${TOOL_PATH}", &single_tool_install_path.to_string_lossy());
                vars.push((var_name.clone(), processed_value));
            }
        }
    }

    log::debug!("Export vars from list: {:?}", vars);
    vars
}

/// Recursively searches for directories named "bin" within the given path.
///
/// # Parameters
///
/// * `path`: A reference to a `Path` representing the starting directory for the search.
///
/// # Return
///
/// * A vector of `PathBuf` instances representing the directories found.
///
pub fn find_bin_directories(path: &Path) -> Vec<String> {
    find_directories_by_name(path, "bin")
}

/// Sets up (downloads and installs) a list of selected tools based on their definitions.
///
/// This asynchronous function orchestrates the entire setup process for a given set of tools.
/// It first determines which tools need to be downloaded, then iteratively processes each tool:
///
/// 1. **Verifies Installation Status**: Checks if the tool is already installed correctly,
///    if a different version is present, or if it's missing. If already correct, it skips
///    the download and installation.
/// 2. **Handles Existing Downloads**: If a tool's archive already exists in the download
///    directory and passes checksum verification, it skips the download phase.
/// 3. **Downloads Tools**: Downloads the tool's archive to the specified download directory,
///    providing progress updates via the `progress_callback`.
/// 4. **Verifies Checksum**: After download, it verifies the integrity of the downloaded file
///    using its SHA256 checksum. Corrupted files are removed.
/// 5. **Extracts Archives**: Decompresses the downloaded archive into the appropriate
///    installation directory, structured by tool name and version.
///
/// Progress updates throughout these stages are communicated via the `progress_callback`.
///
/// # Arguments
///
/// * `tools` - A reference to a `ToolsFile` struct, containing the definitions for all known tools.
/// * `selected_targets` - A `Vec` of strings, representing the names of the tools to be set up.
/// * `download_dir` - A `PathBuf` indicating the directory where tool archives should be downloaded.
/// * `install_dir` - A `PathBuf` indicating the base directory where tools should be installed.
/// * `mirror` - An `Option<&str>` specifying an optional mirror URL to use for downloads.
///   If `Some`, download URLs will be adjusted to use this mirror.
/// * `progress_callback` - A closure that implements `Fn(DownloadProgress) + Clone + Send + 'static`.
///   This callback is invoked to report the progress and status of downloads
///   and installations.
///
/// # Returns
///
/// A `Result` which is:
/// * `Ok(HashMap<String, (String, Download)>)` - A `HashMap` containing the names of the tools
///   that were processed, mapped to a tuple of their preferred version (String) and the
///   `Download` information used for that version.
/// * `Err(anyhow::Error)` - An error if any critical step during the setup process fails
///   (e.g., download failure, checksum mismatch, extraction error).
///
pub async fn setup_tools(
    tools: &ToolsFile,
    selected_targets: Vec<String>,
    download_dir: &Path,
    install_dir: &Path,
    mirror: Option<&str>,
    progress_callback: impl Fn(DownloadProgress) + Clone + Send + 'static,
) -> anyhow::Result<HashMap<String, (String, Download)>> {
    let download_links = get_list_of_tools_to_download(tools.clone(), selected_targets, mirror);
    // Download each tool
    for (tool_name, (version, download_link)) in download_links.iter() {
        let file_path = Path::new(&download_link.url);
        let filename = file_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid filename in URL"))?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in filename"))?;

        let full_file_path = download_dir.join(filename);
        let this_install_dir = install_dir.join(tool_name).join(version);

        let status = verify_tool_installation(tool_name, tools, install_dir, version);
        if is_tool_already_installed(tool_name, status, &download_link.url, &progress_callback)? {
            continue; // Skip if already installed correctly
        }

        // Notify start of processing this tool
        progress_callback(DownloadProgress::Start(download_link.url.clone()));

        // Check if file already exists and has correct checksum
        if let Ok(true) =
            verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap())
        {
            install_verified_archive(
                tools,
                tool_name,
                &download_link.url,
                &full_file_path,
                &this_install_dir,
                &progress_callback,
            )?;
            continue;
        }

        let tx =
            spawn_download_progress_forwarder(progress_callback.clone(), download_link.url.clone());

        // Download the file
        match download_file(&download_link.url, download_dir.to_str().unwrap(), Some(tx)).await {
            Ok(_) => {
                // Verify downloaded file
                if verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap())? {
                    install_verified_archive(
                        tools,
                        tool_name,
                        &download_link.url,
                        &full_file_path,
                        &this_install_dir,
                        &progress_callback,
                    )?;
                } else {
                    // Remove corrupted file
                    std::fs::remove_file(&full_file_path)?;
                    return Err(anyhow::anyhow!("Downloaded file is corrupted"));
                }
            }
            Err(e) => {
                progress_callback(DownloadProgress::Error(e.to_string()));
                return Err(anyhow::anyhow!("Download failed: {}", e));
            }
        }
    }

    Ok(download_links)
}

/// Logs the verification outcome for a tool and reports whether it can be skipped.
/// An already-correct tool is reported as verified and complete through `progress_callback`.
fn is_tool_already_installed(
    tool_name: &str,
    status: Result<ToolStatus>,
    url: &str,
    progress_callback: &impl Fn(DownloadProgress),
) -> anyhow::Result<bool> {
    match status {
        Ok(ToolStatus::Correct { version }) => {
            progress_callback(DownloadProgress::Verified(url.to_string()));
            progress_callback(DownloadProgress::Complete);
            log::info!(
                "Tool '{}' is already installed with the correct version: {}",
                tool_name,
                version
            );
            Ok(true)
        }
        Ok(ToolStatus::DifferentVersion {
            installed,
            expected,
        }) => {
            // todo: install to different folder
            log::warn!(
                "Tool '{}' is installed with version '{}', but expected '{}'. Reinstalling...",
                tool_name,
                installed,
                expected
            );
            Ok(false)
        }
        Ok(ToolStatus::Missing) => {
            log::info!("Tool '{}' is not installed. Downloading...", tool_name);
            Ok(false)
        }
        Err(e) => {
            log::error!("Error verifying tool '{}': {}", tool_name, e);
            Err(anyhow::anyhow!(
                "Error verifying tool '{}': {}",
                tool_name,
                e
            ))
        }
    }
}

/// Extracts an archive whose checksum has already been verified and runs the
/// post-extraction steps, reporting progress along the way.
fn install_verified_archive(
    tools: &ToolsFile,
    tool_name: &str,
    url: &str,
    archive_path: &Path,
    install_dir: &Path,
    progress_callback: &impl Fn(DownloadProgress),
) -> anyhow::Result<()> {
    progress_callback(DownloadProgress::Verified(url.to_string()));
    decompress_archive(
        archive_path.to_str().unwrap(),
        install_dir.to_str().unwrap(),
    )?;

    if let Some(tool) = tools.tools.iter().find(|t| t.name == tool_name) {
        post_extract_operations(tool_name, tool, install_dir)?;
    }

    progress_callback(DownloadProgress::Extracted(
        url.to_string(),
        install_dir.to_str().unwrap().to_string(),
    ));
    progress_callback(DownloadProgress::Complete);
    Ok(())
}

/// Spawns a thread that forwards download progress to `callback`, translating the
/// downloader's `Complete` into `Downloaded(url)`.
fn spawn_download_progress_forwarder(
    callback: impl Fn(DownloadProgress) + Send + 'static,
    url: String,
) -> std::sync::mpsc::Sender<DownloadProgress> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(progress) = rx.recv() {
            match progress {
                DownloadProgress::Progress(current, total) => {
                    callback(DownloadProgress::Progress(current, total));
                }
                DownloadProgress::Complete => {
                    callback(DownloadProgress::Downloaded(url.clone()));
                }
                DownloadProgress::Error(e) => {
                    callback(DownloadProgress::Error(e));
                }
                _ => {}
            }
        }
    });
    tx
}

/// Performs post-extraction operations: stripping container directories and setting permissions.
///
/// # Arguments
///
/// * `tool_name` - The name of the tool
/// * `tool` - The Tool struct containing configuration
/// * `install_dir` - The directory where the tool was extracted
///
/// # Returns
///
/// * `Result<()>` - Ok if successful, Err otherwise
fn post_extract_operations(tool_name: &str, tool: &Tool, install_dir: &Path) -> Result<()> {
    // Strip container directories if specified
    if let Some(levels) = tool.strip_container_dirs {
        if levels > 0 {
            log::debug!(
                "Stripping {} container directory levels for tool '{}'",
                levels,
                tool_name
            );
            if let Err(e) = do_strip_container_dirs(install_dir, levels) {
                log::warn!(
                    "Failed to strip container directories for '{}': {}. Continuing anyway.",
                    tool_name,
                    e
                );
                // Don't return error - if stripping fails, we still have a usable installation
            }
        }
    }

    // Fix for ninja not having `x` permission in zip archive
    if tool_name.contains("ninja") {
        match add_x_permission_to_tool(install_dir, "ninja") {
            Ok(_) => {
                log::info!(
                    "Set executable permissions for ninja in {}",
                    install_dir.display()
                );
            }
            Err(e) => {
                log::error!("Failed to set executable permissions for ninja: {}. Please set the `+x` permission manually.", e);
            }
        }
    }

    Ok(())
}

/// Adds execute (x) permission to the specified tool within the installation directory.
///
/// This function searches for a tool by its name within the given `install_dir`
/// and, if found, sets its file permissions to `0o755` (read, write, and execute for owner;
/// read and execute for group and others) on Unix-like systems.
///
/// # Arguments
///
/// * `install_dir` - A reference to a `PathBuf` indicating the directory where the tool is installed.
/// * `executable_name` - A string slice representing the name of the executable file.
///
/// # Returns
///
/// A `Result` indicating success (`Ok(())`) or an error (`Err`) if the permissions
/// could not be set for any reason.
///
/// # Examples
///
/// ```no_run
/// use std::path::PathBuf;
/// use anyhow::Result;
///
/// // Assuming `add_x_permission_to_tool` is in scope
/// fn add_x_permission_to_tool(install_dir: &PathBuf, executable_name:&str) -> Result<()> {
///     // ... function implementation ...
/// #    Ok(())
/// }
///
/// let install_dir = PathBuf::from("/usr/local/bin");
/// let executable_name = "my_tool";
///
/// if let Err(e) = add_x_permission_to_tool(&install_dir, executable_name) {
///     eprintln!("Failed to add execute permission: {}", e);
/// }
/// ```
fn add_x_permission_to_tool(install_dir: &Path, executable_name: &str) -> Result<()> {
    let tool_path = find_by_name_and_extension(install_dir, executable_name, "");
    let direct = install_dir.join(executable_name);
    #[cfg(unix)]
    {
        if direct.exists() {
            log::debug!("Found tool at: {}", direct.display());
            use std::{fs::set_permissions, os::unix::fs::PermissionsExt};
            // Set the file as executable (mode 0o755)
            let permissions = PermissionsExt::from_mode(0o755);
            set_permissions(Path::new(&direct), permissions).map_err(|e| anyhow!(e))?;
        }
    }
    for single_tool in tool_path {
        log::debug!("Setting execute permission for tool: {}", single_tool);
        #[cfg(unix)]
        {
            use std::{fs::set_permissions, os::unix::fs::PermissionsExt};
            // Set the file as executable (mode 0o755)
            let permissions = PermissionsExt::from_mode(0o755);
            set_permissions(Path::new(&single_tool), permissions).map_err(|e| anyhow!(e))?;
        }
    }
    Ok(())
}

/// Verify if a tool is installed with the correct version
pub fn verify_tool_installation(
    tool_name: &str,
    tools_file: &ToolsFile,
    install_dir: &Path,
    _version: &str,
) -> Result<ToolStatus> {
    // Find the tool in the tools file
    let tool = tools_file
        .tools
        .iter()
        .find(|t| t.name == tool_name)
        .ok_or(format!("Tool '{}' not found in tools file", tool_name))
        .map_err(|e| anyhow!(e))?;

    // Get the expected version (recommended or first available)
    let expected_version = tool
        .versions
        .iter()
        .find(|v| v.status == "recommended")
        .or_else(|| tool.versions.first())
        .ok_or(format!("No version found for tool '{}'", tool_name))
        .map_err(|e| anyhow!(e))?;

    // adding to PATH the directory where the tool is(or will be) installed
    debug!(
        "Checking tool: {}, expected version: {} and install dir: {}",
        tool_name,
        expected_version.name,
        install_dir.display()
    );
    let expected_dir = expected_tool_dir(
        install_dir,
        tool_name,
        &expected_version.name,
        &tool.export_paths,
    );

    if tool.version_cmd.is_empty() || tool.version_cmd[0].is_empty() {
        return match expected_dir.try_exists() {
            Ok(true) => Ok(ToolStatus::Correct {
                version: expected_version.name.clone(),
            }),
            _ => Ok(ToolStatus::Missing),
        };
    }

    let tmp_path = prepend_to_path_var(
        expected_dir.to_str().unwrap(),
        &std::env::var("PATH").unwrap_or_default(),
        std::env::consts::OS,
    );

    let Some(output) = run_version_command(&tool.version_cmd, &expected_dir, &tmp_path) else {
        return Ok(ToolStatus::Missing);
    };

    // Convert output to string
    let output_str = String::from_utf8_lossy(&output.stdout);
    let error_str = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", output_str, error_str);

    // Parse version using regex
    let regex = Regex::new(&tool.version_regex)
        .map_err(|e| format!("Invalid regex '{}': {}", tool.version_regex, e))
        .map_err(|e| anyhow!(e))?;

    let installed_version = match regex
        .captures(&combined_output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
    {
        Some(version) => version,
        None => {
            return Ok(ToolStatus::DifferentVersion {
                installed: format!("Unknown version from output: {}", output_str.trim()),
                expected: expected_version.name.clone(),
            });
        }
    };

    Ok(classify_installed_version(
        installed_version,
        &expected_version.name,
    ))
}

/// Builds the directory a tool's binaries are expected in:
/// `<install_dir>/<tool_name>/<version>/<every export path level, in order>`.
fn expected_tool_dir(
    install_dir: &Path,
    tool_name: &str,
    version: &str,
    export_paths: &[Vec<String>],
) -> PathBuf {
    let mut expected_dir = install_dir.join(tool_name).join(version);
    for level in export_paths.iter().flatten() {
        expected_dir.push(level);
    }
    expected_dir
}

fn prepend_to_path_var(dir: &str, path_var: &str, os: &str) -> String {
    match os {
        "windows" => format!("{};{}", dir, path_var),
        _ => format!("{}:{}", dir, path_var),
    }
}

/// Runs the tool's version command, trying the exact binary in `expected_dir`
/// first (plus `.exe` on Windows) and falling back to a `PATH` lookup.
/// Returns `None` when the command could not be executed.
fn run_version_command(
    version_cmd: &[String],
    expected_dir: &Path,
    path_var: &str,
) -> Option<std::process::Output> {
    let tool_name = &version_cmd[0];
    let args = match version_cmd.get(1) {
        Some(arg) => vec![arg.as_str()],
        None => vec![],
    };
    let env = vec![("PATH", path_var)];

    let exact_tool_path = expected_dir.join(tool_name);
    if exact_tool_path.try_exists().unwrap_or(false) {
        log::debug!("Found exact tool at: {}", exact_tool_path.display());
        return execute_command_with_env(&exact_tool_path.to_string_lossy(), &args, env).ok();
    }
    if std::env::consts::OS == "windows" {
        let exact_tool_path_exe = expected_dir.join(format!("{}.exe", tool_name));
        if exact_tool_path_exe.try_exists().unwrap_or(false) {
            log::debug!("Found exact tool at: {}", exact_tool_path_exe.display());
            return execute_command_with_env(&exact_tool_path_exe.to_string_lossy(), &args, env)
                .ok();
        }
    }
    log::debug!(
        "Exact tool not found at path: {}, falling back to version command",
        exact_tool_path.display()
    );
    execute_command_with_env(tool_name, &args, env).ok()
}

/// Collapses runs of underscores so cosmetic build-string differences like
/// "esp-21.1.3_20260408" vs "esp-21.1.3__20260408" (seen on some Windows
/// builds of clangd) still compare equal.
fn collapse_underscores(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for ch in s.chars() {
        if ch == '_' {
            if !prev_underscore {
                out.push('_');
            }
            prev_underscore = true;
        } else {
            out.push(ch);
            prev_underscore = false;
        }
    }
    out
}

fn classify_installed_version(installed_version: &str, expected: &str) -> ToolStatus {
    if installed_version == expected
        || versions_match(installed_version, expected)
        || collapse_underscores(installed_version) == collapse_underscores(expected)
    {
        ToolStatus::Correct {
            version: installed_version.to_string(),
        }
    } else {
        ToolStatus::DifferentVersion {
            installed: installed_version.to_string(),
            expected: expected.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    use std::path::Path;

    use super::find_bin_directories;

    #[test]
    fn test_find_bin_directories_non_existing_path() {
        let non_existing_path = Path::new("/path/that/does/not/exist");
        let result = find_bin_directories(non_existing_path);

        assert_eq!(
            result.len(),
            0,
            "Expected an empty vector when the path does not exist"
        );
    }
    /// Creates `<tmp>/<search_root>/bin` and asserts it is the only bin dir found from `search_root`.
    fn assert_finds_only_bin_under(search_root: &str) {
        let tmp = tempfile::TempDir::new().unwrap();
        let test_dir = tmp.path().join(search_root);
        let bin_dir = test_dir.join("bin").to_string_lossy().to_string();
        std::fs::create_dir_all(&bin_dir).unwrap();

        let result = find_bin_directories(&test_dir);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], bin_dir);
    }

    #[test]
    fn test_find_bin_directories_root_level() {
        assert_finds_only_bin_under("test_directory");
    }

    #[test]
    fn test_find_bin_directories_deeply_nested() {
        assert_finds_only_bin_under("test_files/deeply_nested_directory/something/");
    }

    #[test]
    fn test_change_links_download_mirror_multiple_tools() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            (
                "1.0.0".to_string(),
                Download {
                    sha256: "abc123".to_string(),
                    size: 1024,
                    url: "https://github.com/example/tool1.tar.gz".to_string(),
                    rename_dist: None,
                },
            ),
        );
        tools.insert(
            "tool2".to_string(),
            (
                "1.0.0".to_string(),
                Download {
                    sha256: "def456".to_string(),
                    size: 2048,
                    url: "https://github.com/example/tool2.tar.gz".to_string(),
                    rename_dist: None,
                },
            ),
        );

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(
            updated_tools.get("tool1").unwrap().1.url,
            "https://dl.espressif.com/github_assets/example/tool1.tar.gz"
        );
        assert_eq!(
            updated_tools.get("tool2").unwrap().1.url,
            "https://dl.espressif.com/github_assets/example/tool2.tar.gz"
        );
    }

    #[test]
    fn test_change_links_download_mirror_no_mirror() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            (
                "1.0.0".to_string(),
                Download {
                    sha256: "abc123".to_string(),
                    size: 1024,
                    url: "https://github.com/example/tool1.tar.gz".to_string(),
                    rename_dist: None,
                },
            ),
        );

        let mirror = None;
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(
            updated_tools.get("tool1").unwrap().1.url,
            "https://github.com/example/tool1.tar.gz"
        );
    }

    #[test]
    fn test_change_links_download_mirror_empty_tools() {
        let tools = HashMap::new();

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(updated_tools.len(), 0);
    }

    #[test]
    fn test_change_links_download_mirror_no_github_url() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            (
                "1.0.0".to_string(),
                Download {
                    sha256: "abc123".to_string(),
                    size: 1024,
                    url: "https://example.com/tool1.tar.gz".to_string(),
                    rename_dist: None,
                },
            ),
        );

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(
            updated_tools.get("tool1").unwrap().1.url,
            "https://example.com/tool1.tar.gz"
        );
    }

    #[test]
    fn test_change_links_download_mirror_empty_url() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            (
                "1.0.0".to_string(),
                Download {
                    sha256: "abc123".to_string(),
                    size: 1024,
                    url: "".to_string(),
                    rename_dist: None,
                },
            ),
        );

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(updated_tools.get("tool1").unwrap().1.url, "");
    }

    #[test]
    fn test_platform_detection() {
        let result = get_platform_identification();
        assert!(result.is_ok());

        // The actual result will depend on the platform the test runs on
        println!("Detected platform: {:?}", result);
    }

    #[test]
    fn test_apply_platform_overrides() {
        let tool = Tool {
            description: "Test tool".to_string(),
            export_paths: vec![vec!["bin".to_string()]],
            export_vars: HashMap::new(),
            info_url: "https://example.com".to_string(),
            install: "on_request".to_string(),
            license: Some("MIT".to_string()),
            name: "test-tool".to_string(),
            platform_overrides: Some(vec![
                PlatformOverride {
                    install: Some("always".to_string()),
                    platforms: vec!["win32".to_string(), "win64".to_string()],
                    export_paths: Some(vec![vec!["windows".to_string(), "bin".to_string()]]),
                },
                PlatformOverride {
                    install: Some("never".to_string()),
                    platforms: vec!["linux-i686".to_string()],
                    export_paths: None,
                },
            ]),
            supported_targets: Some(vec!["all".to_string()]),
            strip_container_dirs: None,
            version_cmd: vec!["test".to_string(), "--version".to_string()],
            version_regex: "version ([0-9.]+)".to_string(),
            version_regex_replace: None,
            versions: vec![],
        };

        let tools_file = ToolsFile {
            tools: vec![tool.clone()],
            version: 3,
        };

        // Test applying win64 override
        let result = apply_platform_overrides(tools_file.clone(), "win64");
        assert_eq!(result.tools[0].install, "always");
        assert_eq!(
            result.tools[0].export_paths,
            vec![vec!["windows".to_string(), "bin".to_string()]]
        );
        assert!(result.tools[0].platform_overrides.is_none());

        // Test applying linux-i686 override
        let result = apply_platform_overrides(tools_file.clone(), "linux-i686");
        assert_eq!(result.tools[0].install, "never");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]); // unchanged
        assert!(result.tools[0].platform_overrides.is_none());

        // Test applying non-matching platform
        let result = apply_platform_overrides(tools_file.clone(), "macos");
        assert_eq!(result.tools[0].install, "on_request"); // unchanged
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]); // unchanged
        assert!(result.tools[0].platform_overrides.is_none());

        // Test idempotency - applying again should not change anything
        let result_first = apply_platform_overrides(tools_file.clone(), "win64");
        let result_second = apply_platform_overrides(result_first.clone(), "win64");
        assert_eq!(
            result_first.tools[0].install,
            result_second.tools[0].install
        );
        assert_eq!(
            result_first.tools[0].export_paths,
            result_second.tools[0].export_paths
        );
    }
    fn override_test_tool(
        name: &str,
        install: &str,
        platform_overrides: Vec<PlatformOverride>,
    ) -> ToolsFile {
        let tool = Tool {
            description: "Test tool".to_string(),
            export_paths: vec![vec!["bin".to_string()]],
            export_vars: HashMap::new(),
            info_url: "https://example.com".to_string(),
            install: install.to_string(),
            license: None,
            name: name.to_string(),
            platform_overrides: Some(platform_overrides),
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: vec![],
            version_regex: "".to_string(),
            version_regex_replace: None,
            versions: vec![],
        };
        ToolsFile {
            tools: vec![tool],
            version: 1,
        }
    }

    fn win64_always_override() -> PlatformOverride {
        PlatformOverride {
            install: Some("always".to_string()),
            platforms: vec!["win64".to_string()],
            export_paths: None,
        }
    }

    #[test]
    fn test_platform_override_install_only() {
        let tools_file =
            override_test_tool("test-tool", "on_request", vec![win64_always_override()]);

        let result = apply_platform_overrides(tools_file, "win64");

        assert_eq!(result.tools[0].install, "always");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]);
        assert!(result.tools[0].platform_overrides.is_none());
    }

    #[test]
    fn test_platform_override_export_paths_only() {
        let tools_file = override_test_tool(
            "cmake",
            "on_request",
            vec![PlatformOverride {
                install: None,
                platforms: vec!["macos".to_string(), "macos-arm64".to_string()],
                export_paths: Some(vec![vec![
                    "CMake.app".to_string(),
                    "Contents".to_string(),
                    "bin".to_string(),
                ]]),
            }],
        );

        let result = apply_platform_overrides(tools_file, "macos-arm64");

        assert_eq!(result.tools[0].install, "on_request"); // Unchanged
        assert_eq!(
            result.tools[0].export_paths,
            vec![vec![
                "CMake.app".to_string(),
                "Contents".to_string(),
                "bin".to_string()
            ]]
        );
    }

    #[test]
    fn test_no_matching_platform() {
        let tools_file =
            override_test_tool("test-tool", "on_request", vec![win64_always_override()]);

        let result = apply_platform_overrides(tools_file, "linux-amd64");

        // Nothing should change except platform_overrides being removed
        assert_eq!(result.tools[0].install, "on_request");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]);
        assert!(result.tools[0].platform_overrides.is_none());
    }

    #[test]
    fn test_multiple_overrides_first_match_wins() {
        let tools_file = override_test_tool(
            "test-tool",
            "never",
            vec![
                PlatformOverride {
                    install: Some("always".to_string()),
                    platforms: vec!["win64".to_string(), "linux-amd64".to_string()],
                    export_paths: None,
                },
                PlatformOverride {
                    install: Some("on_request".to_string()),
                    platforms: vec!["linux-amd64".to_string()],
                    export_paths: Some(vec![vec!["other".to_string()]]),
                },
            ],
        );

        let result = apply_platform_overrides(tools_file, "linux-amd64");

        // First override should win
        assert_eq!(result.tools[0].install, "always");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]);
        // Unchanged from original
    }
    #[test]
    fn test_platform_identification() {
        let result = get_platform_identification();
        assert!(result.is_ok());

        let platform = result.unwrap();
        println!("Platform: {}", platform);

        // Should be one of the known platforms
        let valid_platforms = vec![
            "win32",
            "win64",
            "macos",
            "macos-arm64",
            "linux-i686",
            "linux-amd64",
            "linux-arm64",
            "linux-armel",
            "linux-armhf",
        ];
        assert!(valid_platforms.contains(&platform.as_str()));
    }

    #[test]
    fn test_platform_definition() {
        let platform = get_platform_definition();
        println!("Platform definition: {}", platform);

        // Should contain a hyphen separating os and arch
        assert!(platform.contains('-'));

        let parts: Vec<&str> = platform.split('-').collect();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_os_name() {
        let os = get_os_name();
        println!("OS: {}", os);

        // Should be one of the known OS names
        let valid_os = ["windows", "macos", "linux", "freebsd"];
        assert!(valid_os.contains(&os.as_str()));
    }

    #[test]
    fn test_arch_name() {
        let arch = get_arch_name();
        println!("Arch: {}", arch);

        // Should be one of the known architectures
        let valid_arch = ["x86", "x86_64", "arm", "aarch64"];
        assert!(valid_arch.contains(&arch.as_str()));
    }

    #[test]
    fn test_expected_tool_dir_appends_all_export_levels() {
        let export_paths = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["bin".to_string()],
        ];
        let dir = expected_tool_dir(Path::new("/tools"), "cmake", "3.30", &export_paths);
        assert_eq!(
            dir,
            Path::new("/tools")
                .join("cmake")
                .join("3.30")
                .join("a")
                .join("b")
                .join("bin")
        );
    }

    #[test]
    fn test_expected_tool_dir_without_export_paths() {
        let dir = expected_tool_dir(Path::new("/tools"), "ninja", "1.12", &[]);
        assert_eq!(dir, Path::new("/tools").join("ninja").join("1.12"));
    }

    #[test]
    fn test_prepend_to_path_var_uses_os_separator() {
        assert_eq!(
            prepend_to_path_var("C:\\t", "C:\\x", "windows"),
            "C:\\t;C:\\x"
        );
        assert_eq!(prepend_to_path_var("/t", "/x", "linux"), "/t:/x");
        assert_eq!(prepend_to_path_var("/t", "/x", "macos"), "/t:/x");
        assert_eq!(prepend_to_path_var("/t", "", "macos"), "/t:");
    }

    #[test]
    fn test_collapse_underscores() {
        assert_eq!(
            collapse_underscores("esp-21.1.3__20260408"),
            "esp-21.1.3_20260408"
        );
        assert_eq!(collapse_underscores("a___b_c"), "a_b_c");
        assert_eq!(collapse_underscores("no-underscores"), "no-underscores");
        assert_eq!(collapse_underscores(""), "");
    }

    #[test]
    fn test_classify_installed_version_exact_match() {
        assert_eq!(
            classify_installed_version("esp-14.2.0_20241119", "esp-14.2.0_20241119"),
            ToolStatus::Correct {
                version: "esp-14.2.0_20241119".to_string()
            }
        );
    }

    #[test]
    fn test_classify_installed_version_major_minor_match() {
        assert_eq!(
            classify_installed_version("3.30.9", "3.30.2"),
            ToolStatus::Correct {
                version: "3.30.9".to_string()
            }
        );
    }

    #[test]
    fn test_classify_installed_version_underscore_runs_match() {
        assert_eq!(
            classify_installed_version("esp-21.1.3__20260408", "esp-21.1.3_20260408"),
            ToolStatus::Correct {
                version: "esp-21.1.3__20260408".to_string()
            }
        );
    }

    #[test]
    fn test_classify_installed_version_mismatch() {
        assert_eq!(
            classify_installed_version("3.29.0", "3.30.2"),
            ToolStatus::DifferentVersion {
                installed: "3.29.0".to_string(),
                expected: "3.30.2".to_string(),
            }
        );
    }

    #[test]
    fn test_insert_if_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut set = HashSet::new();
        insert_if_exists(&mut set, tmp.path(), "missing", "error");
        insert_if_exists(&mut set, &tmp.path().join("nope"), "missing", "error");
        assert_eq!(set.len(), 1);
        assert!(set.contains(tmp.path().to_str().unwrap()));
    }

    #[test]
    fn test_collect_export_path_plain_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let existing = tmp.path().join("share").join("tool");
        std::fs::create_dir_all(&existing).unwrap();
        let mut set = HashSet::new();

        collect_export_path(
            tmp.path(),
            &["share".to_string(), "tool".to_string()],
            &mut set,
        );
        collect_export_path(tmp.path(), &["missing".to_string()], &mut set);

        assert_eq!(set.len(), 1);
        assert!(set.contains(existing.to_str().unwrap()));
    }

    #[test]
    fn test_collect_export_path_bin_entry_finds_nested_bins() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin_a = tmp.path().join("x").join("bin");
        let bin_b = tmp.path().join("y").join("z").join("bin");
        std::fs::create_dir_all(&bin_a).unwrap();
        std::fs::create_dir_all(&bin_b).unwrap();
        let mut set = HashSet::new();

        collect_export_path(
            tmp.path(),
            &["tool".to_string(), "bin".to_string()],
            &mut set,
        );

        assert_eq!(set.len(), 2);
        assert!(set.contains(bin_a.to_str().unwrap()));
        assert!(set.contains(bin_b.to_str().unwrap()));
    }

    fn progress_label(p: &DownloadProgress) -> String {
        match p {
            DownloadProgress::Start(u) => format!("Start {}", u),
            DownloadProgress::Progress(c, t) => format!("Progress {}/{}", c, t),
            DownloadProgress::Downloaded(u) => format!("Downloaded {}", u),
            DownloadProgress::Verified(u) => format!("Verified {}", u),
            DownloadProgress::Extracted(u, d) => format!("Extracted {} {}", u, d),
            DownloadProgress::Indeterminate(n) => format!("Indeterminate {}", n),
            DownloadProgress::Complete => "Complete".to_string(),
            DownloadProgress::Error(e) => format!("Error {}", e),
        }
    }

    fn record_progress() -> (
        impl Fn(DownloadProgress),
        std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    ) {
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = events.clone();
        (
            move |p: DownloadProgress| sink.borrow_mut().push(progress_label(&p)),
            events,
        )
    }

    #[test]
    fn test_is_tool_already_installed_correct_skips_and_reports() {
        let (cb, events) = record_progress();
        let status = Ok(ToolStatus::Correct {
            version: "1.0".to_string(),
        });
        assert!(is_tool_already_installed("t", status, "https://u", &cb).unwrap());
        let events = events.borrow();
        assert_eq!(events.len(), 2);
        assert!(events[0].starts_with("Verified"));
        assert!(events[1].starts_with("Complete"));
    }

    #[test]
    fn test_is_tool_already_installed_missing_or_different_does_not_skip() {
        let (cb, events) = record_progress();
        assert!(!is_tool_already_installed("t", Ok(ToolStatus::Missing), "u", &cb).unwrap());
        let different = Ok(ToolStatus::DifferentVersion {
            installed: "1".to_string(),
            expected: "2".to_string(),
        });
        assert!(!is_tool_already_installed("t", different, "u", &cb).unwrap());
        assert!(events.borrow().is_empty());
    }

    #[test]
    fn test_is_tool_already_installed_error_is_propagated() {
        let (cb, _events) = record_progress();
        let err = is_tool_already_installed("t", Err(anyhow!("boom")), "u", &cb).unwrap_err();
        assert_eq!(err.to_string(), "Error verifying tool 't': boom");
    }

    #[test]
    fn test_spawn_download_progress_forwarder_translates_events() {
        let (out_tx, out_rx) = std::sync::mpsc::channel::<String>();
        let tx = spawn_download_progress_forwarder(
            move |p| out_tx.send(progress_label(&p)).unwrap(),
            "https://u/file.zip".to_string(),
        );
        tx.send(DownloadProgress::Progress(1, 2)).unwrap();
        tx.send(DownloadProgress::Start("ignored".to_string()))
            .unwrap();
        tx.send(DownloadProgress::Complete).unwrap();
        tx.send(DownloadProgress::Error("bad".to_string())).unwrap();
        drop(tx);

        let received: Vec<String> = out_rx.iter().collect();
        assert_eq!(received.len(), 3);
        assert!(received[0].starts_with("Progress"));
        assert!(received[1].contains("Downloaded") && received[1].contains("https://u/file.zip"));
        assert!(received[2].starts_with("Error") && received[2].contains("bad"));
    }
}
