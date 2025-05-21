use crate::{
    command_executor::execute_command,
    idf_config::{IdfConfig, IdfInstallation},
    idf_tools::read_and_parse_tools_file,
    single_version_post_install,
    version_manager::get_default_config_path,
};
use anyhow::{anyhow, Result};
use log::{debug, warn};
use rust_search::SearchBuilder;
use serde::{Deserialize, Serialize};
#[cfg(not(windows))]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::{HashMap, HashSet},
    fs::{self},
    io,
    path::{Path, PathBuf},
};
/// This function retrieves the path to the git executable.
///
/// # Purpose
///
/// The function attempts to locate the git executable by checking the system's PATH environment variable.
/// It uses the appropriate command ("where" on Windows, "which" on Unix-like systems) to find the git executable.
///
/// # Parameters
///
/// There are no parameters for this function.
///
/// # Return Value
///
/// - `Ok(String)`: If the git executable is found, the function returns a `Result` containing the path to the git executable as a `String`.
/// - `Err(String)`: If the git executable is not found or an error occurs during the process of locating the git executable, the function returns a `Result` containing an error message as a `String`.
pub fn get_git_path() -> Result<String, String> {
    let cmd = match std::env::consts::OS {
        "windows" => "where",
        _ => "which",
    };

    let output = execute_command(cmd, &["git"]).expect("failed to execute process");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Filters a vector of strings containing paths to only include directories.
///
/// # Purpose
///
/// This function takes a vector of strings representing paths and filters out any paths that are not directories.
/// It uses the `PathBuf::is_dir` method to check if each path is a directory.
///
/// # Parameters
///
/// * `paths`: A vector of strings containing paths to be filtered.
///
/// # Return Value
///
/// * `Vec<String>`: A vector of strings containing the paths of directories.
fn filter_directories(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path_str| {
            let path = PathBuf::from(path_str);
            path.is_dir()
        })
        .collect()
}
// Finds all directories in the specified path that match the given name.
// The function recursively searches subdirectories and collects matching paths in a vector.
// Returns a vector of PathBuf containing the paths of matching directories.
pub fn find_directories_by_name(path: &Path, name: &str) -> Vec<String> {
    let search: Vec<String> = SearchBuilder::default()
        .location(path)
        .search_input(name)
        // .limit(1000) // results to return
        .strict()
        // .depth(1)
        .ignore_case()
        .hidden()
        .build()
        .collect();
    filter_directories(filter_subpaths(search))
}

/// Searches for files within a specified directory that match a given name and extension.
///
/// This function constructs a search query using `SearchBuilder` to find files.
/// The search is strict, case-insensitive, includes hidden files, and looks for
/// an exact match for the file name and extension.
///
/// # Arguments
///
/// * `path` - A reference to a `Path` indicating the directory where the search should begin.
/// * `name` - A string slice representing the exact name of the file to search for (without the extension).
/// * `extension` - A string slice representing the exact extension of the file to search for (e.g., "txt", "rs").
///
/// # Returns
///
/// A `Vec<String>` containing the paths of all files found that match the criteria.
/// The paths are returned as strings. If no files are found, an empty vector is returned.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// let search_path = Path::new("./my_directory");
/// let found_files = find_by_name_and_extension(search_path, "document", "pdf");
/// for file in found_files {
///     println!("Found: {}", file);
/// }
/// ```
pub fn find_by_name_and_extension(path: &Path, name: &str, extension: &str) -> Vec<String> {
  SearchBuilder::default()
    .location(path)
    .search_input(name)
    .ext(extension)
    .strict()
    .ignore_case()
    .hidden()
    .build()
    .collect()
}

/// Checks if the given path is a valid ESP-IDF directory.
///
/// # Purpose
///
/// This function verifies if the specified directory contains a valid ESP-IDF setup by checking for the existence of the "tools.json" file in the "tools" subdirectory.
///
/// # Parameters
///
/// - `path`: A reference to a string representing the path to be checked.
///
/// # Return Value
///
/// - `bool`: Returns `true` if the specified path is a valid ESP-IDF directory, and `false` otherwise.
pub fn is_valid_idf_directory(path: &str) -> bool {
    let path = PathBuf::from(path);
    let tools_path = path.join("tools");
    let tools_json_path = tools_path.join("tools.json");
    if !tools_json_path.exists() {
        return false;
    }
    match read_and_parse_tools_file(tools_json_path.to_str().unwrap()) {
        Ok(_) => {
            true
        }
        Err(_) => {
            false
        }
    }
}

