use std::{fs, path::{Path, PathBuf}};

use log::{debug, error, info, warn};
use tempfile::TempDir;
use tera::{Context, Tera};

use crate::{add_path_to_path, command_executor::{self, execute_command}, settings::Settings, system_dependencies::{add_to_path, get_correct_powershell_command, get_scoop_path}, utils::copy_dir_contents};

// Structure to define package information with compile-time template content
pub struct ScoopPackage {
    pub name: &'static str,
    pub template_content: &'static str,
    pub manifest_filename: &'static str,
    pub test_command: &'static str,
}

// Helper function to create and write a manifest
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

// Helper function to install a single package with retry logic
fn install_package_with_scoop(
    main_command: &str,
    scoop_command: &Path,
    manifest_path: &Path,
    package: &ScoopPackage,
    path_with_scoop: &str,
    max_retries: u32,
) -> Result<(), String> {
    for attempt in 1..=max_retries {
        info!("Installing {} (attempt {}/{})", package.name, attempt, max_retries);

        // Build the complete PowerShell command as a single string to avoid shell spawning
        // let full_command = &format!("Start-Process -FilePath '{}' -ArgumentList 'install', '--no-update-scoop', '{}' -Wait -NoNewWindow", scoop_command.to_str().unwrap(), manifest_path.to_str().unwrap());
        let full_command = format!(
            "scoop install --no-update-scoop '{}'",
            // scoop_command.to_str().unwrap(),
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

// Helper function to test if a package is working
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

// Main function that creates manifests and installs packages
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
            template_content: include_str!("../../scoop_manifest_templates/python310.json"),
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
            scoop_command,
            &manifest_path,
            package,
            &path_with_scoop,
            MAX_RETRIES,
        )?;
    }

    Ok(())
}

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
