use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use crate::python_utils::get_python_platform_definition;
use crate::system_dependencies;
use crate::utils::find_directories_by_name;

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

// TODO: maybe get this by direct calling the idf_tool.py so the hashtable is not duplicate
/// Retrieves the platform identification based on the Python platform definition.
///
/// This function maps the Python platform definition to a corresponding platform identifier.
/// It uses a predefined hashmap to perform the mapping. If the Python platform definition is not found in the hashmap,
/// an error is returned.
///
/// # Parameters
///
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
///   If `None`, the function will default to using "python3".
///
/// # Returns
///
/// * `Result<String, String>`:
///   - `Ok(String)`: If the platform identification is successfully retrieved.
///   - `Err(String)`: If the Python platform definition is not supported.
///
pub fn get_platform_identification(python: Option<&str>) -> Result<String, String> {
    let mut platform_from_name = HashMap::new();

    // Windows
    platform_from_name.insert("win32", "win32");
    platform_from_name.insert("Windows-i686", "win32");
    platform_from_name.insert("Windows-x86", "win32");
    platform_from_name.insert("i686-w64-mingw32", "win32");
    platform_from_name.insert("win64", "win64");
    platform_from_name.insert("Windows-x86_64", "win64");
    platform_from_name.insert("Windows-AMD64", "win64");
    platform_from_name.insert("x86_64-w64-mingw32", "win64");
    platform_from_name.insert("Windows-ARM64", "win64");

    // macOS
    platform_from_name.insert("macos", "macos");
    platform_from_name.insert("osx", "macos");
    platform_from_name.insert("darwin", "macos");
    platform_from_name.insert("Darwin-x86_64", "macos");
    platform_from_name.insert("x86_64-apple-darwin", "macos");
    platform_from_name.insert("macos-arm64", "macos-arm64");
    platform_from_name.insert("macos-aarch64", "macos-arm64");
    platform_from_name.insert("Darwin-arm64", "macos-arm64");
    platform_from_name.insert("Darwin-aarch64", "macos-arm64");
    platform_from_name.insert("aarch64-apple-darwin", "macos-arm64");
    platform_from_name.insert("arm64-apple-darwin", "macos-arm64");
    platform_from_name.insert("aarch64-apple-darwin", "macos-arm64");

    // Linux
    platform_from_name.insert("linux-amd64", "linux-amd64");
    platform_from_name.insert("linux64", "linux-amd64");
    platform_from_name.insert("Linux-x86_64", "linux-amd64");
    platform_from_name.insert("FreeBSD-amd64", "linux-amd64");
    platform_from_name.insert("x86_64-linux-gnu", "linux-amd64");
    platform_from_name.insert("linux-i686", "linux-i686");
    platform_from_name.insert("linux32", "linux-i686");
    platform_from_name.insert("Linux-i686", "linux-i686");
    platform_from_name.insert("FreeBSD-i386", "linux-i686");
    platform_from_name.insert("i586-linux-gnu", "linux-i686");
    platform_from_name.insert("i686-linux-gnu", "linux-i686");
    platform_from_name.insert("linux-arm64", "linux-arm64");
    platform_from_name.insert("Linux-arm64", "linux-arm64");
    platform_from_name.insert("Linux-aarch64", "linux-arm64");
    platform_from_name.insert("Linux-armv8l", "linux-arm64");
    platform_from_name.insert("aarch64", "linux-arm64");
    platform_from_name.insert("linux-armhf", "linux-armhf");
    platform_from_name.insert("arm-linux-gnueabihf", "linux-armhf");
    platform_from_name.insert("linux-armel", "linux-armel");
    platform_from_name.insert("arm-linux-gnueabi", "linux-armel");
    platform_from_name.insert("Linux-armv7l", "linux-armel");
    platform_from_name.insert("Linux-arm", "linux-armel");

    let python_platform_string = get_python_platform_definition(python).trim().to_string();

    let platform = match platform_from_name.get(&python_platform_string.as_str()) {
        Some(platform) => platform,
        None => return Err(format!("Unsupported platform: {}", python_platform_string)),
    };
    Ok(platform.to_string())
}

