use std::{collections::HashSet, env, fs};
use anyhow::{anyhow, Result, Context};

use log::{debug, trace, warn};

use crate::{command_executor, utils::find_by_name_and_extension};

pub const PYTHON_NAME_TO_INSTALL: &str = "python313";

/// Result of a prerequisites check operation.
///
/// This struct provides detailed information about the outcome of checking
/// system prerequisites, including both the list of missing tools and whether
/// the system was able to run verification commands at all.
#[derive(Debug, Clone)]
pub struct PrerequisitesCheckResult {
    /// List of missing prerequisites that need to be installed.
    pub missing: Vec<&'static str>,
    /// Whether the system can actually run verification commands.
    /// If false, the system may be unable to execute shell commands,
    /// and the `missing` list may be unreliable.
    pub can_verify: bool,
    /// Whether the basic shell execution failed on this system.
    pub shell_failed: bool,
}

impl PrerequisitesCheckResult {
    /// Creates a new PrerequisitesCheckResult with the given missing tools and verification status.
    pub fn new(missing: Vec<&'static str>, can_verify: bool, shell_failed: bool) -> Self {
        Self { missing, can_verify, shell_failed }
    }

    /// Returns true if all prerequisites are satisfied and verification was successful.
    pub fn is_satisfied(&self) -> bool {
        self.can_verify && self.missing.is_empty()
    }
}

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

/// Verifies that basic shell execution works on the current system.
///
/// This function tests whether the system can execute simple shell commands,
/// which is a prerequisite for checking other system dependencies.
///
/// # Platform-specific behavior:
/// - **Linux**: Executes `sh -c "echo test"`
/// - **macOS**: Executes `zsh -c "echo test"`
/// - **Windows**: Executes `cmd /c echo test`
///
/// # Returns
///
/// * `true` - If shell execution works correctly
/// * `false` - If shell execution fails or the OS is unsupported
pub fn verify_shell_execution() -> bool {
    let result = match std::env::consts::OS {
        "linux" => {
            command_executor::execute_command("sh", &["-c", "echo test"])
        }
        "macos" => {
            command_executor::execute_command("zsh", &["-c", "echo test"])
        }
        "windows" => {
            command_executor::execute_command("cmd", &["/c", "echo", "test"])
        }
        _ => {
            debug!("Unsupported OS for shell verification: {}", std::env::consts::OS);
            return false;
        }
    };

    match result {
        Ok(output) => {
            if output.status.success() {
                debug!("Shell execution verification succeeded");
                true
            } else {
                debug!("Shell execution verification failed with non-zero exit code");
                false
            }
        }
        Err(e) => {
            debug!("Shell execution verification failed with error: {:?}", e);
            false
        }
    }
}

/// Returns a hardcoded vector of required tools based on the operating system.
///
/// # Returns
///
/// * `Vec<&'static str>` - A vector of required tools for the current operating system.
pub fn get_prequisites() -> Vec<&'static str> {
    match std::env::consts::OS {
        "linux" => vec![
            "git",
            "wget",
            "flex",
            "bison",
            "gperf",
            "ccache",
            "dfu-util",
            "cmake",
        ],
        "windows" => vec!["git"],
        "macos" => vec!["dfu-util","cmake"],
        _ => vec![],
    }
}

