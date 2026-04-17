use std::{fs, io::{self, Read, Write}, path::{Path, PathBuf}};

use log::{debug, error, info, warn};
use tempfile::TempDir;

use crate::{add_path_to_path, command_executor::{self, execute_command}, render_template, settings::Settings, system_dependencies::{add_to_path, get_correct_powershell_command}, utils::{copy_dir_contents,copy_dir_contents_preserving_mtime, extract_zst_archive}};

/// Installs prerequisite software packages from an offline archive.
///
/// This function handles the installation of development prerequisites on different
/// operating systems. On Windows, it installs Git and Python from the archive.
/// On Linux and macOS, it simply logs that users should ensure necessary tools are
/// installed manually.
///
/// # Arguments
///
/// * `archive_dir` - Temporary directory containing the offline installation archive
/// * `tools_dir` - Directory where tools (git, python) should be installed
///
/// # Returns
///
/// * `Ok(())` - Prerequisites installed successfully
/// * `Err(String)` - Error message if installation fails
///
/// # Platform Behavior
///
/// * **Windows**: Installs Git and Python from the offline archive
/// * **Linux/macOS**: Logs informational message about manual tool installation
/// * **Other platforms**: Returns error for unsupported operating systems
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
/// use std::path::PathBuf;
///
/// let archive_dir = TempDir::new().unwrap();
/// let tools_dir = PathBuf::from("C:\\tools");
/// install_prerequisites_offline(&archive_dir, tools_dir)?;
/// ```
pub fn install_prerequisites_offline(
    archive_dir: &TempDir,
    tools_dir: PathBuf,
) -> Result<(), String> {
    match std::env::consts::OS {
        "windows" => {
            // Ensure tools directory exists
            crate::ensure_path(tools_dir.to_str().unwrap())
                .map_err(|e| format!("Failed to create tools directory: {}", e))?;

            // Git is expected to be in archive_dir as PortableGit-X.Y.Z-64-bit.7z.exe
            let git_exe = archive_dir.path().join("PortableGit-2.47.0.2-64-bit.7z.exe");
            if !git_exe.exists() {
                return Err(format!("Git installer not found in archive: {}", git_exe.display()));
            }

            // Copy git to tools_dir for installation
            let git_in_tools = tools_dir.join("PortableGit-2.47.0.2-64-bit.7z.exe");
            std::fs::copy(&git_exe, &git_in_tools)
                .map_err(|e| format!("Failed to copy git installer: {}", e))?;

            // Install git from the copied file
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;
            match rt.block_on(crate::system_dependencies::install_git_from_downloaded(
                tools_dir.clone(),
                Some(git_in_tools),
            )) {
                Ok(install_path) => {
                    info!("Git installed successfully to {:?}", install_path);
                }
                Err(e) => {
                    return Err(format!("Failed to install git: {}", e));
                }
            }

            // Python is expected to be in archive_dir as cpython-X.Y.Z+-x86_64-pc-windows-msvc-install_only.tar.gz
            let python_archive = archive_dir.path().join("cpython-3.12.8+20250107-x86_64-pc-windows-msvc-install_only.tar.gz");
            if !python_archive.exists() {
                return Err(format!("Python archive not found in archive: {}", python_archive.display()));
            }

            // Copy python archive to tools_dir for installation
            let python_in_tools = tools_dir.join("cpython-3.12.8+20250107-x86_64-pc-windows-msvc-install_only.tar.gz");
            std::fs::copy(&python_archive, &python_in_tools)
                .map_err(|e| format!("Failed to copy python archive: {}", e))?;

            // Install python from the copied archive
            match rt.block_on(crate::system_dependencies::install_python_from_downloaded(
                tools_dir.clone(),
                Some(python_in_tools),
            )) {
                Ok(install_path) => {
                    info!("Python installed successfully to {:?}", install_path);
                }
                Err(e) => {
                    return Err(format!("Failed to install python: {}", e));
                }
            }
        }
        _ => {
            info!("On non-Windows system, prerequisites installation from offline archive is not supported.");
        }
    }
    Ok(())
}

