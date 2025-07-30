use crate::{
    command_executor::execute_command,
    idf_config::{IdfConfig, IdfInstallation},
    idf_tools::read_and_parse_tools_file,
    single_version_post_install,
    version_manager::get_default_config_path,
};
use anyhow::{anyhow, Result, Error};
use git2::Repository;
use log::{debug, error, info, warn};
use rust_search::SearchBuilder;
use serde::{Deserialize, Serialize};
use tar::Archive;
use zstd::{decode_all, Decoder};
#[cfg(not(windows))]
use std::os::unix::fs::MetadataExt;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};
use regex::Regex;

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
    let mut settings = crate::settings::Settings::default();
    let config_path = get_default_config_path();
    let mut current_config = match IdfConfig::from_file(&config_path) {
      Ok(config) => config,
      Err(_e) => {
        info!("Config file not found, creating a new one at: {}", config_path.display());
        settings.idf_versions = Some(vec![]);
        match settings.save_esp_ide_json() {
          Ok(_) => info!("Created new config file at: {}", config_path.display()),
          Err(e) => error!("Failed to create config file: {}", e),
        }
        IdfConfig::from_file(&config_path).unwrap()
      }
    };
    for tool_set in config.into_iter() {
        let new_idf_tools_path = extract_tools_path_from_python_env_path(
            tool_set.env_vars.get("IDF_PYTHON_ENV_PATH").unwrap(),
        )
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
        let new_export_paths = vec![tool_set.env_vars.get("PATH").unwrap().to_string()];

        let paths = settings.get_version_paths(&tool_set.idf_version)?;
        let idf_python_env_path = tool_set
            .env_vars
            .get("IDF_PYTHON_ENV_PATH")
            .map(|s| s.to_string());

        let mut env_vars = tool_set.env_vars.clone();
        env_vars.remove("PATH");
        env_vars.remove("ESP_IDF_VERSION");
        let env_vars_vec = env_vars.into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect::<Vec<(String, String)>>();

        single_version_post_install(
            &paths.activation_script_path.to_string_lossy().into_owned(),
            &tool_set.idf_location,
            &tool_set.idf_version,
            &new_idf_tools_path,
            new_export_paths,
            idf_python_env_path.as_deref(),
            Some(env_vars_vec),
        );

        let python = match std::env::consts::OS {
            "windows" => PathBuf::from(idf_python_env_path.unwrap()).join("Scripts").join("python.exe"),
            _ => PathBuf::from(idf_python_env_path.unwrap()).join("bin").join("python"),
        };
        let installation = IdfInstallation {
            id: tool_set.id.to_string(),
            activation_script: paths.activation_script.to_string_lossy().into_owned(),
            path: tool_set.idf_location,
            name: tool_set.idf_version,
            python: python.to_str()
                .unwrap()
                .to_string(),
            idf_tools_path: new_idf_tools_path,
        };

        current_config.idf_installed.push(installation);

    }
    match current_config.to_file(config_path, true, true) {
      Ok(_) => {
        debug!("Updated config file with new tool set");
        return Ok(())
      }
      Err(e) => {
        return Err(anyhow!("Failed to update config file: {}", e))
      }
    }
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

/// Removes everything after the second dot in a string (including the second dot).
///
/// This function searches for the first two dots in the input string and returns
/// a new string containing only the characters up to the second dot.
/// If the string contains fewer than two dots, the original string is returned unchanged.
///
/// # Arguments
///
/// * `s` - A string slice to process
///
/// # Returns
///
/// A `String` containing the input up to and not including the second dot, or the
/// original string if it contains fewer than two dots.
///
/// # Examples
///
/// ```
/// let result = remove_after_second_dot("hello.world.foo.bar");
/// assert_eq!(result, "hello.world");
///
/// let result = remove_after_second_dot("one.two");
/// assert_eq!(result, "one.two");
/// ```
pub fn remove_after_second_dot(s: &str) -> String {
  if let Some(first_dot) = s.find('.') {
      if let Some(second_dot) = s[first_dot + 1..].find('.') {
          return s[..first_dot + 1 + second_dot].to_string();
      }
  }
  s.to_string()
}