/// Filters out duplicate paths from a vector of strings.
///
/// This function checks for duplicate paths in the input vector and removes them.
/// It uses different strategies based on the operating system:
/// - On Windows, it compares the modification time and size of each file to identify duplicates.
/// - On Unix-like systems, it uses the device ID and inode number to identify duplicates.
///
/// # Parameters
///
/// - `paths`: A vector of strings representing file paths.
///
/// # Return Value
///
/// - A vector of strings containing the unique paths from the input vector.
pub fn filter_duplicate_paths(paths: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    match std::env::consts::OS {
        "windows" => {
            let mut seen = HashSet::new();
            for path in paths {
                if let Ok(metadata) = fs::metadata(&path) {
                    let key = format!("{:?}-{:?}", metadata.modified().ok(), metadata.len());

                    if seen.insert(key) {
                        result.push(path);
                    }
                } else {
                    result.push(path);
                }
            }
        }
        _ => {
            #[cfg(not(windows))]
            let mut seen = HashSet::new();
            #[cfg(not(windows))]
            for path in paths {
                // Get the metadata for the path
                if let Ok(metadata) = fs::metadata(&path) {
                    // Create a tuple of device ID and inode number
                    let file_id = (metadata.dev(), metadata.ino());

                    // Only keep the path if we haven't seen this file_id before
                    if seen.insert(file_id) {
                        result.push(path);
                    }
                } else {
                    // If we can't get metadata, keep the original path
                    result.push(path);
                }
            }
        }
    }

    result
}

/// Filters out subpaths from a vector of strings.
///
/// This function checks for subpaths in the input vector and removes them.
/// It ensures that only the highest-level paths are retained.
///
/// # Parameters
///
/// - `paths`: A vector of strings representing file paths.
///
/// # Return Value
///
/// - A vector of strings containing the highest-level paths from the input vector.
///   Subpaths are removed, and only the highest-level paths are retained.
fn filter_subpaths(paths: Vec<String>) -> Vec<String> {
    let mut filtered = Vec::new();

    'outer: for path in paths {
        // Check if this path is a subpath of any already filtered path
        for other in &filtered {
            if path.starts_with(other) {
                continue 'outer;
            }
        }

        // Remove any previously added paths that are subpaths of this one
        filtered.retain(|other: &String| !other.starts_with(&path));

        // Add this path
        filtered.push(path);
    }

    filtered
}

/// Removes a directory and all its contents recursively.
///
/// This function attempts to remove a directory and all its contents, including subdirectories and files.
/// It handles cases where the directory or files are read-only on Windows.
///
/// # Parameters
///
/// - `path`: A reference to a type that implements the `AsRef<Path>` trait, representing the path to the directory to be removed.
///
/// # Return Value
///
/// - `io::Result<()>`: If the directory and its contents are successfully removed, the function returns `Ok(())`.
///   If an error occurs during the process, the function returns an `io::Error` containing the specific error details.
pub fn remove_directory_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let expanded = crate::expand_tilde(path.as_ref());
    let path = Path::new(&expanded);

    if !path.exists() {
        warn!("Directory {} does not exist, didn't remove", path.display());
        return Ok(());
    }

    // First ensure all contents are writable to handle readonly files
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                // On Windows, we need to ensure the file is writable before removal
                #[cfg(windows)]
                {
                    let metadata = fs::metadata(&path)?;
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(&path, permissions)?;
                }
                fs::remove_file(&path)?;
            }
        }
    }

    // Now remove the directory itself
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }

    Ok(())
}