/// Returns a list of system-level prerequisites (development libraries)
/// required for general functionalities, based on the detected operating system
/// and, for Linux, the package manager in use.
///
/// These prerequisites are typically needed for compiling or running applications
/// that depend on native libraries like `libffi`, `libusb`, and `OpenSSL`.
///
/// # Returns
///
/// A `Vec<&'static str>` containing the names of the packages to be installed.
/// An empty `Vec` is returned if the operating system is not Linux,
/// or if the Linux package manager is not recognized.
///
/// # Linux Package Manager Mappings:
///
/// - **apt (Debian/Ubuntu):**
///   - `libffi-dev`: Development headers for Foreign Function Interface.
///   - `libusb-1.0-0`: Runtime library for USB device access.
///   - `libssl-dev`: Development headers for OpenSSL (SSL/TLS cryptography).
///
/// - **dpkg (Debian/Ubuntu fallback):**
///   - Same as `apt`.
///
/// - **dnf (Fedora/RHEL/CentOS):**
///   - `libffi-devel`: Development headers for Foreign Function Interface.
///   - `libusb`: Runtime library for USB device access.
///   - `openssl-devel`: Development headers for OpenSSL.
///
/// - **pacman (Arch Linux):**
///   - `libusb`: Includes both runtime and development files for USB access.
///   - `libffi`: Includes both runtime and development files for Foreign Function Interface.
///   - `openssl`: Includes both runtime and development files for OpenSSL.
///
/// - **zypper (openSUSE/SUSE Linux Enterprise):**
///   - `libusb-1_0-0`: Runtime library for USB device access.
///   - `libffi-devel`: Development headers for Foreign Function Interface.
///   - `libopenssl-devel`: Development headers for OpenSSL.
///
/// For other operating systems (Windows, macOS) or unrecognized Linux package managers,
/// an empty vector is returned.
pub fn get_general_prerequisites_based_on_package_manager() -> Vec<&'static str> {
    match std::env::consts::OS {
        "linux" => match determine_package_manager() {
            Some("apt") => vec!["libffi-dev", "libusb-1.0-0", "libssl-dev"],
            Some("dpkg") => vec!["libffi-dev", "libusb-1.0-0", "libssl-dev"],
            Some("dnf") => vec!["libffi-devel", "libusb", "openssl-devel"],
            Some("pacman") => vec!["libusb", "libffi", "openssl"],
            Some("zypper") => vec!["libusb-1_0-0", "libffi-devel", "libopenssl-devel"],
            _ => vec![],
        },
        _ => vec![],
    }
}

/// Returns a list of QEMU-specific system-level prerequisites (development libraries)
/// required for running QEMU, based on the detected operating system
/// and, for Linux, the package manager in use.
///
/// These prerequisites include libraries required for QEMU such as
/// `libgcrypt`, `glib`, `pixman`, `sdl2`, and `libslirp`.
///
/// # Returns
///
/// A `Vec<&'static str>` containing the names of the packages to be installed.
/// An empty `Vec` is returned if the operating system is Windows,
/// or if the Linux package manager is not recognized.
///
/// # Linux Package Manager Mappings:
///
/// - **apt (Debian/Ubuntu):**
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
///   - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
///   - `glib2`: Runtime library for GLib (QEMU dependency).
///   - `pixman`: Runtime library for pixman (QEMU dependency).
///   - `SDL2`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// - **pacman (Arch Linux):**
///   - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
///   - `glib`: Runtime library for GLib (QEMU dependency).
///   - `pixman`: Runtime library for pixman (QEMU dependency).
///   - `sdl2`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// - **zypper (openSUSE/SUSE Linux Enterprise):**
///   - `libgcrypt20`: Runtime library for cryptographic functions (QEMU dependency).
///   - `glib2`: Runtime library for GLib (QEMU dependency).
///   - `pixman-1`: Runtime library for pixman (QEMU dependency).
///   - `libSDL2-2_0-0`: Runtime library for SDL2 (QEMU dependency).
///   - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// # macOS Prerequisites:
/// - `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
/// - `glib`: Runtime library for GLib (QEMU dependency).
/// - `pixman`: Runtime library for pixman (QEMU dependency).
/// - `sdl2`: Runtime library for SDL2 (QEMU dependency).
/// - `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
///
/// For Windows or unrecognized Linux package managers, an empty vector is returned.
pub fn get_qemu_prerequisites_based_on_package_manager() -> Vec<&'static str> {
    match std::env::consts::OS {
        "linux" => match determine_package_manager() {
            Some("apt") => vec!["libgcrypt20", "libglib2.0-0", "libpixman-1-0", "libsdl2-2.0-0", "libslirp0"],
            Some("dpkg") => vec!["libgcrypt20", "libglib2.0-0", "libpixman-1-0", "libsdl2-2.0-0", "libslirp0"],
            Some("dnf") => vec!["libgcrypt", "glib2", "pixman", "SDL2", "libslirp"],
            Some("pacman") => vec!["libgcrypt", "glib", "pixman", "sdl2", "libslirp"],
            Some("zypper") => vec!["libgcrypt20", "glib2", "pixman-1", "libSDL2-2_0-0", "libslirp"],
            _ => vec![],
        },
        "macos" => vec!["libgcrypt", "glib", "pixman", "sdl2", "libslirp"],
        _ => vec![],
    }
}