/// Parses the IDF version from an ESP-IDF installation CMakec file.
///
/// This function reads the `version.cmake` file from the ESP-IDF tools directory
/// and extracts the major and minor version numbers from the IDF_VERSION_MAJOR
/// and IDF_VERSION_MINOR variables.
///
/// # Arguments
///
/// * `idf_path` - A string slice containing the path to the ESP-IDF installation directory
///
/// # Returns
///
/// A `Result` containing a tuple of `(major, minor)` version strings on success,
/// or an error if:
/// - The version.cmake file doesn't exist
/// - The file cannot be read
/// - The version numbers cannot be parsed
/// - Either major or minor version is missing
///
/// # File Format Expected
///
/// The function expects lines in the format:
/// ```cmake
/// set(IDF_VERSION_MAJOR 5)
/// set(IDF_VERSION_MINOR 1)
/// ...
/// ```
///
/// # Examples
///
/// ```
/// # use std::fs;
/// # use std::path::Path;
/// # fn main() -> anyhow::Result<()> {
/// // Assuming you have a valid ESP-IDF installation
/// let (major, minor) = parse_cmake_version("/path/to/esp-idf")?;
/// println!("ESP-IDF version: {}.{}", major, minor);
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an error if the version.cmake file is not found, cannot be read,
/// or does not contain valid version information.
pub fn parse_cmake_version(idf_path: &str) -> Result<(String, String)> {
    let mut cmake_path = PathBuf::from(idf_path);
    cmake_path.push("tools");
    cmake_path.push("cmake");
    cmake_path.push("version.cmake");

    // Check if file exists
    if !cmake_path.exists() {
        return Err(anyhow!("CMake version file not found at: {}", cmake_path.display()));
    }

    // Read the file content
    let content = fs::read_to_string(&cmake_path)
        .map_err(|e| anyhow!("Failed to read CMake version file: {}", e))?;

    // Regex to extract numbers from lines
    let re = Regex::new(r"\d+")
        .map_err(|e| anyhow!("Failed to compile regex: {}", e))?;

    // Parse major and minor versions
    let mut major = None;
    let mut minor = None;
    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("set(IDF_VERSION_MAJOR") {
            if let Some(captures) = re.find(line) {
                major = Some(captures.as_str().parse::<u32>());
            }
        } else if line.starts_with("set(IDF_VERSION_MINOR") {
            if let Some(captures) = re.find(line) {
                minor = Some(captures.as_str().parse::<u32>());
            }
        }
    }
    if let (Some(Ok(maj)), Some(Ok(min))) = (major, minor) {
        return Ok((maj.to_string(), min.to_string()));
    }
    Err(anyhow!("Could not find both major and minor version numbers"))
}

/// Parse version string and extract major.minor components
pub fn parse_version_major_minor(version: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            return Some((major, minor));
        }
    }
    None
}

/// Compare two versions considering only major and minor components
pub fn versions_match(installed: &str, expected: &str) -> bool {
    match (parse_version_major_minor(installed), parse_version_major_minor(expected)) {
        (Some((inst_major, inst_minor)), Some((exp_major, exp_minor))) => {
            inst_major == exp_major && inst_minor == exp_minor
        }
        _ => false,
    }
}

pub fn get_commit_hash(repo_path: &str) -> Result<String, git2::Error> {
    let repo = Repository::open(repo_path)?;
    // Get the HEAD reference
    let head = repo.head()?;
    // Get the commit that HEAD points to
    let commit = head.peel_to_commit()?;
    Ok(commit.id().to_string()[..7].to_string()) // Return the first 7 characters of the commit hash
}

