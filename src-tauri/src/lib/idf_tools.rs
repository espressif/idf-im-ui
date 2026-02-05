use log::{debug, trace};
use regex::Regex;
use serde::{de, Deserialize};
use sysinfo::System;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};

use crate::command_executor::{execute_command, execute_command_with_env};
use crate::{decompress_archive, download_file, verify_file_checksum, DownloadProgress};
use crate::utils::{find_by_name_and_extension, find_directories_by_name, versions_match};

#[derive(Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct PlatformOverride {
    #[serde(default)]
    pub install: Option<String>,
    pub platforms: Vec<String>,
    #[serde(default)]
    pub export_paths: Option<Vec<Vec<String>>>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Version {
    pub name: String,
    pub status: String,
    #[serde(flatten)]
    pub downloads: HashMap<String, Download>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Download {
    pub sha256: String,
    pub size: u64,
    pub url: String,
    #[serde(default)]
    pub rename_dist: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
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
                        tool.name,
                        platform
                    );

                    // Apply install override if present
                    if let Some(install) = &override_entry.install {
                        debug!(
                            "  - Overriding install: '{}' -> '{}'",
                            tool.install,
                            install
                        );
                        tool.install = install.clone();
                    }

                    // Apply export_paths override if present
                    if let Some(export_paths) = &override_entry.export_paths {
                        debug!(
                            "  - Overriding export_paths ({} paths)",
                            export_paths.len()
                        );
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

        let entry = entries.next()
            .ok_or_else(|| anyhow!("at level {}, directory is empty", level))??;

        // Check if there's only one entry
        if entries.next().is_some() {
            return Err(anyhow!("at level {}, expected 1 entry, found multiple", level));
        }

        let next_path = entry.path();
        if !next_path.is_dir() {
            return Err(anyhow!("at level {}, '{}' is not a directory", level, entry.file_name().to_string_lossy()));
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
fn get_platform_definition() -> String {
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
        } else if name_lower.contains("darwin") || name_lower.contains("macos") || name_lower.contains("mac os") {
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
    if let arch = System::cpu_arch() {
        let arch_lower = arch.to_lowercase();

        // Normalize architecture names to match our existing convention
        if arch_lower.contains("x86_64") || arch_lower.contains("amd64") {
            return "x86_64".to_string();
        } else if arch_lower.contains("aarch64") || arch_lower.contains("arm64") {
            return "aarch64".to_string();
        } else if arch_lower.contains("i686") || arch_lower.contains("x86") && !arch_lower.contains("x86_64") {
            return "x86".to_string();
        } else if arch_lower.contains("arm") && !arch_lower.contains("aarch64") {
            return "arm".to_string();
        }
    }

    // Fallback to compile-time constant
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
    let preferred_version;
    if tool.versions.len() > 1 {
      log::info!("Tool {} has multiple versions, using the recommended or the first one", tool.name);
      preferred_version = tool.versions.iter().find(|v| v.status == "recommended").unwrap_or(&tool.versions[0]);
    } else if tool.versions.is_empty() {
      log::warn!("Tool {} has no versions", tool.name);
      continue;
    } else {
      preferred_version = tool.versions.first().unwrap();
    }
    if let Some(download) = preferred_version.downloads.get(platform).or_else(|| preferred_version.downloads.get("any")) {
      tool_links.insert(tool.name.clone(), (preferred_version.name.clone(), download.clone()));
    } else {
      log::warn!("Tool {} does not have a download link for platform {}", tool.name, platform);
    }
  }
  tool_links
}

/// Changes the download links of tools to use a specified mirror.
///
/// # Arguments
///
/// * `tools` - A HashMap where keys are tool names (String) and values are
///             tuples containing the tool's version (String) and its corresponding
///             Download instance.
/// * `mirror` - An optional reference to a string representing the mirror URL.
///              If `None`, the original URLs are used.
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
        .map(|(name, (version,  link))| {
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
    let mut paths:Vec<String> = paths_set.into_iter().collect();
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
///                  including their `export_paths`.
/// * `installed_tools` - A `HashMap` where keys are tool names (`String`) and values
///                       are tuples containing the tool's installed version (`String`)
///                       and its `Download` information.
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
    for (tool_name, (version, download)) in installed_tools {
      let mut p = PathBuf::from(tools_install_path);
      p.push(tool_name.clone());
      p.push(version);
      tools_file.tools.iter().find(|tool| tool.name == tool_name)
        .and_then(|tool| {
          tool.export_paths.iter().for_each(|path| {
            if path.iter().find(| level | {
                *level == "bin"
            }).is_some() {
                let bin_dirs = find_bin_directories(&p);
                for bin_dir in bin_dirs {
                    match Path::new(&bin_dir).try_exists() {
                        Ok(true) => {
                            paths_set.insert(bin_dir);
                        }
                        Ok(false) => {
                            log::warn!("Bin directory does not exist: {}", bin_dir);
                        }
                        Err(e) => {
                            log::error!("Error checking bin directory: {}", e);
                        }
                    }
                }
            } else {
              let mut export_path = p.clone();
              for level in path {
                  export_path.push(level);
              }
              match export_path.try_exists() {
                Ok(true) => {
                    paths_set.insert(export_path.to_str().unwrap().to_string());
                }
                Ok(false) => {
                    log::warn!("Export path does not exist: {}", export_path.to_str().unwrap());
                }
                Err(e) => {
                    log::error!("Error checking export path: {}", e);
                }
              }
            }
          });
          Some(())
        });
    }
    let mut paths:Vec<String> = paths_set.into_iter().collect();
    paths.sort();
    log::debug!("Export paths from list: {:?}", paths);
    paths
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
///              If `Some`, download URLs will be adjusted to use this mirror.
/// * `progress_callback` - A closure that implements `Fn(DownloadProgress) + Clone + Send + 'static`.
///                         This callback is invoked to report the progress and status of downloads
///                         and installations.
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
    download_dir: &PathBuf,
    install_dir: &PathBuf,
    mirror: Option<&str>,
    progress_callback: impl Fn(DownloadProgress) + Clone + Send + 'static,
) -> anyhow::Result<HashMap<String, (String, Download)>> {

    let download_links = get_list_of_tools_to_download(tools.clone(), selected_targets, mirror);
    // Download each tool
    for (tool_name, (version, download_link)) in download_links.iter() {
      let file_path = Path::new(&download_link.url);
      let filename = file_path.file_name()
          .ok_or_else(|| anyhow::anyhow!("Invalid filename in URL"))?
          .to_str()
          .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in filename"))?;

      let full_file_path = download_dir.join(filename);
      let this_install_dir = install_dir.join(tool_name).join(version);

      match verify_tool_installation(tool_name, tools, install_dir, version) {
        Ok(ToolStatus::Correct { version }) => {
          progress_callback(DownloadProgress::Verified(download_link.url.clone()));
          progress_callback(DownloadProgress::Complete);
          log::info!("Tool '{}' is already installed with the correct version: {}", tool_name, version);
          continue; // Skip if already installed correctly
        }
        Ok(ToolStatus::DifferentVersion { installed, expected }) => {
          // todo: install to different folder
          log::warn!("Tool '{}' is installed with version '{}', but expected '{}'. Reinstalling...", tool_name, installed, expected);
        }
        Ok(ToolStatus::Missing) => {
          log::info!("Tool '{}' is not installed. Downloading...", tool_name);
        }
        Err(e) => {
          log::error!("Error verifying tool '{}': {}", tool_name, e);
          return Err(anyhow::anyhow!("Error verifying tool '{}': {}", tool_name, e));
        }
      }




      // Notify start of processing this tool
      progress_callback(DownloadProgress::Start(download_link.url.clone()));

      // Check if file already exists and has correct checksum
      if let Ok(true) = verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap()) {
        progress_callback(DownloadProgress::Verified(download_link.url.clone()));
        decompress_archive(full_file_path.to_str().unwrap(), this_install_dir.to_str().unwrap())?;

        // Post-extraction operations
        if let Some(tool) = tools.tools.iter().find(|t| &t.name == tool_name) {
          post_extract_operations(tool_name, tool, &this_install_dir)?;
        }

        progress_callback(DownloadProgress::Extracted(download_link.url.clone(), this_install_dir.to_str().unwrap().to_string()));
        progress_callback(DownloadProgress::Complete);
        continue;
      }

      // Create a channel for progress updates
      let (tx, rx) = std::sync::mpsc::channel();

      // Spawn a thread to forward progress updates to the callback
      let callback = progress_callback.clone();
      let url = download_link.url.clone();
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

      // Download the file
      match download_file(&download_link.url, download_dir.to_str().unwrap(), Some(tx)).await {
        Ok(_) => {
          // Verify downloaded file
          if verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap())? {
            progress_callback(DownloadProgress::Verified(download_link.url.clone()));
            // Extract the archive

            decompress_archive(full_file_path.to_str().unwrap(), this_install_dir.to_str().unwrap())?;

            // Post-extraction operations
            if let Some(tool) = tools.tools.iter().find(|t| &t.name == tool_name) {
              post_extract_operations(tool_name, tool, &this_install_dir)?;
            }

            progress_callback(DownloadProgress::Extracted(download_link.url.clone(), this_install_dir.to_str().unwrap().to_string()));
            progress_callback(DownloadProgress::Complete);
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
fn post_extract_operations(tool_name: &str, tool: &Tool, install_dir: &PathBuf) -> Result<()> {
  // Strip container directories if specified
  if let Some(levels) = tool.strip_container_dirs {
    if levels > 0 {
        log::debug!("Stripping {} container directory levels for tool '{}'", levels, tool_name);
      if let Err(e) = do_strip_container_dirs(install_dir, levels) {
        log::warn!("Failed to strip container directories for '{}': {}. Continuing anyway.", tool_name, e);
        // Don't return error - if stripping fails, we still have a usable installation
      }
    }
  }

  // Fix for ninja not having `x` permission in zip archive
  if tool_name.contains("ninja") {
    match add_x_permission_to_tool(install_dir, "ninja") {
      Ok(_) => {
        log::info!("Set executable permissions for ninja in {}", install_dir.display());
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
fn add_x_permission_to_tool(install_dir: &PathBuf, executable_name:&str) -> Result<()> {
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
pub fn verify_tool_installation(tool_name: &str, tools_file: &ToolsFile, install_dir: &PathBuf, version: &str) -> Result<ToolStatus> {
    // Find the tool in the tools file
    let tool = tools_file.tools.iter()
        .find(|t| t.name == tool_name)
        .ok_or(format!("Tool '{}' not found in tools file", tool_name)).map_err(|e| anyhow!(e))?;



    // Get the expected version (recommended or first available)
    let expected_version = tool.versions.iter().find(|v| v.status == "recommended")
        .or_else(|| tool.versions.first())
        .ok_or(format!("No version found for tool '{}'", tool_name)).map_err(|e| anyhow!(e))?;

    // adding to PATH the directory where the tool is(or will be) installed
    debug!("Checking tool: {}, expected version: {} and install dir: {}", tool_name, expected_version.name, install_dir.display());
    let mut expected_dir = install_dir.join(&tool_name).join(expected_version.clone().name);
    for ex_path in &tool.export_paths {
        for level in ex_path {
            expected_dir.push(level);
        }
    }

    if tool.version_cmd.is_empty() || tool.version_cmd[0].is_empty() {
      match expected_dir.try_exists() {
          Ok(true) => {
              return Ok(ToolStatus::Correct {
                  version: expected_version.name.clone(),
              });
          }
          _ => {
            return Ok(ToolStatus::Missing);
          }
      }
    }

    let mut tmp_path = std::env::var("PATH").unwrap_or_default();
    match std::env::consts::OS {
        "windows" => {
            tmp_path = format!("{};{}", expected_dir.to_str().unwrap(), tmp_path);
        }
        _ => {
            tmp_path = format!("{}:{}", expected_dir.to_str().unwrap(), tmp_path);
        }
    }

    // Execute the version command
    // first try exactly expected binary
    let output = {
      let tool_name = &tool.version_cmd[0];
      let args = match tool.version_cmd.get(1) {
        Some(arg) => vec![arg.as_str()],
        None => vec![],
      };
      let env = vec![("PATH", tmp_path.as_str())];

      // Try 1: Exact tool path
      let exact_tool_path = expected_dir.join(tool_name);
      if exact_tool_path.try_exists().unwrap_or(false) {
          log::debug!("Found exact tool at: {}", exact_tool_path.display());
          if let Ok(output) = execute_command_with_env(&exact_tool_path.to_string_lossy(), &args, env.clone()) {
              output
          } else {
              return Ok(ToolStatus::Missing);
          }
      }
      // Try 2: Windows .exe extension
      else if std::env::consts::OS == "windows" {
          let exact_tool_path_exe = expected_dir.join(format!("{}.exe", tool_name));
          if exact_tool_path_exe.try_exists().unwrap_or(false) {
              log::debug!("Found exact tool at: {}", exact_tool_path_exe.display());
              if let Ok(output) = execute_command_with_env(&exact_tool_path_exe.to_string_lossy(), &args, env.clone()) {
                  output
              } else {
                  return Ok(ToolStatus::Missing);
              }
          } else {
              // Try 3: Fallback to PATH
              log::debug!("Exact tool not found at path: {}, falling back to version command", exact_tool_path.display());
              match execute_command_with_env(tool_name, &args, env) {
                  Ok(output) => output,
                  Err(_) => return Ok(ToolStatus::Missing),
              }
          }
      }
      // Try 3: Non-Windows fallback to PATH
      else {
          log::debug!("Exact tool not found at path: {}, falling back to version command", exact_tool_path.display());
          match execute_command_with_env(tool_name, &args, env) {
              Ok(output) => output,
              Err(_) => return Ok(ToolStatus::Missing),
          }
      }
  };

    // Convert output to string
    let output_str = String::from_utf8_lossy(&output.stdout);
    let error_str = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", output_str, error_str);

    // Parse version using regex
    let regex = Regex::new(&tool.version_regex)
        .map_err(|e| format!("Invalid regex '{}': {}", tool.version_regex, e)).map_err(|e| anyhow!(e))?;

    let installed_version = match regex.captures(&combined_output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str()) {
        Some(version) => version,
        None => {
            return Ok(ToolStatus::DifferentVersion {
                installed: format!("Unknown version from output: {}", output_str.trim()),
                expected: expected_version.name.clone(),
            });
        }
      };

    // Compare versions (major.minor only)
    if installed_version == expected_version.name || versions_match(installed_version, &expected_version.name) {
        Ok(ToolStatus::Correct {
            version: installed_version.to_string(),
        })
    } else {
        Ok(ToolStatus::DifferentVersion {
            installed: installed_version.to_string(),
            expected: expected_version.name.clone(),
        })
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
    #[test]
    fn test_find_bin_directories_root_level() {
        let test_dir = Path::new("/tmp/test_directory");
        let bin_dir = test_dir.join("bin").to_string_lossy().to_string();

        // Create the test directory and the "bin" directory
        std::fs::create_dir_all(&bin_dir).unwrap();

        let result = find_bin_directories(&test_dir);

        // Remove the test directory
        std::fs::remove_dir_all(&test_dir).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], bin_dir);
    }

    #[test]
    fn test_find_bin_directories_deeply_nested() {
        let test_dir = Path::new("/tmp/test_files/deeply_nested_directory/something/");
        let bin_dir = test_dir.join("bin").to_string_lossy().to_string();

        // Create the test directory and the "bin" directory
        std::fs::create_dir_all(&bin_dir).unwrap();

        let result = find_bin_directories(&test_dir);

        // Remove the test directory
        std::fs::remove_dir_all(&test_dir).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], bin_dir);
    }

    #[test]
    fn test_change_links_download_mirror_multiple_tools() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            ("1.0.0".to_string(),Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "https://github.com/example/tool1.tar.gz".to_string(),
                rename_dist: None,
            }),
        );
        tools.insert(
            "tool2".to_string(),
            ("1.0.0".to_string(),Download {
                sha256: "def456".to_string(),
                size: 2048,
                url: "https://github.com/example/tool2.tar.gz".to_string(),
                rename_dist: None,
            }),
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
            ("1.0.0".to_string(),Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "https://github.com/example/tool1.tar.gz".to_string(),
                rename_dist: None,
            }),
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
            ("1.0.0".to_string(),Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "https://example.com/tool1.tar.gz".to_string(),
                rename_dist: None,
            }),
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
            ("1.0.0".to_string(),Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "".to_string(),
                rename_dist: None,
            }),
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
        let mut tool = Tool {
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
        assert_eq!(result.tools[0].export_paths, vec![vec!["windows".to_string(), "bin".to_string()]]);
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
        assert_eq!(result_first.tools[0].install, result_second.tools[0].install);
        assert_eq!(result_first.tools[0].export_paths, result_second.tools[0].export_paths);
    }
    #[test]
    fn test_platform_override_install_only() {
        let mut tool = Tool {
            description: "Test tool".to_string(),
            export_paths: vec![vec!["bin".to_string()]],
            export_vars: HashMap::new(),
            info_url: "https://example.com".to_string(),
            install: "on_request".to_string(),
            license: None,
            name: "test-tool".to_string(),
            platform_overrides: Some(vec![
                PlatformOverride {
                    install: Some("always".to_string()),
                    platforms: vec!["win64".to_string()],
                    export_paths: None,
                }
            ]),
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: vec![],
            version_regex: "".to_string(),
            version_regex_replace: None,
            versions: vec![],
        };

        let tools_file = ToolsFile {
            tools: vec![tool.clone()],
            version: 1,
        };

        let result = apply_platform_overrides(tools_file, "win64");

        assert_eq!(result.tools[0].install, "always");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]);
        assert!(result.tools[0].platform_overrides.is_none());
    }

    #[test]
    fn test_platform_override_export_paths_only() {
        let mut tool = Tool {
            description: "CMake".to_string(),
            export_paths: vec![vec!["bin".to_string()]],
            export_vars: HashMap::new(),
            info_url: "https://example.com".to_string(),
            install: "on_request".to_string(),
            license: None,
            name: "cmake".to_string(),
            platform_overrides: Some(vec![
                PlatformOverride {
                    install: None,
                    platforms: vec!["macos".to_string(), "macos-arm64".to_string()],
                    export_paths: Some(vec![vec![
                        "CMake.app".to_string(),
                        "Contents".to_string(),
                        "bin".to_string()
                    ]]),
                }
            ]),
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: vec![],
            version_regex: "".to_string(),
            version_regex_replace: None,
            versions: vec![],
        };

        let tools_file = ToolsFile {
            tools: vec![tool.clone()],
            version: 1,
        };

        let result = apply_platform_overrides(tools_file, "macos-arm64");

        assert_eq!(result.tools[0].install, "on_request"); // Unchanged
        assert_eq!(
            result.tools[0].export_paths,
            vec![vec!["CMake.app".to_string(), "Contents".to_string(), "bin".to_string()]]
        );
    }

    #[test]
    fn test_no_matching_platform() {
        let mut tool = Tool {
            description: "Test tool".to_string(),
            export_paths: vec![vec!["bin".to_string()]],
            export_vars: HashMap::new(),
            info_url: "https://example.com".to_string(),
            install: "on_request".to_string(),
            license: None,
            name: "test-tool".to_string(),
            platform_overrides: Some(vec![
                PlatformOverride {
                    install: Some("always".to_string()),
                    platforms: vec!["win64".to_string()],
                    export_paths: None,
                }
            ]),
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: vec![],
            version_regex: "".to_string(),
            version_regex_replace: None,
            versions: vec![],
        };

        let tools_file = ToolsFile {
            tools: vec![tool.clone()],
            version: 1,
        };

        let result = apply_platform_overrides(tools_file, "linux-amd64");

        // Nothing should change except platform_overrides being removed
        assert_eq!(result.tools[0].install, "on_request");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]);
        assert!(result.tools[0].platform_overrides.is_none());
    }

    #[test]
    fn test_multiple_overrides_first_match_wins() {
        let mut tool = Tool {
            description: "Test tool".to_string(),
            export_paths: vec![vec!["bin".to_string()]],
            export_vars: HashMap::new(),
            info_url: "https://example.com".to_string(),
            install: "never".to_string(),
            license: None,
            name: "test-tool".to_string(),
            platform_overrides: Some(vec![
                PlatformOverride {
                    install: Some("always".to_string()),
                    platforms: vec!["win64".to_string(), "linux-amd64".to_string()],
                    export_paths: None,
                },
                PlatformOverride {
                    install: Some("on_request".to_string()),
                    platforms: vec!["linux-amd64".to_string()],
                    export_paths: Some(vec![vec!["other".to_string()]]),
                }
            ]),
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: vec![],
            version_regex: "".to_string(),
            version_regex_replace: None,
            versions: vec![],
        };

        let tools_file = ToolsFile {
            tools: vec![tool.clone()],
            version: 1,
        };

        let result = apply_platform_overrides(tools_file, "linux-amd64");

        // First override should win
        assert_eq!(result.tools[0].install, "always");
        assert_eq!(result.tools[0].export_paths, vec![vec!["bin".to_string()]]); // Unchanged from original
    }
    #[test]
    fn test_platform_identification() {
        let result = get_platform_identification();
        assert!(result.is_ok());

        let platform = result.unwrap();
        println!("Platform: {}", platform);

        // Should be one of the known platforms
        let valid_platforms = vec![
            "win32", "win64", "macos", "macos-arm64",
            "linux-i686", "linux-amd64", "linux-arm64", "linux-armel"
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
        let valid_os = vec!["windows", "macos", "linux", "freebsd"];
        assert!(valid_os.contains(&os.as_str()));
    }

    #[test]
    fn test_arch_name() {
        let arch = get_arch_name();
        println!("Arch: {}", arch);

        // Should be one of the known architectures
        let valid_arch = vec!["x86", "x86_64", "arm", "aarch64"];
        assert!(valid_arch.contains(&arch.as_str()));
    }
}
