use std::{env, fs};
use anyhow::{anyhow, Result, Context};

use log::{debug, trace, warn};

use crate::{command_executor, utils::find_by_name_and_extension};

/// Determines the package manager installed on the system.
///
/// This function attempts to identify the package manager by executing each
/// listed package manager's version command and checking if the command
/// execution is successful.
///
/// This should be only executed on Linux systems, as package managers on other operating systems
/// are not supported.
///
/// # Returns
///
/// * `Some(&'static str)` - If a package manager is found, returns the name of the package manager.
/// * `None` - If no package manager is found, returns None.
fn determine_package_manager() -> Option<&'static str> {
    let package_managers = vec!["apt", "dpkg", "dnf", "pacman", "zypper"];

    for manager in package_managers {
        let output = command_executor::execute_command(manager, &["--version"]);
        match output {
            Ok(output) => {
                if output.status.success() {
                    return Some(manager);
                }
            }
            Err(_) => continue,
        }
    }

    None
}

/// Returns a hardcoded vector of required tools based on the operating system.
///
/// # Returns
///
/// * `Vec<&'static str>` - A vector of required tools for the current operating system.
pub fn get_prequisites() -> Vec<&'static str> {
    match std::env::consts::OS {
        "linux" => vec![
            "wget",
            "flex",
            "bison",
            "gperf",
            "ccache",
            "dfu-util",
        ],
        "windows" => vec![],
        "macos" => vec!["dfu-util"],
        _ => vec![],
    }
}

/// Returns a list of additional system-level prerequisites (development libraries)
/// required for certain functionalities, based on the detected operating system
/// and, for Linux, the package manager in use.
///
/// These prerequisites are typically needed for compiling or running applications
/// that depend on native libraries like `libffi`, `libusb`, `OpenSSL`, and
/// libraries required for QEMU (e.g., `libgcrypt`, `glib`, `pixman`, `sdl2`, `libslirp`).
///
/// # Returns
///
/// A `Vec<&'static str>` containing the names of the packages to be installed.
/// An empty `Vec` is returned if the operating system is not Linux (unless it's macOS with specific QEMU needs),
/// or if the Linux package manager is not recognized or does not have specific prerequisites
/// defined by this function.
///
/// # Linux Package Manager Mappings:
///
/// - **apt (Debian/Ubuntu):**
///   - `libffi-dev`: Development headers for Foreign Function Interface.
///   - `libusb-1.0-0`: Runtime library for USB device access.
///   - `libssl-dev`: Development headers for OpenSSL (SSL/TLS cryptography).
///   - `libgcrypt20`: Runtime library for cryptographic functions (QEMU dependency).
///   - `libglib2.0-0`: Runtime library for GLib (QEMU dependency).
///   - `libpixman-1-0`: Runtime library for pixman (QEMU dependency).
///   - `libsdl2-2.0-0`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp0`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// - **dpkg (Debian/Ubuntu fallback):**
///   - Same as `apt`.
///
/// - **dnf (Fedora/RHEL/CentOS):**
///   - `libffi-devel`: Development headers for Foreign Function Interface.
///   - `libusb`: Runtime library for USB device access.
///   - `openssl-devel`: Development headers for OpenSSL.
///   - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
///   - `glib2`: Runtime library for GLib (QEMU dependency).
///   - `pixman`: Runtime library for pixman (QEMU dependency).
///   - `SDL2`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// - **pacman (Arch Linux):**
///   - `libusb`: Includes both runtime and development files for USB access.
///   - `libffi`: Includes both runtime and development files for Foreign Function Interface.
///   - `openssl`: Includes both runtime and development files for OpenSSL.
///   - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
///   - `glib`: Runtime library for GLib (QEMU dependency).
///   - `pixman`: Runtime library for pixman (QEMU dependency).
///   - `sdl2`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// - **zypper (openSUSE/SUSE Linux Enterprise):**
///   - `libusb-1_0-0`: Runtime library for USB device access.
///   - `libffi-devel`: Development headers for Foreign Function Interface.
///   - `libopenssl-devel`: Development headers for OpenSSL.
///   - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
///   - `glib2`: Runtime library for GLib (QEMU dependency).
///   - `pixman-1`: Runtime library for pixman (QEMU dependency).
///   - `libsdl2-2_0_0`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// # macOS Prerequisites:
/// - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
/// - `glib`: Runtime library for GLib (QEMU dependency).
/// - `pixman`: Runtime library for pixman (QEMU dependency).
/// - `sdl2`: Runtime library for SDL2 (QEMU dependency).
/// - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// For other operating systems (Windows) or unrecognized Linux package managers,
/// an empty vector is returned.
pub fn get_additional_prerequisites_based_on_package_manager() -> Vec<&'static str> {
    match std::env::consts::OS {
        "linux" => match determine_package_manager() {
            Some("apt") => vec!["libffi-dev", "libusb-1.0-0", "libssl-dev", "libgcrypt20", "libglib2.0-0", "libpixman-1-0", "libsdl2-2.0-0", "libslirp0"],
            Some("dpkg") => vec!["libffi-dev", "libusb-1.0-0", "libssl-dev", "libgcrypt20", "libglib2.0-0", "libpixman-1-0", "libsdl2-2.0-0", "libslirp0"],
            Some("dnf") => vec!["libffi-devel", "libusb", "openssl-devel", "libgcrypt", "glib2", "pixman", "SDL2", "libslirp"],
            Some("pacman") => vec!["libusb", "libffi", "openssl", "libgcrypt", "glib", "pixman", "sdl2", "libslirp"],
            Some("zypper") => vec!["libusb-1_0-0", "libffi-devel", "libopenssl-devel", "libgcrypt", "glib2", "pixman-1", "libsdl2-2_0_0", "libslirp"],
            _ => vec![],
        },
        "windows" => vec![],
        "macos" => vec!["libgcrypt", "glib", "pixman", "sdl2", "libslirp"],
        _ => vec![],
    }
}