pub fn extract_zst_archive_with_buffer_size(
    archive_path: &Path,
    extract_to: &Path,
    buffer_size: usize
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Extracting archive: {:?} to: {:?} with buffer size: {}", archive_path, extract_to, buffer_size);

    // Create extraction directory if it doesn't exist
    std::fs::create_dir_all(extract_to)?;

    // Open the compressed file
    let file = File::open(archive_path)?;
    let buf_reader = BufReader::with_capacity(buffer_size, file);

    info!("Setting up zstd decoder with custom buffer size...");
    // Create a streaming zstd decoder
    let decoder = Decoder::new(buf_reader)?;

    info!("Extracting tar archive...");
    // Create tar archive from the streaming decoder
    let mut archive = Archive::new(decoder);

    // Extract the archive directly from the stream
    archive.unpack(extract_to)?;

    info!("Archive extracted successfully to: {:?}", extract_to);
    Ok(())
}

/// Convenience function that replaces the original with better defaults
pub fn extract_zst_archive(archive_path: &Path, extract_to: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Use the streaming version with a reasonable default buffer size (64KB)
    extract_zst_archive_with_buffer_size(archive_path, extract_to, 64 * 1024)
}

pub fn copy_dir_contents(src: &Path, dst: &Path) -> io::Result<()> {
    copy_dir_contents_with_retries(src, dst, 3, std::time::Duration::from_millis(100))
}

pub fn copy_dir_contents_with_retries(
    src: &Path,
    dst: &Path,
    max_retries: u32,
    retry_delay: std::time::Duration
) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid file name for path: {:?}", path),
            )
        })?;
        let dest_path = dst.join(file_name);

        if path.is_dir() {
            // Recursively copy subdirectories
            copy_dir_contents_with_retries(&path, &dest_path, max_retries, retry_delay)?;
        } else {
            // Copy files with retry logic
            copy_file_with_retries(&path, &dest_path, max_retries, retry_delay)?;
        }
    }
    Ok(())
}

fn copy_file_with_retries(
    src: &Path,
    dst: &Path,
    max_retries: u32,
    retry_delay: std::time::Duration
) -> io::Result<()> {
    let mut attempts = 0;

    loop {
        match fs::copy(src, dst) {
            Ok(_) => return Ok(()),
            Err(e) => {
                attempts += 1;

                // Check if it's a retryable error
                if is_retryable_error(&e) && attempts <= max_retries {
                    eprintln!("Warning: Failed to copy {:?} to {:?} (attempt {}/{}): {}. Retrying...",
                             src, dst, attempts, max_retries, e);
                    std::thread::sleep(retry_delay);
                    continue;
                } else {
                    // Add more context to the error
                    return Err(io::Error::new(
                        e.kind(),
                        format!("Failed to copy {:?} to {:?} after {} attempts: {}",
                               src, dst, attempts, e)
                    ));
                }
            }
        }
    }
}

