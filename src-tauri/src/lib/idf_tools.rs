use log::debug;
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};

use crate::command_executor::execute_command;
use crate::{decompress_archive, download_file, verify_file_checksum, DownloadProgress};
use crate::utils::{find_directories_by_name, versions_match};

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
pub struct PlatformOverride {
    #[serde(default)]
    pub install: Option<String>,
    pub platforms: Vec<String>,
    #[serde(default)]
    pub export_paths: Option<Vec<Vec<String>>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Version {
    pub name: String,
    pub status: String,
    #[serde(flatten)]
    pub downloads: HashMap<String, Download>,
}

#[derive(Deserialize, Debug, Clone)]
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

    Ok(tools_file)
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

/// Returns a standardized platform identifier string based on the current Rust target.
///
/// This function maps the Rust-specific platform definition (e.g., "windows-x86_64")
/// obtained from `get_rust_platform_definition()` to a more common and standardized
/// identifier (e.g., "win64").
///
/// # Errors
///
/// Returns an `Err` containing a `String` if the current Rust platform is not
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

    // Rust native Windows identifiers
    platform_from_name.insert("windows-x86", "win32");
    platform_from_name.insert("windows-x86_64", "win64");
    platform_from_name.insert("windows-aarch64", "win64");

    // Rust native macOS identifiers
    platform_from_name.insert("macos-x86_64", "macos");
    platform_from_name.insert("macos-aarch64", "macos-arm64");

    // Rust native Linux identifiers
    platform_from_name.insert("linux-x86", "linux-i686");
    platform_from_name.insert("linux-x86_64", "linux-amd64");
    platform_from_name.insert("linux-aarch64", "linux-arm64");
    platform_from_name.insert("linux-arm", "linux-armel");

    // Rust native FreeBSD identifiers
    platform_from_name.insert("freebsd-x86_64", "linux-amd64");
    platform_from_name.insert("freebsd-x86", "linux-i686");

    let platform_string = get_rust_platform_definition();

    let platform = match platform_from_name.get(&platform_string.as_str()) {
        Some(platform) => platform,
        None => return Err(format!("Unsupported platform: {}", platform_string)),
    };
    Ok(platform.to_string())
}

/// Returns a string representing the current Rust platform in the format "os-arch".
///
/// This function retrieves the operating system and architecture information
/// at compile time using `std::env::consts::OS` and `std::env::consts::ARCH`
/// respectively.
///
/// # Examples
///
/// ```
/// let platform = get_rust_platform_definition();
/// // On a 64-bit Linux system, platform might be "linux-x86_64"
/// // On a macOS M1 system, platform might be "macos-aarch64"
/// println!("{}", platform);
/// ```
///
/// # Returns
///
/// A `String` in the format "os-arch" (e.g., "linux-x86_64", "macos-aarch64").
fn get_rust_platform_definition() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    format!("{}-{}", os, arch)
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
    if let Some(download) = preferred_version.downloads.get(platform) {
      tool_links.insert(tool.name.clone(), (preferred_version.name.clone(), download.clone()));
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

      match verify_tool_installation(tool_name, tools) {
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


      let file_path = Path::new(&download_link.url);
      let filename = file_path.file_name()
          .ok_or_else(|| anyhow::anyhow!("Invalid filename in URL"))?
          .to_str()
          .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in filename"))?;

      let full_file_path = download_dir.join(filename);
      let this_install_dir = install_dir.join(tool_name).join(version);

      // Notify start of processing this tool
      progress_callback(DownloadProgress::Start(download_link.url.clone()));

      // Check if file already exists and has correct checksum
      if let Ok(true) = verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap()) {
        progress_callback(DownloadProgress::Verified(download_link.url.clone()));
        decompress_archive(full_file_path.to_str().unwrap(), this_install_dir.to_str().unwrap())?;
        progress_callback(DownloadProgress::Extracted(download_link.url.clone()));
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
            progress_callback(DownloadProgress::Extracted(download_link.url.clone()));
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

/// Verify if a tool is installed with the correct version
pub fn verify_tool_installation(tool_name: &str, tools_file: &ToolsFile) -> Result<ToolStatus> {
    // Find the tool in the tools file
    let tool = tools_file.tools.iter()
        .find(|t| t.name == tool_name)
        .ok_or(format!("Tool '{}' not found in tools file", tool_name)).map_err(|e| anyhow!(e))?;

    // Skip verification if version_cmd is empty
    if tool.version_cmd.is_empty() {
        return Ok(ToolStatus::Missing);
    }

    // Get the expected version (recommended or first available)
    let expected_version = tool.versions.iter().find(|v| v.status == "recommended")
        .or_else(|| tool.versions.first())
        .ok_or(format!("No version found for tool '{}'", tool_name)).map_err(|e| anyhow!(e))?;

    // Execute the version command
    let output = match execute_command(
        &tool.version_cmd[0],
        &[&tool.version_cmd[1]],
    ) {
        Ok(output) => output,
        Err(_) => return Ok(ToolStatus::Missing),
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
    if versions_match(installed_version, &expected_version.name) {
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
    fn test_rust_platform_definition() {
        let platform_def = get_rust_platform_definition();
        println!("Platform definition: {}", platform_def);

        // Should match one of the expected formats
        assert!(platform_def.contains("-"));
    }
}
