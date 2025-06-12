use std::{fs, path::{Path, PathBuf}};

use log::{debug, error, info, warn};
use tempfile::TempDir;
use tera::{Context, Tera};

use crate::{add_path_to_path, command_executor::{self, execute_command}, settings::Settings, system_dependencies::{add_to_path, get_correct_powershell_command, get_scoop_path}, utils::copy_dir_contents};

/// Structure to define package information with compile-time template content.
///
/// This struct contains all the necessary information to install a Scoop package
/// including the template content, manifest filename, and test command to verify
/// successful installation.
pub struct ScoopPackage {
    /// The name of the package (e.g., "git", "python", "7zip")
    pub name: &'static str,
    /// The JSON template content for the Scoop manifest, embedded at compile time
    pub template_content: &'static str,
    /// The filename for the generated manifest file (e.g., "git.json")
    pub manifest_filename: &'static str,
    /// Command used to test if the package was installed successfully
    pub test_command: &'static str,
}

/// Creates and writes a Scoop manifest file for a given package.
///
/// This function takes a package definition, renders its template with the provided context,
/// normalizes line endings for Windows compatibility, and writes the resulting manifest
/// to the specified Scoop directory.
///
/// # Arguments
///
/// * `package` - The package definition containing template content and metadata
/// * `context` - Tera template context with variables for rendering
/// * `scoop_path` - Path to the Scoop directory where the manifest will be written
/// * `tera` - Mutable reference to the Tera template engine
///
/// # Returns
///
/// * `Ok(PathBuf)` - Path to the created manifest file on success
/// * `Err(String)` - Error message if template rendering or file writing fails
///
/// # Examples
///
/// ```rust
/// let package = ScoopPackage { /* ... */ };
/// let mut context = Context::new();
/// let mut tera = Tera::default();
/// let manifest_path = create_manifest(&package, &context, &scoop_path, &mut tera)?;
/// ```
fn create_manifest(
    package: &ScoopPackage,
    context: &Context,
    scoop_path: &Path,
    tera: &mut Tera,
) -> Result<PathBuf, String> {
    // Add template to Tera
    let template_name = format!("{}_manifest", package.name);
    if let Err(e) = tera.add_raw_template(&template_name, package.template_content) {
        error!("Failed to add {} template: {}", package.name, e);
        return Err(format!("Failed to add {} template", package.name));
    }

    // Render the template
    let mut rendered_manifest = match tera.render(&template_name, context) {
        Err(e) => {
            error!("Failed to render {} template: {}", package.name, e);
            return Err(format!("Failed to render {} template", package.name));
        }
        Ok(text) => text,
    };

    // Normalize line endings
    rendered_manifest = rendered_manifest.replace("\r\n", "\n").replace("\n", "\r\n");

    // Write manifest to file
    let manifest_path = scoop_path.join(package.manifest_filename);
    if let Err(e) = fs::write(&manifest_path, rendered_manifest) {
        error!("Failed to write {} manifest: {}", package.name, e);
        return Err(format!("Failed to write {} manifest", package.name));
    }

    info!("{} manifest written to: {}", package.name, manifest_path.display());
    Ok(manifest_path)
}