/// Checks if the given list of tools/packages are installed on the system.
///
/// This is a helper function that contains the core checking logic for different
/// operating systems and package managers.
///
/// # Arguments
///
/// * `tools` - A vector of tool/package names to check
///
/// # Returns
///
/// * `Ok(Vec<&'static str>)` - Vector of unsatisfied tools/packages
/// * `Err(String)` - If an error occurs, returns an error message
fn check_tools_installed(tools: Vec<&'static str>) -> Result<Vec<&'static str>, String> {
    let mut list_of_required_tools = tools;
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
                let current_path = std::env::var("PATH").unwrap_or_default();
                let system_path = format!("{};{}", get_scoop_path().unwrap(), current_path);
                let output = command_executor::execute_command_with_env(
                    "powershell",
                    &vec!["-Command", &format!("{} --version", tool)],
                    vec![("PATH", &system_path)],
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

/// Checks the system for the required tools and returns a detailed result.
///
/// This function determines the operating system and package manager, then checks if each required tool is installed.
/// If a tool is not found, it is added to the `missing` list in the result.
///
/// When an error occurs during prerequisite checking, this function will verify if basic shell
/// execution works. If shell verification also fails, the result will indicate that the system
/// cannot verify prerequisites.
///
/// # Returns
///
/// * `Ok(PrerequisitesCheckResult)` - A result containing the list of missing prerequisites
///   and whether verification was possible.
/// * `Err(String)` - If a critical error occurs that prevents any checking.
///   The error message will mention `--skip-prerequisites-check` if shell verification fails.
pub fn check_prerequisites_with_result() -> Result<PrerequisitesCheckResult, String> {
    let mut list_of_required_tools = get_prequisites();
    list_of_required_tools = [list_of_required_tools, get_general_prerequisites_based_on_package_manager()].concat();
    debug!("Checking for prerequisites with detailed result...");
    debug!("will be checking for : {:?}", list_of_required_tools);

    match check_tools_installed(list_of_required_tools) {
        Ok(missing) => {
            Ok(PrerequisitesCheckResult::new(missing, true, false))
        }
        Err(error_msg) => {
            debug!("Prerequisites check encountered an error: {}", error_msg);
            let shell_failed = !verify_shell_execution();
            // Verify if shell execution works
            if shell_failed {
                debug!("Shell execution verification also failed");
                Ok(PrerequisitesCheckResult::new(vec![], false, true))
            } else {
                // Shell works but we had an error - return the original error
                Err(error_msg)
            }
        }
    }
}

/// Checks the system for QEMU-specific dependencies and returns a list of unsatisfied packages.
///
/// This function retrieves the QEMU prerequisites based on the operating system and package manager,
/// then checks if each required package is installed. If a package is not found, it is added to
/// the `unsatisfied` vector and returned. The prerequisites are met when an empty vector is returned.
///
/// # Returns
///
/// * `Ok(Vec<&'static str>)` - If the function completes successfully, returns a vector of unsatisfied QEMU packages.
/// * `Err(String)` - If an error occurs, returns an error message.
pub fn check_qemu_prerequisites() -> Result<Vec<&'static str>, String> {
    let list_of_qemu_tools = get_qemu_prerequisites_based_on_package_manager();
    debug!("Checking for QEMU prerequisites...");
    debug!("will be checking for : {:?}", list_of_qemu_tools);

    check_tools_installed(list_of_qemu_tools)
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
                    let output = command_executor::execute_command(
                        "powershell",
                        &[
                          "-ExecutionPolicy",
                          "Bypass",
                          "-Command",
                          "scoop",
                          "bucket",
                          "add",
                          "versions"
                        ],
                    );
                    match output {
                        Ok(o) => {
                            if o.status.success() {
                                debug!("Successfully added versions bucket to scoop");
                            } else {
                                let output = String::from_utf8(o.stdout).unwrap();
                                let error_message = String::from_utf8(o.stderr).unwrap();
                                debug!("Failed to add versions bucket to scoop: {}", error_message);
                                debug!("Output: {}", output);
                            }
                        }
                        Err(e) => {
                            debug!("Failed to add versions bucket to scoop: {}", e);
                        }
                    }
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
                let mut main_command = get_correct_powershell_command();

                let output = command_executor::execute_command_with_env(
                    &main_command,
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

pub fn get_correct_powershell_command() -> String {
    match command_executor::execute_command("pwsh", &["--version"]) {
        Ok(_) => {
            debug!("Found powershell core");
            "pwsh".to_string()
        }
        Err(_) => {
            debug!("Powershell core not found, using powershell");
            "powershell".to_string()
        }
    }
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
pub fn add_to_path(new_path: &str) -> Result<String, std::io::Error> {
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