fn is_retryable_error(error: &io::Error) -> bool {
    match error.raw_os_error() {
        Some(5) => true,   // ERROR_ACCESS_DENIED
        Some(32) => true,  // ERROR_SHARING_VIOLATION
        Some(33) => true,  // ERROR_LOCK_VIOLATION
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tempfile::TempDir;
    use tar::Builder;
    use zstd::stream::write::Encoder;

    fn create_test_zst_archive(temp_dir: &TempDir) -> std::path::PathBuf {
        // Create some test files
        let test_dir = temp_dir.path().join("test_content");
        fs::create_dir_all(&test_dir).unwrap();

        for i in 0..5 {
            let file_path = test_dir.join(format!("test_file_{}.txt", i));
            fs::write(&file_path, format!("Test content for file {}", i)).unwrap();
        }

        // Create tar.zst archive
        let archive_path = temp_dir.path().join("test_archive.tar.zst");
        let file = File::create(&archive_path).unwrap();
        let encoder = Encoder::new(file, 0).unwrap(); // Compression level 0 for speed
        let mut tar_builder = Builder::new(encoder);

        tar_builder.append_dir_all("test_content", &test_dir).unwrap();
        let encoder = tar_builder.into_inner().unwrap();
        encoder.finish().unwrap();

        archive_path
    }

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

    #[test]
    fn test_multiple_dots() {
        assert_eq!(
            remove_after_second_dot("hello.world.foo.bar"),
            "hello.world"
        );
        assert_eq!(
            remove_after_second_dot("a.b.c.d.e"),
            "a.b"
        );
    }

    #[test]
    fn test_exactly_two_dots() {
        assert_eq!(
            remove_after_second_dot("first.second."),
            "first.second"
        );
        assert_eq!(
            remove_after_second_dot("one.two.three"),
            "one.two"
        );
    }

    #[test]
    fn test_one_dot() {
        assert_eq!(
            remove_after_second_dot("hello.world"),
            "hello.world"
        );
        assert_eq!(
            remove_after_second_dot("test."),
            "test."
        );
    }

    #[test]
    fn test_no_dots() {
        assert_eq!(
            remove_after_second_dot("hello"),
            "hello"
        );
        assert_eq!(
            remove_after_second_dot(""),
            ""
        );
    }

    #[test]
    fn test_dots_at_start() {
        assert_eq!(
            remove_after_second_dot("..rest"),
            "."
        );
        assert_eq!(
            remove_after_second_dot(".hello.world.foo"),
            ".hello"
        );
    }

    #[test]
    fn test_consecutive_dots() {
        assert_eq!(
            remove_after_second_dot("hello...world"),
            "hello."
        );
    }

    #[test]
    fn test_parse_cmake_version() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary directory structure
        let temp_dir = TempDir::new()?;
        let tools_dir = temp_dir.path().join("tools").join("cmake");
        fs::create_dir_all(&tools_dir)?;

        // Create the version.cmake file
        let version_file = tools_dir.join("version.cmake");
        let content = r#"set(IDF_VERSION_MAJOR 6)
set(IDF_VERSION_MINOR 0)
set(IDF_VERSION_PATCH 0)

set(ENV{IDF_VERSION} "${IDF_VERSION_MAJOR}.${IDF_VERSION_MINOR}.${IDF_VERSION_PATCH}")
"#;
        fs::write(&version_file, content)?;

        // Test the function
        let version = parse_cmake_version(temp_dir.path().to_str().unwrap())?;
        assert_eq!(version, ("6".to_string(), "0".to_string()));

        Ok(())
    }

    #[test]
    fn test_parse_cmake_version_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = parse_cmake_version(temp_dir.path().to_str().unwrap());
        assert!(result.is_err());
    }

    fn create_test_cmake_file(temp_dir: &TempDir, content: &str) -> PathBuf {
        let mut cmake_dir = temp_dir.path().to_path_buf();
        cmake_dir.push("tools");
        cmake_dir.push("cmake");
        fs::create_dir_all(&cmake_dir).unwrap();

        let cmake_file = cmake_dir.join("version.cmake");
        fs::write(&cmake_file, content).unwrap();
        temp_dir.path().to_path_buf()
    }

    #[test]
    fn test_valid_version_file() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
# ESP-IDF CMake version file
set(IDF_VERSION_MAJOR 5)
set(IDF_VERSION_MINOR 1)
set(IDF_VERSION_PATCH 2)
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_ok());
        let (major, minor) = result.unwrap();
        assert_eq!(major, "5");
        assert_eq!(minor, "1");
    }

    #[test]
    fn test_version_with_extra_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
    set(IDF_VERSION_MAJOR   4  )
  set(IDF_VERSION_MINOR 3)
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_ok());
        let (major, minor) = result.unwrap();
        assert_eq!(major, "4");
        assert_eq!(minor, "3");
    }

    #[test]
    fn test_version_with_comments() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