/// Retry wrapper function that takes a closure and retries it according to the configuration
pub fn with_retry<F, T, E>(f: F, max_retries: usize) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
    E: std::fmt::Debug,
{
    let mut attempt = 0;

    loop {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) => {
                attempt += 1;
                if attempt >= max_retries {
                    return Err(e);
                }

                debug!("Attempt {} failed with error: {:?}", attempt, e);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdfToolsConfig {
    pub id: i64,
    #[serde(rename = "idfLocation")]
    pub idf_location: String,
    #[serde(rename = "idfVersion")]
    pub idf_version: String,
    pub active: bool,
    #[serde(rename = "systemGitExecutablePath")]
    pub system_git_executable_path: String,
    #[serde(rename = "systemPythonExecutablePath")]
    pub system_python_executable_path: String,
    #[serde(rename = "envVars")]
    pub env_vars: HashMap<String, String>,
}

fn extract_tools_path_from_python_env_path(path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    path.ancestors()
        .find(|p| p.file_name().is_some_and(|name| name == "python_env"))
        .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
}

/// Parses and processes a configuration file for IDF tools.
///
/// # Purpose
///
/// This function reads a JSON configuration file containing information about different IDF tool sets.
/// It then processes this information to update the IDF installation configuration.
///
/// # Parameters
///
/// - `config_path`: A string representing the path to the configuration file.
///
/// # Return Value
///
/// This function does not return a value.
///
/// # Errors
///
/// This function logs errors to the console if the configuration file cannot be read or parsed.
/// It also logs errors if the IDF installation configuration cannot be updated.
pub fn parse_tool_set_config(config_path: &str) -> Result<()> {
    let config_path = Path::new(config_path);
    let json_str = std::fs::read_to_string(config_path).unwrap();
    let config: Vec<IdfToolsConfig> = match serde_json::from_str(&json_str) {
        Ok(config) => config,
        Err(e) => return Err(anyhow!("Failed to parse config file: {}", e)),
    };
    if let Some(tool_set) = config.into_iter().next() {
        let new_idf_tools_path = extract_tools_path_from_python_env_path(
            tool_set.env_vars.get("IDF_PYTHON_ENV_PATH").unwrap(),
        )
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        let new_export_paths = vec![tool_set.env_vars.get("PATH").unwrap().to_string()];
        let tmp = PathBuf::from(tool_set.idf_location.clone());
        let version_path = tmp.parent().unwrap();
        single_version_post_install(
            version_path.to_str().unwrap(),
            &tool_set.idf_location,
            &tool_set.idf_version,
            &new_idf_tools_path,
            new_export_paths,
        );

        let new_activation_script = match std::env::consts::OS {
            "windows" => format!(
                "{}\\Microsoft.PowerShell_profile.ps1",
                version_path.to_str().unwrap()
            ),
            _ => format!(
                "{}/activate_idf_{}.sh",
                version_path.to_str().unwrap(),
                tool_set.idf_version
            ),
        };
        let installation = IdfInstallation {
            id: tool_set.id.to_string(),
            activation_script: new_activation_script,
            path: tool_set.idf_location,
            name: tool_set.idf_version,
            python: tool_set.system_python_executable_path,
            idf_tools_path: new_idf_tools_path,
        };
        let config_path = get_default_config_path();
        let mut current_config = match IdfConfig::from_file(&config_path) {
            Ok(config) => config,
            Err(e) => {
                return Err(anyhow!("Config file not found: {}", e));
            }
        };
        current_config.idf_installed.push(installation);
        match current_config.to_file(config_path, true, false) {
            Ok(_) => {
                debug!("Updated config file with new tool set");
                return Ok(());
            }
            Err(e) => {
                return Err(anyhow!("Failed to update config file: {}", e));
            }
        }
    }
    Ok(())
}

/// Converts a path to a long path compatible with Windows.
///
/// This function takes a string representing a path and returns a new string.
/// If the input path is on a Windows system and does not already start with `\\?\`,
/// the function converts the path to a long path by canonicalizing the path,
/// and then adding the `\\?\` prefix.
/// If the input path is not on a Windows system or already starts with `\\?\`,
/// the function returns the input path unchanged.
///
/// # Parameters
///
/// * `path`: A string representing the path to be converted.
///
/// # Return Value
///
/// A string representing the converted path.
/// If the input path is on a Windows system and does not already start with `\\?\`,
/// the returned string will be a long path with the `\\?\` prefix.
/// If the input path is not on a Windows system or already starts with `\\?\`,
/// the returned string will be the same as the input path.
pub fn make_long_path_compatible(path: &str) -> String {
    if std::env::consts::OS == "windows" && !path.starts_with(r"\\?\") {
        // Convert to absolute path and add \\?\ prefix
        let absolute_path = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));

        let mut long_path = PathBuf::from(r"\\?\");
        long_path.push(absolute_path);
        long_path.to_str().unwrap().to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tempfile::TempDir;

    #[test]
    fn test_find_directories_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create test directory structure
        let test_dir1 = base_path.join("test_dir");
        let test_dir2 = base_path.join("subdir").join("test_dir");
        fs::create_dir_all(&test_dir1).unwrap();
        fs::create_dir_all(&test_dir2).unwrap();

        let results = find_directories_by_name(base_path, "test_dir");
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .any(|p| p.contains(test_dir1.to_str().unwrap())));
        assert!(results
            .iter()
            .any(|p| p.contains(test_dir2.to_str().unwrap())));
    }

    #[test]
    fn test_is_valid_idf_directory() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create invalid directory (no tools.json)
        assert!(!is_valid_idf_directory(base_path.to_str().unwrap()));

        // Create valid IDF directory structure
        let tools_dir = base_path.join("tools");
        fs::create_dir_all(&tools_dir).unwrap();
        let tools_json_path = tools_dir.join("tools.json");
        let mut file = File::create(tools_json_path).unwrap();
        write!(file, r#"{{"tools": [], "version": 1}}"#).unwrap();

        assert!(is_valid_idf_directory(base_path.to_str().unwrap()));
    }

    #[test]
    fn test_filter_duplicate_paths() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create test files with different content
        let file1_path = base_path.join("file1.txt");
        let file2_path = base_path.join("file2.txt");

        fs::write(&file1_path, "content1").unwrap();
        let duration = std::time::Duration::from_millis(1000); // Sleep for 1 second
        std::thread::sleep(duration); // because on windows we use the modified time to identify duplicates
        fs::write(&file2_path, "content2").unwrap();

        let paths = vec![
            file1_path.to_string_lossy().to_string(),
            file1_path.to_string_lossy().to_string(), // Duplicate
            file2_path.to_string_lossy().to_string(),
        ];

        let filtered = filter_duplicate_paths(paths);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_subpaths() {
        let paths = vec![
            "/path/to/dir".to_string(),
            "/path/to/dir/subdir".to_string(),
            "/path/to/another".to_string(),
        ];

        let filtered = filter_subpaths(paths);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"/path/to/dir".to_string()));
        assert!(filtered.contains(&"/path/to/another".to_string()));
        assert!(!filtered.contains(&"/path/to/dir/subdir".to_string()));
    }

    #[test]
    fn test_remove_directory_all() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create test directory structure
        let test_dir = base_path.join("test_dir");
        let test_subdir = test_dir.join("subdir");
        let test_file = test_dir.join("test.txt");

        fs::create_dir_all(&test_subdir).unwrap();
        fs::write(&test_file, "test content").unwrap();

        // Test removal
        assert!(remove_directory_all(&test_dir).is_ok());
        assert!(!test_dir.exists());
    }

    #[test]
    fn test_remove_directory_all_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let non_existent = temp_dir.path().join("non_existent");

        assert!(remove_directory_all(&non_existent).is_ok());
    }

    #[test]
    fn test_remove_directory_all_readonly() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("readonly_dir");
        let test_file = test_dir.join("readonly.txt");

        fs::create_dir_all(&test_dir).unwrap();
        fs::write(&test_file, "readonly content").unwrap();

        #[cfg(windows)]
        {
            let metadata = fs::metadata(&test_file).unwrap();
            let mut permissions = metadata.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&test_file, permissions).unwrap();
        }

        assert!(remove_directory_all(&test_dir).is_ok());
        assert!(!test_dir.exists());
    }
    #[test]
    fn test_retry_success_after_failure() {
        let counter = AtomicU32::new(0);

        let result = with_retry(
            || {
                let current = counter.fetch_add(1, Ordering::SeqCst);
                if current < 2 {
                    Err("Not ready yet")
                } else {
                    Ok("Success!")
                }
            },
            3,
        );

        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
    #[test]
    fn test_retry_all_attempts_failed() {
        let counter = AtomicU32::new(0);

        let result: Result<&str, &str> = with_retry(
            || {
                counter.fetch_add(1, Ordering::SeqCst);
                Err("Always fails")
            },
            3,
        );

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