/// Retrieves a HashMap of tool names and their corresponding Download instances based on the given platform.
///
/// # Arguments
///
/// * `tools` - A vector of `Tool` instances.
/// * `platform` - A reference to a string representing the target platform. This can be obtained from the `get_platform_identification` function.
///
/// # Returns
///
/// * A HashMap where the keys are tool names and the values are Download instances.
///   If a tool does not have a download for the given platform, it is not included in the HashMap.
///
pub fn get_download_link_by_platform(
    tools: Vec<Tool>,
    platform: &String,
) -> HashMap<String, Download> {
    let mut tool_links = HashMap::new();
    for tool in tools {
        tool.versions.iter().for_each(|version| {
            match version.downloads.get(platform) {
                Some(download) => tool_links.insert(tool.name.clone(), download.clone()),
                None => None,
            };
        });
    }
    tool_links
}

/// Changes the download links of tools to use a specified mirror.
///
/// # Arguments
///
/// * `tools` - A HashMap containing tool names as keys and their corresponding Download instances as values.
/// * `mirror` - An optional reference to a string representing the mirror URL. If None, the original URLs are used.
///
/// # Returns
///
/// * A new HashMap with the same keys as the input `tools` but with updated Download instances.
///   The URLs of the Download instances are replaced with the mirror URL if provided.
///

pub fn change_links_donwanload_mirror(
    tools: HashMap<String, Download>,
    mirror: Option<&str>,
) -> HashMap<String, Download> {
    let new_tools: HashMap<String, Download> = tools
        .iter()
        .map(|(name, link)| {
            let new_link = match mirror {
                Some(mirror) => Download {
                    sha256: link.sha256.clone(),
                    size: link.size,
                    url: link.url.replace("https://github.com", mirror),
                    rename_dist: link.rename_dist.clone(),
                },
                None => link.clone(),
            };
            (name.to_string(), new_link)
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
) -> HashMap<String, Download> {
    let list = filter_tools_by_target(tools_file.tools, &selected_chips);
    let platform = match get_platform_identification(None) {
        Ok(platform) => platform,
        Err(err) => {
            if std::env::consts::OS == "windows" {
                // All this is for cases when on windows microsoft store creates "pseudolinks" for python
                let scp = system_dependencies::get_scoop_path();
                let usable_python = match scp {
                    Some(path) => {
                        let mut python_path = PathBuf::from(path);
                        python_path.push("python3.exe");
                        python_path.to_str().unwrap().to_string()
                    }
                    None => "python3.exe".to_string(),
                };
                match get_platform_identification(Some(&usable_python)) {
                    Ok(platform) => platform,
                    Err(err) => {
                        log::error!("Unable to identify platform: {}", err);
                        panic!("Unable to identify platform: {}", err);
                    }
                }
            } else {
                panic!("Unable to identify platform: {}", err);
            }
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
    // debug!("Creating export paths for: {:?}", list);
    println!("Creating export paths from path: {:?}", tools_install_path);
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
    let paths = paths_set.into_iter().collect();
    log::debug!("Export paths: {:?}", paths);
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
    let result = find_directories_by_name(path, "bin");

    result
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
            Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "https://github.com/example/tool1.tar.gz".to_string(),
                rename_dist: None,
            },
        );
        tools.insert(
            "tool2".to_string(),
            Download {
                sha256: "def456".to_string(),
                size: 2048,
                url: "https://github.com/example/tool2.tar.gz".to_string(),
                rename_dist: None,
            },
        );

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(
            updated_tools.get("tool1").unwrap().url,
            "https://dl.espressif.com/github_assets/example/tool1.tar.gz"
        );
        assert_eq!(
            updated_tools.get("tool2").unwrap().url,
            "https://dl.espressif.com/github_assets/example/tool2.tar.gz"
        );
    }

    #[test]
    fn test_change_links_download_mirror_no_mirror() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "https://github.com/example/tool1.tar.gz".to_string(),
                rename_dist: None,
            },
        );

        let mirror = None;
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(
            updated_tools.get("tool1").unwrap().url,
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
            Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "https://example.com/tool1.tar.gz".to_string(),
                rename_dist: None,
            },
        );

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(
            updated_tools.get("tool1").unwrap().url,
            "https://example.com/tool1.tar.gz"
        );
    }

    #[test]
    fn test_change_links_download_mirror_empty_url() {
        let mut tools = HashMap::new();
        tools.insert(
            "tool1".to_string(),
            Download {
                sha256: "abc123".to_string(),
                size: 1024,
                url: "".to_string(),
                rename_dist: None,
            },
        );

        let mirror = Some("https://dl.espressif.com/github_assets");
        let updated_tools = change_links_donwanload_mirror(tools, mirror);

        assert_eq!(updated_tools.get("tool1").unwrap().url, "");
    }
}