# This is a comment
set(IDF_VERSION_MAJOR 5) # Major version
# Another comment
set(IDF_VERSION_MINOR 2) # Minor version
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_ok());
        let (major, minor) = result.unwrap();
        assert_eq!(major, "5");
        assert_eq!(minor, "2");
    }

    #[test]
    fn test_missing_major_version() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
set(IDF_VERSION_MINOR 1)
set(IDF_VERSION_PATCH 2)
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not find both major and minor version numbers"));
    }

     #[test]
    fn test_missing_minor_version() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
set(IDF_VERSION_MAJOR 5)
set(IDF_VERSION_PATCH 2)
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not find both major and minor version numbers"));
    }

    #[test]
    fn test_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let idf_path = temp_dir.path().to_str().unwrap();

        let result = parse_cmake_version(idf_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CMake version file not found"));
    }

    #[test]
    fn test_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let content = "";
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not find both major and minor version numbers"));
    }

    #[test]
    fn test_malformed_version_numbers() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
set(IDF_VERSION_MAJOR abc)
set(IDF_VERSION_MINOR 1)
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Could not find both major and minor version numbers"));
    }

    #[test]
    fn test_different_order() {
        let temp_dir = TempDir::new().unwrap();
        let content = r#"
set(IDF_VERSION_PATCH 0)
set(IDF_VERSION_MINOR 4)
set(IDF_VERSION_MAJOR 5)
"#;
        let idf_path = create_test_cmake_file(&temp_dir, content);

        let result = parse_cmake_version(idf_path.to_str().unwrap());
        assert!(result.is_ok());
        let (major, minor) = result.unwrap();
        assert_eq!(major, "5");
        assert_eq!(minor, "4");
    }

    #[test]
    fn test_parse_version_major_minor() {
        assert_eq!(parse_version_major_minor("1.2.3"), Some((1, 2)));
        assert_eq!(parse_version_major_minor("14.2.0_20241119"), Some((14, 2)));
        assert_eq!(parse_version_major_minor("3.30.2"), Some((3, 30)));
        assert_eq!(parse_version_major_minor("1.0"), Some((1, 0)));
        assert_eq!(parse_version_major_minor("1"), None);
        assert_eq!(parse_version_major_minor("invalid"), None);
    }

    #[test]
    fn test_versions_match() {
        assert!(versions_match("1.2.3", "1.2.0"));
        assert!(versions_match("14.2.0_20241119", "14.2.1"));
        assert!(!versions_match("1.2.3", "1.3.0"));
        assert!(!versions_match("2.0.0", "1.2.0"));
        assert!(!versions_match("invalid", "1.2.0"));
    }

    #[test]
    fn test_extract_zst_archive_streaming() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = create_test_zst_archive(&temp_dir);
        let extract_to = temp_dir.path().join("extracted");

        let result = extract_zst_archive(&archive_path, &extract_to);
        assert!(result.is_ok());

        // Verify extracted files
        for i in 0..5 {
            let extracted_file = extract_to.join("test_content").join(format!("test_file_{}.txt", i));
            assert!(extracted_file.exists());
            let content = fs::read_to_string(&extracted_file).unwrap();
            assert_eq!(content, format!("Test content for file {}", i));
        }
    }

    #[test]
    fn test_extract_zst_archive_with_small_buffer() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = create_test_zst_archive(&temp_dir);
        let extract_to = temp_dir.path().join("extracted_small_buffer");

        // Use very small buffer to test memory efficiency
        let result = extract_zst_archive_with_buffer_size(&archive_path, &extract_to, 512);
        assert!(result.is_ok());

        // Verify extracted files
        for i in 0..5 {
            let extracted_file = extract_to.join("test_content").join(format!("test_file_{}.txt", i));
            assert!(extracted_file.exists());
        }
    }

    #[test]
    fn test_extract_nonexistent_archive() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent.tar.zst");
        let extract_to = temp_dir.path().join("extracted");

        let result = extract_zst_archive(&nonexistent_path, &extract_to);
        assert!(result.is_err());
    }
}