/// Copies ESP-IDF (Espressif IoT Development Framework) files from offline archive.
///
/// This function copies ESP-IDF framework files from an offline archive to the
/// target installation directory. It handles multiple IDF versions as specified
/// in the configuration and uses platform-specific copy mechanisms to handle
/// Windows path length limitations.
///
/// # Arguments
///
/// * `archive_dir` - Temporary directory containing the offline ESP-IDF archive
/// * `config` - Settings containing IDF versions and target installation path
///
/// # Returns
///
/// * `Ok(())` - All IDF versions copied successfully
/// * `Err(String)` - Error message if any copy operation fails
///
/// # Platform Behavior
///
/// * **Windows**: Uses PowerShell `cp -r` command to handle long path names
/// * **Other platforms**: Uses custom `copy_dir_contents` utility function
///
/// # Configuration Requirements
///
/// The `Settings` struct must contain:
/// * `idf_versions` - Optional vector of IDF version strings to copy
/// * `path` - Optional target path where IDF versions should be installed
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
///
/// let archive_dir = TempDir::new().unwrap();
/// let config = Settings {
///     idf_versions: Some(vec!["v4.4".to_string(), "v5.0".to_string()]),
///     path: Some(PathBuf::from("C:\\esp\\esp-idf")),
///     // ... other fields
/// };
///
/// copy_idf_from_offline_archive(&archive_dir, &config)?;
/// ```
///
/// # Error Handling
///
/// The function continues copying other versions even if one fails, but returns
/// an error if any copy operation was unsuccessful. Individual failures are
/// logged with error-level messages.
pub fn copy_idf_from_offline_archive(
    archive_dir: &TempDir,
    config: &Settings
) -> Result<(), String> {
    let mut everything_copied = true;
    for archive_version in config.clone().idf_versions.unwrap() {
        let src_path = archive_dir.path().join(&archive_version);
        let dst_path = config.clone().path.unwrap().join(&archive_version);

        if let Some(parent) = dst_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    error!("Failed to create destination directory: {}", e);
                    everything_copied = false;
                    continue;
                }
            }
        }

        match std::env::consts::OS {
            "windows" => {
                // robocopy handles long paths (no MAX_PATH limit) and preserves timestamps
                // via /COPY:DAT (Data, Attributes, Timestamps). Exit codes 0-7 are success.
                let output = command_executor::execute_command(
                    "robocopy",
                    &[
                        src_path.to_string_lossy().as_ref(),
                        dst_path.to_string_lossy().as_ref(),
                        "/E",
                        "/COPY:DAT",
                        "/R:3",
                        "/W:1",
                        "/NP",
                        "/NFL",
                        "/NDL",
                    ],
                );
                match output {
                    Ok(out) => {
                        let exit_code = out.status.code().unwrap_or(-1);
                        if exit_code <= 7 {
                            info!("Successfully copied IDF version {} (robocopy exit code: {})", archive_version, exit_code);
                        } else {
                            error!("robocopy failed for version {} with exit code {}: {:?} | {:?}",
                                archive_version, exit_code, out.stdout, out.stderr);
                            everything_copied = false;
                        }
                    }
                    Err(err) => {
                        error!("Failed to execute robocopy for version {}: {}", archive_version, err);
                        everything_copied = false;
                    }
                }
            },
            _ => {
                debug!("Moving IDF version: {}", archive_version);
                match fs::rename(&src_path, &dst_path) {
                    Ok(_) => {
                        info!("Successfully moved IDF version: {}", archive_version);
                    }
                    Err(rename_err) => {
                        debug!("fs::rename failed (likely cross-filesystem): {}, falling back to mtime-preserving copy", rename_err);
                        // For copy, we need to copy the esp-idf subdirectory specifically
                        // since the archive structure is: version/esp-idf/...
                        let src_idf_path = src_path.join("esp-idf");
                        let dst_idf_path = dst_path.join("esp-idf");
                        match copy_dir_contents_preserving_mtime(&src_idf_path, &dst_idf_path) {
                            Ok(_) => {
                                info!("Successfully copied IDF version with preserved timestamps: {}", archive_version);
                            }
                            Err(err) => {
                                error!("Failed to copy IDF version {}: {}", archive_version, err);
                                everything_copied = false;
                            }
                        }
                    }
                }
            }
        }
    }
    if everything_copied {
        Ok(())
    } else {
        Err("Failed to copy some IDF versions".into())
    }
}