/// Installs a single package using Scoop with retry logic.
///
/// This function attempts to install a package using the Scoop package manager with
/// automatic retry logic. After installation, it verifies the package works correctly
/// by running the test command. If the installation or test fails, it will retry up
/// to the specified maximum number of attempts.
///
/// # Arguments
///
/// * `main_command` - The PowerShell command to use for execution
/// * `manifest_path` - Path to the package manifest file
/// * `package` - The package definition containing test command and metadata
/// * `path_with_scoop` - Environment PATH variable that includes Scoop directories
/// * `max_retries` - Maximum number of installation attempts
///
/// # Returns
///
/// * `Ok(())` - Package installed and tested successfully
/// * `Err(String)` - Error message if installation fails after all retry attempts
///
/// # Examples
///
/// ```rust
/// install_package_with_scoop(
///     "powershell.exe",
///     &manifest_path,
///     &package,
///     &path_with_scoop,
///     3
/// )?;
/// ```
fn install_package_with_scoop(
    main_command: &str,
    manifest_path: &Path,
    package: &ScoopPackage,
    path_with_scoop: &str,
    max_retries: u32,
) -> Result<(), String> {
    for attempt in 1..=max_retries {
        info!("Installing {} (attempt {}/{})", package.name, attempt, max_retries);

        let full_command = format!(
            "scoop install --no-update-scoop '{}'",
            manifest_path.to_str().unwrap()
        );

        // Use the executor directly to ensure proper process handling
        let executor = command_executor::get_executor();
        let output = executor.execute_with_env(
            main_command,
            &vec![
                "-NoProfile",
                "-NonInteractive",
                "-NoLogo",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &full_command,
            ],
            vec![("PATH", &add_to_path(path_with_scoop).unwrap())],
        );

        match output {
            Ok(out) => {
                if out.status.success() {
                    info!("{} installation completed.", package.name);

                    // Test if the package is working correctly
                    if test_package_installation(package.test_command, main_command, path_with_scoop) {
                        info!("{} installed and tested successfully.", package.name);
                        return Ok(());
                    } else {
                        warn!("{} installed but test command '{}' failed on attempt {}",
                              package.name, package.test_command, attempt);

                        if attempt == max_retries {
                            return Err(format!(
                                "Failed to install {}: package installed but test command '{}' failed after {} attempts",
                                package.name, package.test_command, max_retries
                            ));
                        }
                        // Continue to next retry attempt
                    }
                } else {
                    warn!("Installation failed for {} on attempt {}: {:?} | {:?}",
                          package.name, attempt,
                          String::from_utf8_lossy(&out.stdout),
                          String::from_utf8_lossy(&out.stderr));

                    if attempt == max_retries {
                        return Err(format!(
                            "Failed to install {} after {} attempts: {:?} | {:?}",
                            package.name, max_retries,
                            String::from_utf8_lossy(&out.stdout),
                            String::from_utf8_lossy(&out.stderr)
                        ));
                    }
                    // Continue to next retry attempt
                }
            }
            Err(err) => {
                warn!("Command execution failed for {} on attempt {}: {}", package.name, attempt, err);

                if attempt == max_retries {
                    return Err(format!("Failed to install {} after {} attempts: {}", package.name, max_retries, err));
                }
                // Continue to next retry attempt
            }
        }

        // Add a small delay between retries
        if attempt < max_retries {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    unreachable!()
}

/// Tests if a package installation is working correctly by running its test command.
///
/// This function verifies that an installed package is functional by executing
/// the package's test command. It includes retry logic with small delays to handle
/// cases where the file system operations haven't completed immediately after
/// installation.
///
/// # Arguments
///
/// * `test_command` - Command to run to test the package (e.g., "git --version")
/// * `main_command` - The PowerShell command to use for execution
/// * `path_with_scoop` - Environment PATH variable that includes Scoop directories
///
/// # Returns
///
/// * `true` - Test command executed successfully
/// * `false` - Test command failed after all retry attempts
///
/// # Special Cases
///
/// * If `test_command` is "echo 0", the function returns `true` immediately,
///   indicating packages that don't have meaningful test commands
///
/// # Examples
///
/// ```rust
/// let success = test_package_installation("git --version", "powershell.exe", &path_with_scoop);
/// if success {
///     println!("Git is working correctly");
/// }
/// ```
fn test_package_installation(test_command: &str, main_command: &str, path_with_scoop: &str) -> bool {
    if test_command == "echo 0" {
        return true; // Skip test for packages that don't have a meaningful test
    }

    // Add a small delay to allow file system operations to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Try the test command multiple times with short delays
    for attempt in 1..=3 {
        let executor = command_executor::get_executor();
        let test_result = executor.execute_with_env(
            main_command,
            &vec![
                "-NoProfile",
                "-NonInteractive",
                "-NoLogo",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                test_command
            ],
            vec![("PATH", path_with_scoop)],
        );

        match test_result {
            Ok(output) if output.status.success() => return true,
            Ok(_) => {
                debug!("Test command '{}' failed on attempt {}", test_command, attempt);
            }
            Err(e) => {
                debug!("Test command '{}' error on attempt {}: {}", test_command, attempt, e);
            }
        }

        // Short delay between test attempts
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    false
}

/// Sets up and installs all required Scoop packages from offline manifests.
///
/// This function orchestrates the complete installation process for all required
/// packages. It creates Tera templates, renders manifests for each package,
/// and then installs them using Scoop with retry logic. The packages installed
/// include 7zip, Git, Dark (WiX toolset), and Python 3.10.
///
/// # Arguments
///
/// * `scoop_path` - Path to the Scoop directory containing offline packages
/// * `scoop_command` - Path to the Scoop PowerShell script
///
/// # Returns
///
/// * `Ok(())` - All packages installed successfully
/// * `Err(String)` - Error message if any package installation fails
///
/// # Package Installation Order
///
/// The function installs packages in the following order:
/// 1. 7zip - Archive utility
/// 2. Git - Version control system
/// 3. Dark - WiX toolset decompiler
/// 4. Python 3.10 - Python interpreter
///
/// # Examples
///
/// ```rust
/// let scoop_path = Path::new("C:\\offline\\scoop");
/// let scoop_command = Path::new("C:\\Users\\user\\scoop\\shims\\scoop.ps1");
/// setup_scoop_packages(&scoop_path, &scoop_command)?;
/// ```
fn setup_scoop_packages(scoop_path: &Path, scoop_command: &Path) -> Result<(), String> {
    // Create Tera context
    let mut context = Context::new();
    context.insert("offline_archive_scoop_dir", &scoop_path.to_str().unwrap().replace("\\", "/"));

    // Define packages to install with compile-time template content
    let packages = [
        ScoopPackage {
            name: "7zip",
            template_content: include_str!("../../scoop_manifest_templates/7zip.json"),
            manifest_filename: "7zip.json",
            test_command: "echo 0"
        },
        ScoopPackage {
            name: "git",
            template_content: include_str!("../../scoop_manifest_templates/git.json"),
            manifest_filename: "git.json",
            test_command: "git --version",
        },
        ScoopPackage {
            name: "dark",
            template_content: include_str!("../../scoop_manifest_templates/dark.json"),
            manifest_filename: "dark.json",
            test_command: "echo 0"
        },
        ScoopPackage {
            name: "python",
            template_content: include_str!("../../scoop_manifest_templates/python311.json"),
            manifest_filename: "python.json",
            test_command: "python3 --version",
        },
    ];

    // Create all manifests
    let mut tera = Tera::default();
    let mut manifest_paths = Vec::new();

    for package in &packages {
        let manifest_path = create_manifest(package, &context, scoop_path, &mut tera)?;
        manifest_paths.push((manifest_path, package.name));
    }

    // Get Scoop path and determine PowerShell command
    let path_with_scoop = get_scoop_path()
        .ok_or_else(|| "Could not get scoop path".to_string())?;

    let main_command = get_correct_powershell_command();

    // Install all packages with retry logic
    const MAX_RETRIES: u32 = 3;
    for (manifest_path, package_name) in manifest_paths {
        // Find the package struct to get the test command
        let package = packages.iter()
            .find(|p| p.name == package_name)
            .ok_or_else(|| format!("Package {} not found in package definitions", package_name))?;

        install_package_with_scoop(
            &main_command,
            &manifest_path,
            package,
            &path_with_scoop,
            MAX_RETRIES,
        )?;
    }

    Ok(())
}

/// Installs prerequisite software packages from an offline archive.
///
/// This function handles the installation of development prerequisites on different
/// operating systems. On Windows, it installs and configures Scoop package manager
/// and then installs required packages. On Linux and macOS, it simply logs that
/// users should ensure necessary tools are installed manually.
///
/// # Arguments
///
/// * `archive_dir` - Temporary directory containing the offline installation archive
///
/// # Returns
///
/// * `Ok(())` - Prerequisites installed successfully
/// * `Err(String)` - Error message if installation fails
///
/// # Platform Behavior
///
/// * **Windows**: Installs Scoop package manager and required development tools
/// * **Linux/macOS**: Logs informational message about manual tool installation
/// * **Other platforms**: Returns error for unsupported operating systems
///
/// # Windows Installation Process
///
/// 1. Sets up Scoop package manager from offline installer
/// 2. Configures PATH environment variables
/// 3. Installs development packages (Git, Python, 7zip, Dark)
/// 4. Validates package installations with test commands
///
/// # Examples
///
/// ```rust
/// use tempfile::TempDir;
///
/// let archive_dir = TempDir::new().unwrap();
/// install_prerequisites_offline(&archive_dir)?;
/// ```
pub fn install_prerequisites_offline(
    archive_dir: &TempDir,
) -> Result<(), String> {
    match std::env::consts::OS {
        "windows" => {
            // Setup Scoop
            let scoop_path = archive_dir.path().join("scoop");
            let scoop_install_path = dirs::home_dir()
                .unwrap()
                .join("scoop");
            let scoop_command = scoop_install_path.join("shims").join("scoop.ps1");
            add_to_path(&scoop_install_path.to_str().unwrap());
            add_to_path(&scoop_install_path.join("shims").to_str().unwrap());
            match execute_command(
                "powershell",
                &[
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &scoop_path.join("install_scoop_offline.ps1").to_str().unwrap(),
                    "-OfflineDir",
                    &scoop_path.to_str().unwrap(),
                ],
            ) {
                Ok(out) => {
                    if out.status.success() {
                        info!("Scoop installed successfully.");
                        info!("Scoop install path: {}", scoop_install_path.display());
                        add_path_to_path(&scoop_install_path.to_str().unwrap());
                    } else {
                        warn!(
                            "Failed to install Scoop: {:?} | {:?}",
                            String::from_utf8_lossy(&out.stdout),
                            String::from_utf8_lossy(&out.stderr)
                        );
                    }
                }
                Err(err) => {
                    return Err(format!("Failed to install Scoop: {}", err));
                }
            }
            add_to_path(&scoop_install_path.to_str().unwrap());
            add_to_path(&scoop_install_path.join("shims").to_str().unwrap());

            // TODO: disable auto-updates in scoop config.json
            setup_scoop_packages(&scoop_path, &scoop_command)?;
        }
        "linux" | "macos" => {
            info!("On POSIX system, we do not provide prerequisites. Please ensure you have the necessary tools installed.");
        }
        _ => {
            return Err(format!(
                "Unsupported OS: {}",
                std::env::consts::OS
            ));
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
)-> Result<(), String> {
  let mut everything_copied = true;
  for archive_version in config.clone().idf_versions.unwrap() {
    match std::env::consts::OS {
      "windows" => {
        // As on windows the IDF contains too long paths, we need to copy the content from the offline archive to the IDF path
        // using windows powershell command
        let mut main_command = get_correct_powershell_command();

        let output_cp = command_executor::execute_command(
            &main_command,
            &vec![
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "cp",
                "-r",
                &archive_dir.path().join(&archive_version).to_str().unwrap(),
                &config.clone().path.unwrap().join(&archive_version).to_str().unwrap(),
            ],
        );
        match output_cp {
            Ok(out) => {
                if out.status.success() {
                    info!("Successfully copied content from offline archive to IDF path");
                } else {
                    error!("Failed to copy content from offline archive: {:?} | {:?}", out.stdout, out.stderr);
                    everything_copied = false;
                }
            }
            Err(err) => {
                error!("Failed to copy content from offline archive: {}", err);
                everything_copied = false;
            }
        }

      },
      _ => {
        debug!("Copying IDF version: {}", archive_version);
        match copy_dir_contents(&archive_dir.path().join(&archive_version), &config.clone().path.unwrap().join(&archive_version)) {
          Ok(_) => {
            info!("Successfully copied IDF version: {}", archive_version);
          }
          Err(err) => {
            error!("Failed to copy IDF version {}: {}", archive_version, err);
            everything_copied = false;
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