/// Checks the system for the required tools and returns a list of unsatisfied tools.
///
/// This function determines the operating system and package manager, then checks if each required tool is installed.
/// If a tool is not found, it is added to the `unsatisfied` vector and returned.
/// The prerequsites are met when empty vector is returned.
///
/// # Returns
///
/// * `Ok(Vec<&'static str>)` - If the function completes successfully, returns a vector of unsatisfied tools.
/// * `Err(String)` - If an error occurs, returns an error message.
pub fn check_prerequisites() -> Result<Vec<&'static str>, String> {
    let mut list_of_required_tools = get_prequisites();
    list_of_required_tools = [list_of_required_tools, get_additional_prerequisites_based_on_package_manager()].concat();
    debug!("Checking for prerequisites...");
    debug!("will be checking for : {:?}", list_of_required_tools);
    let mut unsatisfied = vec![];
    match std::env::consts::OS {
        "linux" => {
            // git needs to be checked separately
            let output = command_executor::execute_command("git", &["--version"]);
            match output {
                Ok(output) => {
                    if output.status.success() {
                        debug!("git is already installed: {:?}", output);
                        list_of_required_tools.retain(|&tool| tool != "git");
                    } else {
                        debug!("check for git failed: {:?}", output);
                        unsatisfied.push("git");
                    }
                }
                Err(_e) => {
                    unsatisfied.push("git");
                }
            };
            // check with which command
            list_of_required_tools.retain(|&tool| {
                match command_executor::execute_command("which", &["-a", tool]) {
                    Ok(o) if o.status.success() => {
                        debug!("{} is already installed: {:?}", tool, o);
                        false // Tool found, so remove it from the list (don't retain).
                    }
                    Ok(o) => {
                        debug!("'which' check for {} failed: {:?}", tool, o);
                        true
                    }
                    Err(e) => {
                        debug!("'which' check for {} failed with error: {:?}", tool, e);
                        true
                    }
                }
            });
            // now check if the tools are installed with the package manager
            let package_manager = determine_package_manager();
            debug!("Detected package manager: {:?}", package_manager);
            match package_manager {
                Some("apt") => {
                    for tool in list_of_required_tools {
                        let output = command_executor::execute_command(
                            "sh",
                            &["-c", &format!("apt list --installed | grep {}", tool)],
                        );
                        match output {
                            Ok(o) => {
                                if o.status.success() {
                                    debug!("{} is already installed: {:?}", tool, o);
                                } else {
                                    debug!("check for {} failed: {:?}", tool, o);
                                    unsatisfied.push(tool);
                                }
                            }
                            Err(_e) => {
                                unsatisfied.push(tool);
                            }
                        }
                    }
                }
                Some("dpkg") => {
                    for tool in list_of_required_tools {
                        let output = command_executor::execute_command(
                            "sh",
                            &["-c", &format!("dpkg -l | grep {}", tool)],
                        );
                        match output {
                            Ok(o) => {
                                if o.status.success() {
                                    debug!("{} is already installed: {:?}", tool, o);
                                } else {
                                    debug!("check for {} failed: {:?}", tool, o);
                                    unsatisfied.push(tool);
                                }
                            }
                            Err(_e) => {
                                unsatisfied.push(tool);
                            }
                        }
                    }
                }
                Some("dnf") => {
                    for tool in list_of_required_tools {
                        let output = command_executor::execute_command(
                            "sh",
                            &["-c", &format!("rpm -qa | grep -i {}", tool)],
                        );
                        match output {
                            Ok(o) => {
                                if o.status.success() {
                                    debug!("{} is already installed: {:?}", tool, o);
                                } else {
                                    unsatisfied.push(tool);
                                }
                            }
                            Err(_e) => {
                                unsatisfied.push(tool);
                            }
                        }
                    }
                }
                Some("pacman") => {
                    for tool in list_of_required_tools {
                        let output = command_executor::execute_command(
                            "sh",
                            &["-c", &format!("pacman -Qs | grep {}", tool)],
                        );
                        match output {
                            Ok(o) => {
                                if o.status.success() {
                                    debug!("{} is already installed: {:?}", tool, o);
                                } else {
                                    unsatisfied.push(tool);
                                }
                            }
                            Err(_e) => {
                                unsatisfied.push(tool);
                            }
                        }
                    }
                }
                Some("zypper") => {
                    for tool in list_of_required_tools {
                        let output = command_executor::execute_command(
                            "sh",
                            &["-c", &format!("zypper se --installed-only {}", tool)],
                        );
                        match output {
                            Ok(o) => {
                                if o.status.success() {
                                    debug!("{} is already installed: {:?}", tool, o);
                                } else {
                                    unsatisfied.push(tool);
                                }
                            }
                            Err(_e) => {
                                unsatisfied.push(tool);
                            }
                        }
                    }
                }
                None => {
                    return Err(format!(
                        "Unsupported package manager - {}",
                        package_manager.unwrap()
                    ));
                }
                _ => {
                    return Err(format!(
                        "Unsupported package manager - {}",
                        package_manager.unwrap()
                    ));
                }
            }
        }
        "macos" => {
            for tool in list_of_required_tools {
                let output = command_executor::execute_command(
                    "zsh",
                    &["-c", &format!("which {}", tool)],
                );
                match output {
                    Ok(o) => {
                        if o.status.success() {
                            debug!("{} is already installed: {:?}", tool, o);
                        } else {
                            debug!("check for {} failed: {:?}", tool, o);
                            // check if the tool is installed with brew
                            let output = command_executor::execute_command(
                                "brew",
                                &["list", tool],
                            );
                            match output {
                                Ok(o) => {
                                    if o.status.success() {
                                        debug!("{} is already installed with brew", tool);
                                    } else {
                                        unsatisfied.push(tool);
                                    }
                                }
                                Err(_e) => {
                                    unsatisfied.push(tool);
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        unsatisfied.push(tool);
                    }
                }
            }
        }
        "windows" => {
            for tool in list_of_required_tools {
                let output = command_executor::execute_command(
                    "powershell",
                    &["-Command", &format!("{} --version", tool)],
                );
                match output {
                    Ok(o) => {
                        if o.status.success() {
                            debug!("{} is already installed: {:?}", tool, o);
                        } else {
                            debug!("check for {} failed: {:?}", tool, o);
                            unsatisfied.push(tool);
                        }
                    }
                    Err(_e) => {
                        unsatisfied.push(tool);
                    }
                }
            }
        }
        _ => {
            return Err(format!("Unsupported OS - {}", std::env::consts::OS));
        }
    }
    Ok(unsatisfied)
}

/// Returns the path to the Scoop shims directory.
/// This function is only relevant for Windows systems.
///
/// # Returns
///
/// * `Some(String)` - If the function is executed on a Windows system and the Scoop shims directory is found,
///   the function returns the path to the Scoop shims directory.
/// * `None` - If the function is executed on a non-Windows system or if the Scoop shims directory cannot be found,
///   the function returns None.
pub fn get_scoop_path() -> Option<String> {
    if std::env::consts::OS == "windows" {
        let home_dir = match dirs::home_dir() {
            Some(d) => d,
            None => {
                debug!("Could not get home directory");
                return None;
            }
        };
        let scoop_shims_path = home_dir.join("scoop").join("shims");
        Some(scoop_shims_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Installs the Scoop package manager on Windows.
///
/// This function is only relevant for Windows systems. It sets the execution policy to RemoteSigned,
/// downloads the Scoop installer script from the official website, and executes it.
///
/// # Returns
///
/// * `Ok(())` - If the Scoop package manager is successfully installed.
/// * `Err(String)` - If an error occurs during the installation process.
fn install_scoop_package_manager() -> Result<(), String> {
    match std::env::consts::OS {
        "windows" => {
            let path_with_scoop = match get_scoop_path() {
                Some(s) => s,
                None => {
                    debug!("Could not get scoop path");
                    return Err(String::from("Could not get scoop path"));
                }
            };
            add_to_path(&path_with_scoop).unwrap();
            let scoop_install_cmd = include_str!("../../powershell_scripts/install_scoop.ps1");
            let output = crate::run_powershell_script(scoop_install_cmd);

            match output {
                Ok(o) => {
                    trace!("output: {}", o);
                    debug!("Successfully installed Scoop package manager. Adding to PATH");
                    add_to_path(&path_with_scoop).unwrap();
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            }
        }
        _ => {
            // this function should not be called on non-windows platforms
            debug!("Scoop package manager is only supported on Windows. Skipping installation.");
            Err(format!("Unsupported OS - {}", std::env::consts::OS))
        }
    }
}

/// Ensures that the Scoop package manager is installed on Windows.
///
/// This function checks if the Scoop package manager is installed on the system.
/// If it is not installed, the function installs it by setting the execution policy to RemoteSigned,
/// downloading the Scoop installer script from the official website, and executing it.
///
/// # Returns
///
/// * `Ok(())` - If the Scoop package manager is successfully installed.
/// * `Err(String)` - If an error occurs during the installation process.
pub fn ensure_scoop_package_manager() -> Result<(), String> {
    match std::env::consts::OS {
        "windows" => {
            let path_with_scoop = match get_scoop_path() {
                Some(s) => s,
                None => {
                    debug!("Could not get scoop path");
                    return Err(String::from("Could not get scoop path"));
                }
            };
            // #[cfg(windows)]
            // crate::win_tools::add_to_win_path(&path_with_scoop).unwrap();
            // add_to_windows_path(&path_with_scoop).unwrap();
            add_to_path(&path_with_scoop).unwrap();
            let output = command_executor::execute_command(
                "powershell",
                &["-Command", "scoop", "--version"],
            );
            match output {
                Ok(o) => {
                    if o.status.success() {
                        debug!("Scoop package manager is already installed");
                        Ok(())
                    } else {
                        debug!("Installing scoop package manager");
                        install_scoop_package_manager()
                    }
                }
                Err(_) => install_scoop_package_manager(),
            }
        }
        _ => {
            // this function should not be called on non-windows platforms
            debug!("Scoop package manager is only supported on Windows. Skipping installation.");
            Err(format!("Unsupported OS - {}", std::env::consts::OS))
        }
    }
}

/// Installs the required packages based on the operating system.
/// This function actually panics if the required packages install fail.
/// This is to ensure that user actually sees the error and realize which package failed to install.
///
/// # Parameters
///
/// * `packages_list` - A vector of strings representing the names of the packages to be installed.
///   this can be obtained by calling the check_prerequisites() function.
///
/// # Returns
///
/// * `Ok(())` - If the packages are successfully installed.
/// * `Err(String)` - If an error occurs during the installation process.
pub fn install_prerequisites(packages_list: Vec<String>) -> Result<(), String> {
    match std::env::consts::OS {
        "linux" => {
            let package_manager = determine_package_manager();
            match package_manager {
                Some("apt") => {
                    for package in packages_list {
                        let output = command_executor::execute_command(
                            "sudo",
                            &["apt", "install", "-y", &package],
                        );
                        match output {
                            Ok(_) => {
                                debug!("Successfully installed {}", package);
                            }
                            Err(e) => panic!("Failed to install {}: {}", package, e),
                        }
                    }
                }
                Some("dnf") => {
                    for package in packages_list {
                        let output = command_executor::execute_command(
                            "sudo",
                            &["dnf", "install", "-y", &package],
                        );
                        match output {
                            Ok(_) => {
                                debug!("Successfully installed {}", package);
                            }
                            Err(e) => panic!("Failed to install {}: {}", package, e),
                        }
                    }
                }
                Some("pacman") => {
                    for package in packages_list {
                        let output = command_executor::execute_command(
                            "sudo",
                            &["pacman", "-S", "--noconfirm", &package],
                        );
                        match output {
                            Ok(_) => {
                                debug!("Successfully installed {}", package);
                            }
                            Err(e) => panic!("Failed to install {}: {}", package, e),
                        }
                    }
                }
                Some("zypper") => {
                    for package in packages_list {
                        let output = command_executor::execute_command(
                            "sudo",
                            &["zypper", "install", "-y", &package],
                        );
                        match output {
                            Ok(_) => {
                                debug!("Successfully installed {}", package);
                            }
                            Err(e) => panic!("Failed to install {}: {}", package, e),
                        }
                    }
                }
                _ => {
                    return Err(format!(
                        "Unsupported package manager - {}",
                        package_manager.unwrap()
                    ));
                }
            }
        }
        "macos" => {
            for package in packages_list {
                let output = command_executor::execute_command("brew", &["install", &package]);
                match output {
                    Ok(_) => {
                        debug!("Successfully installed {}", package);
                    }
                    Err(e) => panic!("Failed to install {}: {}", package, e),
                }
            }
        }
        "windows" => {
            ensure_scoop_package_manager()?;
            for package in packages_list {
                let path_with_scoop = match get_scoop_path() {
                    Some(s) => s,
                    None => {
                        debug!("Could not get scoop path");
                        return Err(String::from("Could not get scoop path"));
                    }
                };
                debug!("Installing {} with scoop: {}", package, path_with_scoop);
                let mut main_command = "powershell";

                let test_for_pwsh = command_executor::execute_command("pwsh", &["--version"]);
                match test_for_pwsh {
                    // this needs to be used in powershell 7
                    Ok(_) => {
                        debug!("Found powershell core");
                        main_command = "pwsh";
                    }
                    Err(_) => {
                        debug!("Powershell core not found, using powershell");
                    }
                }

                let output = command_executor::execute_command_with_env(
                    main_command,
                    &vec![
                        "-ExecutionPolicy",
                        "Bypass",
                        "-Command",
                        "scoop",
                        "install",
                        &package,
                    ],
                    vec![("PATH", &add_to_path(&path_with_scoop).unwrap())],
                );
                match output {
                    Ok(o) => {
                        if o.status.success() {
                            trace!("{}", String::from_utf8(o.stdout).unwrap());
                            debug!("Successfully installed {:?}", package);
                        } else {
                            let output = String::from_utf8(o.stdout).unwrap();
                            let error_message = String::from_utf8(o.stderr).unwrap();
                            debug!("Failed to install {}: {}", package, error_message);
                            debug!("Output: {}", output);
                        }
                    }
                    Err(e) => panic!("Failed to install {}: {}", package, e),
                }
            }
        }
        _ => {
            return Err(format!("Unsupported OS - {}", std::env::consts::OS));
        }
    }
    Ok(())
}

/// Adds a new directory to the system's PATH environment variable.
///
/// This function appends the new directory to the current PATH if it's not already present.
/// On Windows systems, it also updates the user's PATH environment variable persistently.
///
/// # Parameters
///
/// * `new_path` - A string slice representing the new directory path to be added to the PATH.
///
/// # Returns
///
/// * `Ok(String)` - Returns the updated PATH string if the operation is successful.
/// * `Err(std::io::Error)` - Returns an IO error if the PATH update fails on Windows systems.
fn add_to_path(new_path: &str) -> Result<String, std::io::Error> {
    let binding = env::var_os("PATH").unwrap_or_default();
    let paths = binding.to_str().unwrap();

    let new_path_string = match std::env::consts::OS {
        "windows" => format!("{};{}", new_path, paths),
        _ => format!("{}:{}", new_path, paths),
    };
    if !paths.contains(new_path) {
        // Update current process PATH
        env::set_var("PATH", &new_path_string);
    }
    if std::env::consts::OS == "windows" {
        // PowerShell 7+ compatible command
        let ps_command = format!(
            "$oldPath = [Environment]::GetEnvironmentVariable('PATH', 'User'); \
               if (-not $oldPath.Contains('{}')) {{ \
                   $newPath = '{}' + ';' + $oldPath; \
                   [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User'); \
               }}",
            new_path.replace("'", "''"),
            new_path.replace("'", "''")
        );

        let res = command_executor::execute_command(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", &ps_command],
        );

        match res {
            Ok(_) => {
                debug!("Added {} to PATH", new_path);
            }
            Err(e) => {
                warn!("Failed to add {} to PATH: {}", new_path, e);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to update PATH: {}", e),
                ));
            }
        }
    }

    Ok(new_path_string)
}

/// Copies the 60-openocd.rules file to /etc/udev/rules.d/ on Linux.
///
/// This function checks if the rules file already exists. If not, it attempts
/// to find it within the provided `tools_path` and copy it.
///
/// # Arguments
/// * `tools_path` - The path where tool-related files might be located,
///                  including the openocd rules file.
///
/// # Returns
/// A `Result` indicating success (`Ok(())`) or an `anyhow::Error` if
/// an error occurs during file operations or if the file is not found.
pub fn copy_openocd_rules(tools_path: &str) -> Result<()> {
  let openocd_rules_path = match std::env::consts::OS {
    "linux" => "/etc/udev/rules.d/60-openocd.rules",
    _ => return Ok(()),
  };
  let openocd_rules_path = std::path::Path::new(openocd_rules_path);
  if openocd_rules_path.exists() {
    debug!("openocd rules file already exists");
    return Ok(());
  }

  let tools_path = std::path::Path::new(tools_path);

  let found_files = find_by_name_and_extension(tools_path, "60-openocd", "rules");

  let openocd_rules_source = found_files.first().ok_or_else(|| {
    anyhow!(
      "60-openocd.rules file not found in {}",
      tools_path.display()
    )
  })?;
  fs::copy(openocd_rules_source, openocd_rules_path).with_context(|| {
    format!(
      "Failed to copy {} to {} . Make sure you have the necessary permissions. Now you can copy it manually.",
      openocd_rules_source,
      openocd_rules_path.display()
    )
  })?;

  Ok(())
}