pub fn copy_components_from_offline_archive(
    archive_dir: &TempDir,
    target_dir: &Path,
) -> Result<(), String> {
    match crate::utils::copy_dir_contents(&archive_dir.path().join("components"), target_dir) {
        Ok(_) => {
            info!("Successfully copied components from offline archive to: {}", target_dir.display());
            Ok(())
        }
        Err(err) => {
            Err(format!("Failed to copy components from offline archive: {}", err))
        }
    }
}

pub fn use_offline_archive(mut config: Settings, offline_archive_dir: &TempDir) -> Result<Settings, String> {
    debug!("Using offline archive: {:?}", config.use_local_archive);
    if !config.use_local_archive.as_ref().unwrap().exists() {
        return Err(format!(
            "Local archive path does not exist: {}",
            config.use_local_archive.as_ref().unwrap().display()
        ));
    }
    match extract_zst_archive(&config.use_local_archive.as_ref().unwrap(), &offline_archive_dir.path()) {
      Ok(_) => {
          info!("Successfully extracted archive to: {:?}", offline_archive_dir);
      }
      Err(err) => {
          return Err(format!("Failed to extract archive: {}", err));
      }
    }
    let config_path = offline_archive_dir.path().join("config.toml");
    if config_path.exists() {
      debug!("Loading config from extracted archive: {}", config_path.display());
      let mut tmp_setting = Settings::default();
      match Settings::load(&mut tmp_setting, &config_path.to_str().unwrap()) {
        Ok(()) => {
          debug!("Config loaded from archive: {:?}", config_path.display());
          debug!("Config: {:?}", tmp_setting);
          debug!("Using only version for now.");
          config.idf_versions = tmp_setting.idf_versions;
      }
        Err(err) => {
          return Err(format!("Failed to load config from archive: {}", err));
        }
      }
    } else {
      warn!("Config file not found in archive: {}. Continuing with default config.", config_path.display());
    }
    Ok(config)
}

/// Finds all 'requirements.*' files in a given directory,
/// merges their content, and writes it to 'requirements.merged.txt'.
///
/// # Arguments
/// * `folder_path` - The path to the directory to search.
///
/// # Returns
/// `Result<(), io::Error>` - Ok(()) on success, or an io::Error on failure.
pub fn merge_requirements_files(folder_path: &Path) -> Result<(), io::Error> {
    let mut merged_content = String::new();
    let mut requirements_found = false;

    // Ensure the folder exists and is a directory
    if !folder_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Folder not found: {}", folder_path.display()),
        ));
    }
    if !folder_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Path is not a directory: {}", folder_path.display()),
        ));
    }

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                if file_name.starts_with("requirements.") && file_name != "requirements.merged.txt" {
                    requirements_found = true;
                    debug!("Merging file: {}", path.display());
                    let mut file = fs::File::open(&path)?;
                    file.read_to_string(&mut merged_content)?;
                    // Add a newline to separate content from different files, if they don't end with one
                    if !merged_content.ends_with('\n') && !merged_content.is_empty() {
                        merged_content.push('\n');
                    }
                }
            }
        }
    }

    if !requirements_found {
        warn!("No 'requirements.*' files found in {}", folder_path.display());
        return Ok(()); // Or return an error if you consider it an error
    }

    let output_file_path = folder_path.join("requirements.merged.txt");
    let mut output_file = fs::File::create(&output_file_path)?;
    output_file.write_all(merged_content.as_bytes())?;

    info!("Successfully merged requirements files to: {}", output_file_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_copy_components_from_offline_archive_success() {
        // Create a temporary archive directory with components
        let archive_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create a components directory in the archive
        let components_src = archive_dir.path().join("components");
        fs::create_dir_all(&components_src).unwrap();

        // Add some test files
        let test_component = components_src.join("test_component");
        fs::create_dir_all(&test_component).unwrap();
        fs::write(test_component.join("test.txt"), "test content").unwrap();

        // Call the function
        let result = copy_components_from_offline_archive(
            &archive_dir,
            target_dir.path(),
        );

        // Verify success
        assert!(result.is_ok());

        // Verify files were copied
        let copied_component = target_dir.path().join("test_component");
        assert!(copied_component.exists());
        assert!(copied_component.join("test.txt").exists());
    }

    #[test]
    fn test_copy_components_from_offline_archive_no_components_dir() {
        // Create a temporary archive directory without components
        let archive_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Don't create components directory

        // Call the function - should fail
        let result = copy_components_from_offline_archive(
            &archive_dir,
            target_dir.path(),
        );

        // Verify failure
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to copy"));
    }
}
