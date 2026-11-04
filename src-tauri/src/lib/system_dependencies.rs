use anyhow::{anyhow, Context, Result};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

use log::{debug, warn};
use serde_json;

use crate::{
    command_executor, decompress_archive, download_file_and_rename,
    utils::find_by_name_and_extension,
};

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
        Self {
            missing,
            can_verify,
            shell_failed,
        }
    }

    /// Returns true if all prerequisites are satisfied and verification was successful.
    pub fn is_satisfied(&self) -> bool {
        self.can_verify && self.missing.is_empty()
    }
}

/// Determines the package manager installed on the system.
///
/// This function first attempts to identify the distribution by reading
/// `/etc/os-release` and mapping the `ID` or `ID_LIKE` fields to the
/// appropriate package manager. This avoids false positives caused by
/// cross-distribution tools being installed (e.g., `dpkg` on Fedora).
///
/// If `/etc/os-release` detection fails, it falls back to checking for
/// package manager binaries (excluding `dpkg`, which is commonly installed
/// as a standalone tool on non-Debian systems).
///
/// This should be only executed on Linux systems, as package managers on other operating systems
/// are not supported.
///
/// # Returns
///
/// * `Some(&'static str)` - If a package manager is found, returns the name of the package manager.
/// * `None` - If no package manager is found, returns None.
fn determine_package_manager() -> Option<&'static str> {
    // First, try to detect via /etc/os-release (most reliable)
    if let Some(pm) = detect_package_manager_from_os_release() {
        debug!("Package manager detected via /etc/os-release: {}", pm);
        return Some(pm);
    }

    // Fallback: probe for package manager binaries (excluding dpkg to avoid
    // false positives on non-Debian systems where dpkg is installed as a
    // standalone tool)
    let package_managers = vec!["apt", "dnf", "pacman", "zypper"];

    for manager in package_managers {
        let output = command_executor::execute_command(manager, &["--version"]);
        match output {
            Ok(output) => {
                if output.status.success() {
                    debug!("Package manager detected via binary probe: {}", manager);
                    return Some(manager);
                }
            }
            Err(_) => continue,
        }
    }

    None
}

/// Reads `/etc/os-release` and maps the distro ID to the appropriate package manager.
///
/// Checks the `ID` field first, then falls back to `ID_LIKE` for derivative distros
/// (e.g., Linux Mint has `ID=linuxmint` but `ID_LIKE=ubuntu`).
///
/// # Returns
///
/// * `Some(&'static str)` - The package manager name if the distro is recognized.
/// * `None` - If `/etc/os-release` cannot be read or the distro is not recognized.
fn detect_package_manager_from_os_release() -> Option<&'static str> {
    let content = match fs::read_to_string("/etc/os-release") {
        Ok(c) => c,
        Err(e) => {
            debug!("Could not read /etc/os-release: {}", e);
            return None;
        }
    };

    let mut id = None;
    let mut id_like = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = Some(value.trim_matches('"').to_lowercase());
        } else if let Some(value) = line.strip_prefix("ID_LIKE=") {
            id_like = Some(value.trim_matches('"').to_lowercase());
        }
    }

    // Try ID first, then ID_LIKE
    if let Some(ref distro_id) = id {
        if let Some(pm) = map_distro_to_package_manager(distro_id) {
            return Some(pm);
        }
    }

    // ID_LIKE can contain multiple space-separated values (e.g., "rhel fedora")
    if let Some(ref like) = id_like {
        for token in like.split_whitespace() {
            if let Some(pm) = map_distro_to_package_manager(token) {
                return Some(pm);
            }
        }
    }

    debug!("Unrecognized distro: ID={:?}, ID_LIKE={:?}", id, id_like);
    None
}

/// Maps a distribution identifier to its native package manager.
fn map_distro_to_package_manager(distro: &str) -> Option<&'static str> {
    match distro {
        // Debian-based
        "debian" | "ubuntu" | "linuxmint" | "pop" | "elementary" | "zorin" | "kali"
        | "raspbian" | "neon" | "deepin" | "peppermint" | "bodhi" => Some("apt"),
        // RPM-based (dnf)
        "fedora" | "rhel" | "centos" | "rocky" | "alma" | "ol" | "nobara" | "ultramarine"
        | "mageia" => Some("dnf"),
        // Arch-based
        "arch" | "manjaro" | "endeavouros" | "garuda" | "artix" | "cachyos" => Some("pacman"),
        // SUSE-based
        "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" | "sles" | "sled" => Some("zypper"),
        _ => None,
    }
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
/// * `Some(true)` - If shell execution works correctly
/// * `Some(false)` - If shell execution fails
/// * `None` - If the OS is unsupported
pub fn verify_shell_execution() -> Option<bool> {
    let result = match std::env::consts::OS {
        "linux" => command_executor::execute_command("sh", &["-c", "echo test"]),
        "macos" => command_executor::execute_command("zsh", &["-c", "echo test"]),
        "windows" => command_executor::execute_command("cmd", &["/c", "echo", "test"]),
        _ => {
            debug!(
                "Unsupported OS for shell verification: {}",
                std::env::consts::OS
            );
            return None;
        }
    };

    match result {
        Ok(output) => {
            if output.status.success() {
                debug!("Shell execution verification succeeded");
                Some(true)
            } else {
                debug!("Shell execution verification failed with non-zero exit code");
                Some(false)
            }
        }
        Err(e) => {
            debug!("Shell execution verification failed with error: {:?}", e);
            Some(false)
        }
    }
}

/// Returns a hardcoded vector of required tools based on the operating system.
///
/// # Returns
///
/// * `Vec<&'static str>` - A vector of required tools for the current operating system.
pub fn get_prerequisites() -> Vec<&'static str> {
    match std::env::consts::OS {
        "linux" => vec![
            "git", "wget", "flex", "bison", "gperf", "ccache", "dfu-util", "cmake",
        ],
        "windows" => vec!["git"],
        "macos" => vec!["dfu-util", "cmake"],
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
            Some("apt") => vec![
                "libgcrypt20",
                "libglib2.0-0",
                "libpixman-1-0",
                "libsdl2-2.0-0",
                "libslirp0",
            ],

            Some("dnf") => vec!["libgcrypt", "glib2", "pixman", "SDL2", "libslirp"],
            Some("pacman") => vec!["libgcrypt", "glib", "pixman", "sdl2", "libslirp"],
            Some("zypper") => vec![
                "libgcrypt20",
                "glib2",
                "pixman-1",
                "libSDL2-2_0-0",
                "libslirp",
            ],
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
    match std::env::consts::OS {
        "linux" => check_tools_with_package_manager(tools, determine_package_manager()),
        _ => check_tools_with_package_manager(tools, None),
    }
}

/// Checks if the given list of tools/packages are installed on the system.
/// This is an internal helper that allows injecting a package manager for testing.
///
/// # Arguments
///
/// * `tools` - A vector of tool/package names to check
/// * `package_manager` - An optional package manager name to use instead of detecting
///
/// # Returns
///
/// * `Ok(Vec<&'static str>)` - Vector of unsatisfied tools/packages
/// * `Err(String)` - If an error occurs, returns an error message
fn check_tools_with_package_manager(
    tools: Vec<&'static str>,
    package_manager: Option<&'static str>,
) -> Result<Vec<&'static str>, String> {
    match std::env::consts::OS {
        "linux" => check_tools_linux(tools, package_manager),
        "macos" => Ok(check_tools_macos(tools)),
        "windows" => check_tools_windows(tools),
        _ => Err(format!("Unsupported OS - {}", std::env::consts::OS)),
    }
}

/// Returns the shell command prefix used to query whether a package is installed
/// with the given Linux package manager, and whether a failed query should be logged.
fn package_query_prefix(package_manager: Option<&str>) -> Option<(&'static str, bool)> {
    match package_manager {
        Some("apt") => Some(("apt list --installed | grep ", true)),
        Some("dnf") => Some(("rpm -qa | grep -i ", false)),
        Some("pacman") => Some(("pacman -Qs | grep ", false)),
        Some("zypper") => Some(("zypper se --installed-only ", false)),
        _ => None,
    }
}

/// Returns true if the command ran and exited successfully, logging the outcome.
fn command_succeeded(
    tool: &str,
    output: Result<std::process::Output, std::io::Error>,
    log_failure: bool,
) -> bool {
    match output {
        Ok(o) if o.status.success() => {
            debug!("{} is already installed: {:?}", tool, o);
            true
        }
        Ok(o) => {
            if log_failure {
                debug!("check for {} failed: {:?}", tool, o);
            }
            false
        }
        Err(_e) => false,
    }
}

fn check_tools_linux(
    tools: Vec<&'static str>,
    package_manager: Option<&'static str>,
) -> Result<Vec<&'static str>, String> {
    let mut list_of_required_tools = tools;
    let mut unsatisfied = vec![];

    // git needs to be checked separately
    match command_executor::execute_command("git", &["--version"]) {
        Ok(output) if output.status.success() => {
            debug!("git is already installed: {:?}", output);
            list_of_required_tools.retain(|&tool| tool != "git");
        }
        Ok(output) => {
            debug!("check for git failed: {:?}", output);
            unsatisfied.push("git");
        }
        Err(_e) => {
            unsatisfied.push("git");
        }
    };

    // Check if tools are available using "command -v" via shell.
    // We use "command -v" instead of "which" because "which" is an external
    // binary that may not be installed on minimal Linux distributions
    // (e.g. Fedora containers).
    list_of_required_tools.retain(|&tool| {
        match command_executor::execute_command("sh", &["-c", &format!("command -v {}", tool)]) {
            Ok(o) if o.status.success() => {
                debug!("{} is already installed: {:?}", tool, o);
                false // Tool found, so remove it from the list (don't retain).
            }
            Ok(o) => {
                debug!("'command -v' check for {} failed: {:?}", tool, o);
                true
            }
            Err(e) => {
                debug!("'command -v' check for {} failed with error: {:?}", tool, e);
                true
            }
        }
    });

    // now check if the tools are installed with the package manager
    debug!("Using package manager: {:?}", package_manager);

    let (prefix, log_failure) = package_query_prefix(package_manager)
        .ok_or_else(|| "Unsupported package manager".to_string())?;
    for tool in list_of_required_tools {
        let output =
            command_executor::execute_command("sh", &["-c", &format!("{}{}", prefix, tool)]);
        if !command_succeeded(tool, output, log_failure) {
            unsatisfied.push(tool);
        }
    }

    Ok(unsatisfied)
}

fn check_tools_macos(tools: Vec<&'static str>) -> Vec<&'static str> {
    let mut unsatisfied = vec![];
    for tool in tools {
        let output =
            command_executor::execute_command("zsh", &["-c", &format!("command -v {}", tool)]);
        match output {
            Ok(o) if o.status.success() => {
                debug!("{} is already installed: {:?}", tool, o);
            }
            Ok(o) => {
                debug!("check for {} failed: {:?}", tool, o);
                // check if the tool is installed with brew
                match command_executor::execute_command("brew", &["list", tool]) {
                    Ok(o) if o.status.success() => {
                        debug!("{} is already installed with brew", tool);
                    }
                    _ => unsatisfied.push(tool),
                }
            }
            Err(_e) => {
                unsatisfied.push(tool);
            }
        }
    }
    unsatisfied
}

fn check_tools_windows(tools: Vec<&'static str>) -> Result<Vec<&'static str>, String> {
    let mut unsatisfied = vec![];
    let executor = command_executor::get_executor();

    // Verify that we can actually run PowerShell before any other test
    const CANARY: &str = "IDF_SHELL_CANARY_OK";
    match executor.run_script_from_string(&format!("Write-Output '{}'", CANARY)) {
        Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).contains(CANARY) => {
            debug!("PowerShell execution verified successfully");
        }
        Ok(o) => {
            return Err(format!(
                "Unable to execute PowerShell to verify prerequisites: command ran but produced unexpected output (stdout: {:?}, stderr: {:?}). PowerShell may be restricted (e.g. by execution policy or group policy).",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ));
        }
        Err(e) => {
            return Err(format!(
                "Unable to execute PowerShell to verify prerequisites: {}. Make sure PowerShell (powershell.exe or pwsh.exe) is installed and accessible.",
                e
            ));
        }
    }

    for tool in tools {
        // First try using where.exe which searches PATH
        let output = executor.run_script_from_string(&format!(
            "where.exe {} 2>$null; if ($LASTEXITCODE -ne 0) {{ Get-Command {} -ErrorAction SilentlyContinue }}",
            tool, tool
        ));
        match output {
            Ok(o) => {
                // The probe swallows failures (`2>$null`, `-ErrorAction SilentlyContinue`),
                // so PowerShell exits with 0 even when the tool is missing. Only the printed
                // location tells us it was actually found.
                let stdout = String::from_utf8_lossy(&o.stdout);
                if o.status.success() && !stdout.trim().is_empty() {
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
    let mut list_of_required_tools = get_prerequisites();
    list_of_required_tools = [
        list_of_required_tools,
        get_general_prerequisites_based_on_package_manager(),
    ]
    .concat();
    debug!("Checking for prerequisites with detailed result...");
    debug!("will be checking for : {:?}", list_of_required_tools);

    match check_tools_installed(list_of_required_tools) {
        Ok(missing) => Ok(PrerequisitesCheckResult::new(missing, true, false)),
        Err(error_msg) => {
            debug!("Prerequisites check encountered an error: {}", error_msg);

            if verify_shell_execution() == Some(false) {
                debug!("Shell execution verification also failed");
                Ok(PrerequisitesCheckResult::new(vec![], false, true))
            } else {
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

fn add_to_registry_path(new_entry: &Path) -> anyhow::Result<()> {
    if std::env::consts::OS != "windows" {
        return Ok(());
    }

    let new_str = new_entry.to_string_lossy().to_string();
    // Escape single quotes for PowerShell
    let escaped = new_str.replace('\'', "''");

    let ps_script = include_str!("../../powershell_scripts/add_to_registry_path_template.ps1")
        .replace("{{new_dir}}", &escaped);

    let executor = command_executor::get_executor();
    let output = executor
        .run_script_from_string(&ps_script)
        .map_err(|e| anyhow!("PowerShell failed: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "Registry PATH update failed: {} {}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    Ok(())
}

/// Adds git/bin and git/cmd directories to the current process PATH and user registry PATH.
///
/// This function makes git accessible in the current session and persists the change
/// across sessions by writing to the Windows registry.
///
/// # Arguments
///
/// * `git_dir` - A PathBuf pointing to the Git installation directory (containing bin/ and cmd/ subdirectories)
///
/// # Returns
///
/// * `Ok(())` - If git was added to PATH successfully
/// * `Err(anyhow::Error)` - If an error occurs
fn add_git_to_path(git_dir: &Path) -> anyhow::Result<()> {
    match std::env::consts::OS {
        "windows" => {
            let git_bin = git_dir.join("bin");
            let git_cmd = git_dir.join("cmd"); // git.exe lives here

            // For current process only (immediate effect):
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!(
                "{};{};{}",
                git_cmd.display(),
                git_bin.display(),
                current_path
            );
            std::env::set_var("PATH", &new_path);
            debug!("Updated current process PATH with git: {}", new_path);

            // For persistence across sessions — write to HKCU (no admin required):
            if let Err(e) = add_to_registry_path(&git_cmd) {
                debug!("Failed to add git_cmd to registry: {}", e);
            }
            if let Err(e) = add_to_registry_path(&git_bin) {
                debug!("Failed to add git_bin to registry: {}", e);
            }

            Ok(())
        }
        _ => {
            // On non-Windows, git is typically installed via package manager
            debug!(
                "Git PATH modification not needed on {}",
                std::env::consts::OS
            );
            Ok(())
        }
    }
}

const S3_TOOLS_BASE_URL: &str = "https://dl.espressif.com/dl/eim/tools";
const PYTHON_GITHUB_BASE_URL: &str =
    "https://github.com/astral-sh/python-build-standalone/releases/download/20260414";

/// Returns the Python standalone archive filename for the current OS/arch combination.
fn python_filename() -> anyhow::Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => {
            Ok("cpython-3.11.15+20260414-x86_64-pc-windows-msvc-install_only.tar.gz")
        }
        ("windows", "aarch64") => {
            Ok("cpython-3.11.15+20260414-aarch64-pc-windows-msvc-install_only.tar.gz")
        }
        (os, arch) => Err(anyhow!(
            "Unsupported OS/arch for Python standalone: {}/{}",
            os,
            arch
        )),
    }
}

/// Fetches the download URL for the latest Git for Windows tar.bz2 archive.
///
/// Tries the Espressif S3/CloudFront mirror first (preferred for users in mainland China),
/// then falls back to the GitHub API.
///
/// # Returns
///
/// * `Ok((String, String))` - A tuple of (url, filename) for the latest Git archive.
/// * `Err(anyhow::Error)` - If an error occurs during the HTTP request or parsing.
pub async fn get_latest_git_for_windows_url() -> anyhow::Result<(String, String)> {
    let arch_key = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" | "arm64" => "arm64",
        arch => {
            return Err(anyhow!(
                "Unsupported architecture for Git for Windows: {}",
                arch
            ))
        }
    };

    // Try S3 manifest first
    match get_git_for_windows_url_from_s3(arch_key).await {
        Ok(result) => {
            debug!("Using S3 mirror for Git for Windows download");
            return Ok(result);
        }
        Err(e) => {
            debug!(
                "S3 Git manifest unavailable ({}), falling back to GitHub API",
                e
            );
        }
    }

    get_git_for_windows_url_from_github(arch_key).await
}

/// Fetches the Git for Windows download URL from the Espressif S3 manifest.
async fn get_git_for_windows_url_from_s3(arch_key: &str) -> anyhow::Result<(String, String)> {
    let manifest_url = format!("{}/git/git-windows-latest.json", S3_TOOLS_BASE_URL);

    let tmp_dir = tempfile::tempdir()?;
    download_file_and_rename(
        &manifest_url,
        &tmp_dir.path().to_string_lossy(),
        None,
        Some("git-windows-latest.json"),
        1,
    )
    .await
    .map_err(|e| anyhow!("Failed to download Git manifest from S3: {}", e))?;

    let manifest_path = tmp_dir.path().join("git-windows-latest.json");
    let content = std::fs::read_to_string(&manifest_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let entry = &json[arch_key];
    let filename = entry["filename"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("No filename for arch '{}' in S3 manifest", arch_key))?
        .to_string();
    let url = entry["url"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("No url for arch '{}' in S3 manifest", arch_key))?
        .to_string();

    Ok((url, filename))
}

/// Fetches the Git for Windows download URL from the GitHub releases API.
async fn get_git_for_windows_url_from_github(arch_key: &str) -> anyhow::Result<(String, String)> {
    const API_URL: &str = "https://api.github.com/repos/git-for-windows/git/releases/latest";

    // Map our arch key to the suffix used in GitHub asset names
    let arch_suffix = match arch_key {
        "x86_64" => "64-bit",
        "arm64" => "arm64",
        arch => {
            return Err(anyhow!(
                "Unsupported architecture for Git for Windows: {}",
                arch
            ))
        }
    };

    let tmp_dir = tempfile::tempdir()?;
    download_file_and_rename(
        API_URL,
        &tmp_dir.path().to_string_lossy(),
        None,
        Some("git-for-windows-latest.json"),
        3,
    )
    .await
    .map_err(|e| anyhow!("Failed to fetch Git for Windows release from GitHub: {}", e))?;

    let content = std::fs::read_to_string(tmp_dir.path().join("git-for-windows-latest.json"))?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let assets = json["assets"]
        .as_array()
        .ok_or_else(|| anyhow!("No assets found in GitHub release response"))?;

    for asset in assets {
        let name = asset["name"].as_str().unwrap_or("");
        let url = asset["browser_download_url"].as_str().unwrap_or("");

        if name.starts_with("Git-") && name.ends_with(".tar.bz2") && name.contains(arch_suffix) {
            debug!("Found latest Git for Windows on GitHub: {}", name);
            return Ok((url.to_string(), name.to_string()));
        }
    }

    Err(anyhow!(
        "Could not find Git tar.bz2 asset for architecture '{}' in latest Git for Windows release",
        arch_suffix
    ))
}

/// Downloads Git for Windows from the official portable distribution.
///
/// This function fetches the latest release URL and downloads the portable archive.
///
/// # Arguments
///
/// * `tools_dir` - A PathBuf pointing to the directory where Git should be downloaded
/// * `progress_sender` - Optional channel sender for download progress updates
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the downloaded Git archive
/// * `Err(anyhow::Error)` - If an error occurs during download
pub async fn download_git(
    tools_dir: PathBuf,
    progress_sender: Option<std::sync::mpsc::Sender<crate::DownloadProgress>>,
) -> anyhow::Result<PathBuf> {
    match std::env::consts::OS {
        "windows" => {
            let (git_url, git_filename) = get_latest_git_for_windows_url().await?;
            debug!("Downloading Git from {}", git_url);

            // Download git portable archive
            download_file_and_rename(
                &git_url,
                &tools_dir.to_string_lossy(),
                progress_sender,
                Some(&git_filename),
                3,
            )
            .await
            .map_err(|e| anyhow!("Failed to download Git: {}", e))?;

            let git_downloaded_path = tools_dir.join(&git_filename);
            debug!("Git downloaded to {}", git_downloaded_path.display());
            Ok(git_downloaded_path)
        }
        _ => Err(anyhow!("download_git is only supported on Windows")),
    }
}

/// Installs Git from a previously downloaded archive.
///
/// This function extracts the Git portable archive to the tools directory
/// and adds it to the system PATH.
///
/// # Arguments
///
/// * `tools_dir` - A PathBuf pointing to the directory where Git should be installed
/// * `git_archive_path` - Optional path to the downloaded Git archive. If None, looks in tools_dir
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the installed Git directory
/// * `Err(anyhow::Error)` - If an error occurs during extraction
pub async fn install_git_from_downloaded(
    tools_dir: PathBuf,
    git_archive_path: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    match std::env::consts::OS {
        "windows" => {
            // Find the downloaded archive by looking for .tar.bz2 files
            let git_archive = if let Some(path) = git_archive_path {
                path
            } else {
                // Look for the archive in tools_dir
                let found = find_by_name_and_extension(&tools_dir, "git", "tar.bz2");
                found
                    .first()
                    .ok_or_else(|| {
                        anyhow!(
                            "Git archive not found in {}. Expected file ending with .tar.bz2",
                            tools_dir.display()
                        )
                    })?
                    .clone()
                    .into()
            };

            debug!("Extracting Git archive at {}", git_archive.display());

            // Extract the archive
            decompress_archive(
                &git_archive.to_string_lossy(),
                &tools_dir.join("git").to_string_lossy(),
            )
            .map_err(|e| anyhow!("Failed to extract Git archive: {}", e))?;

            let expected_git = tools_dir.join("git").join("bin").join("git.exe");
            // Find the actual Git directory (the archive extracts to a subdirectory like Git-2.54.0-64-bit/)
            let git_install_dir = if expected_git.exists() {
                tools_dir.join("git")
            } else {
                find_git_install_dir(&tools_dir)?
                    .parent()
                    .unwrap()
                    .to_path_buf()
            };

            add_git_to_path(&git_install_dir)?;

            // Clean up the archive only after successful installation
            let _ = std::fs::remove_file(&git_archive);

            debug!(
                "Git installed successfully at {}",
                git_install_dir.display()
            );
            Ok(git_install_dir)
        }
        _ => Err(anyhow!(
            "install_git_from_downloaded is only supported on Windows"
        )),
    }
}

/// Installs Git for Windows from the official portable distribution.
///
/// This function downloads the Git portable executable, runs it silently with installation flags,
/// and adds it to the system PATH. The installation is performed in the specified tools directory.
///
/// # Arguments
///
/// * `tools_dir` - A PathBuf pointing to the directory where Git should be installed
/// * `progress_sender` - Optional channel sender for download progress updates
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the installed Git directory
/// * `Err(anyhow::Error)` - If an error occurs during download or installation
pub async fn install_git(
    tools_dir: PathBuf,
    progress_sender: Option<std::sync::mpsc::Sender<crate::DownloadProgress>>,
) -> anyhow::Result<PathBuf> {
    ensure_windows("install_git")?;
    let git_exe = download_git(tools_dir.clone(), progress_sender).await?;
    install_git_from_downloaded(tools_dir, Some(git_exe)).await
}

fn ensure_windows(function_name: &str) -> anyhow::Result<()> {
    if std::env::consts::OS == "windows" {
        Ok(())
    } else {
        Err(anyhow!("{} is only supported on Windows", function_name))
    }
}

/// Downloads Python from the official standalone distribution for Windows.
///
/// This function downloads the Python standalone archive to the specified tools directory.
///
/// # Arguments
///
/// * `tools_dir` - A PathBuf pointing to the directory where Python should be downloaded
/// * `progress_sender` - Optional channel sender for download progress updates
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the downloaded Python archive
/// * `Err(anyhow::Error)` - If an error occurs during download
pub async fn download_python(
    tools_dir: PathBuf,
    progress_sender: Option<std::sync::mpsc::Sender<crate::DownloadProgress>>,
) -> anyhow::Result<PathBuf> {
    match std::env::consts::OS {
        "windows" => {
            let filename = python_filename()?;
            let python_s3_url = format!("{}/python/{}", S3_TOOLS_BASE_URL, filename);
            let python_github_url = format!("{}/{}", PYTHON_GITHUB_BASE_URL, filename);

            // Try S3 mirror first (better availability in mainland China), fall back to GitHub.
            let urls = [python_s3_url.as_str(), python_github_url.as_str()];
            let mut last_err = anyhow!("No download URLs available");

            for url in &urls {
                debug!("Downloading Python from {}", url);
                match download_file_and_rename(
                    url,
                    &tools_dir.to_string_lossy(),
                    progress_sender.clone(),
                    Some(filename),
                    3,
                )
                .await
                {
                    Ok(_) => {
                        let python_archive_path = tools_dir.join(filename);
                        debug!("Python downloaded to {}", python_archive_path.display());
                        return Ok(python_archive_path);
                    }
                    Err(e) => {
                        debug!("Failed to download Python from {}: {}", url, e);
                        last_err = anyhow!("Failed to download Python from {}: {}", url, e);
                    }
                }
            }

            Err(last_err)
        }
        _ => Err(anyhow!("download_python is only supported on Windows")),
    }
}

/// Installs Python from a previously downloaded archive.
///
/// This function extracts the Python archive to the tools directory and adds it to the system PATH.
///
/// # Arguments
///
/// * `tools_dir` - A PathBuf pointing to the directory where Python should be installed
/// * `python_archive_path` - Optional path to the downloaded Python archive. If None, looks in tools_dir
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the installed Python directory
/// * `Err(anyhow::Error)` - If an error occurs during extraction
pub async fn install_python_from_downloaded(
    tools_dir: PathBuf,
    python_archive_path: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    match std::env::consts::OS {
        "windows" => {
            let python_archive = match python_archive_path {
                Some(p) => p,
                None => tools_dir.join(python_filename()?),
            };

            debug!("Extracting Python to {}", tools_dir.display());

            // Extract the archive
            decompress_archive(
                &python_archive.to_string_lossy(),
                &tools_dir.to_string_lossy(),
            )
            .map_err(|e| anyhow!("Failed to extract Python archive: {}", e))?;

            // Clean up the archive
            let _ = std::fs::remove_file(&python_archive);

            // Find the actual Python directory (the archive extracts to a subdirectory like python3.11)
            let python_install_dir = find_python_install_dir(&tools_dir)?;

            // Add python to PATH
            add_python_to_path(&python_install_dir)?;

            debug!(
                "Python installed successfully at {}",
                python_install_dir.display()
            );
            Ok(python_install_dir)
        }
        _ => Err(anyhow!(
            "install_python_from_downloaded is only supported on Windows"
        )),
    }
}

/// Finds the Python installation directory by looking for python.exe in subdirectories.
///
/// The Python standalone archive extracts to a structure like:
/// tools_dir/
///   └── python3.11/
///       └── python.exe
///
/// This function finds that actual directory using find_by_name_and_extension.
/// Returns an error if python.exe is not found, or if it's found directly in tools_dir
/// (not in a subdirectory) which would indicate an unexpected extraction pattern.
fn find_python_install_dir(tools_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    find_exe_install_dir(tools_dir, "python", "Python")
}

/// Finds the Git installation directory by looking for git.exe in subdirectories.
///
/// The Git portable archive extracts to a structure like:
/// tools_dir/
///   └── PortableGit-xxxx.xxx/
///       └── bin\git.exe
///
/// This function finds that actual directory using find_by_name_and_extension.
/// Returns an error if git.exe is not found, or if it's found directly in tools_dir
/// (not in a subdirectory) which would indicate an unexpected extraction pattern.
fn find_git_install_dir(tools_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    find_exe_install_dir(tools_dir, "git", "Git")
}

/// Finds the directory containing `<exe_name>.exe` somewhere below `tools_dir`.
///
/// Returns an error if the executable is not found, or if it's found directly in
/// `tools_dir` (not in a subdirectory), which indicates an unexpected extraction pattern.
fn find_exe_install_dir(
    tools_dir: &PathBuf,
    exe_name: &str,
    display_name: &str,
) -> anyhow::Result<PathBuf> {
    let found = find_by_name_and_extension(tools_dir, exe_name, "exe");

    let Some(exe_path) = found.first() else {
        return Err(anyhow!(
            "{}.exe not found in {}. {} installation may have failed.",
            exe_name,
            tools_dir.display(),
            display_name
        ));
    };
    let exe_dir = PathBuf::from(exe_path)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("Failed to get parent directory of {}.exe", exe_name))?;

    if exe_dir == *tools_dir {
        return Err(anyhow!(
            "{}.exe found directly in tools_dir ({}), expected it to be in a subdirectory. Archive extraction may have unexpected structure.",
            exe_name,
            tools_dir.display()
        ));
    }

    debug!(
        "Found {} installation at: {}",
        display_name,
        exe_dir.display()
    );
    Ok(exe_dir)
}

/// Installs Python from the official standalone distribution for Windows.
///
/// This function downloads the Python standalone archive, extracts it to the tools directory,
/// and adds it to the system PATH. The installation is performed in the specified tools directory.
///
/// # Arguments
///
/// * `tools_dir` - A PathBuf pointing to the directory where Python should be installed
/// * `progress_sender` - Optional channel sender for download progress updates
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the installed Python directory
/// * `Err(anyhow::Error)` - If an error occurs during download or extraction
pub async fn install_python(
    tools_dir: PathBuf,
    progress_sender: Option<std::sync::mpsc::Sender<crate::DownloadProgress>>,
) -> anyhow::Result<PathBuf> {
    ensure_windows("install_python")?;
    let python_archive = download_python(tools_dir.clone(), progress_sender).await?;
    install_python_from_downloaded(tools_dir, Some(python_archive)).await
}

/// Adds Python directories to the current process PATH and user registry PATH.
///
/// This function makes python accessible in the current session and persists the change
/// across sessions by writing to the Windows registry.
///
/// # Arguments
///
/// * `python_dir` - A PathBuf pointing to the Python installation directory (containing Scripts/ and PCBuild/ etc.)
///
/// # Returns
///
/// * `Ok(())` - If Python was added to PATH successfully
/// * `Err(anyhow::Error)` - If an error occurs
fn add_python_to_path(python_dir: &Path) -> anyhow::Result<()> {
    match std::env::consts::OS {
        "windows" => {
            // For Python standalone builds, we need to add:
            // - The root directory (contains python.exe)
            // - Scripts directory if it exists (contains pip.exe)

            let current_path = std::env::var("PATH").unwrap_or_default();

            // Add root directory to PATH for python.exe
            let new_path = format!("{};{}", python_dir.display(), current_path);
            std::env::set_var("PATH", &new_path);
            debug!("Updated current process PATH with python: {}", new_path);

            // Only add Scripts directory if it actually exists
            let scripts_dir = python_dir.join("Scripts");
            if scripts_dir.exists() {
                let new_path_with_scripts = format!(
                    "{};{};{}",
                    scripts_dir.display(),
                    python_dir.display(),
                    current_path
                );
                std::env::set_var("PATH", &new_path_with_scripts);
                debug!(
                    "Updated current process PATH with python Scripts: {}",
                    new_path_with_scripts
                );
                if let Err(e) = add_to_registry_path(&scripts_dir) {
                    debug!("Failed to add Scripts to registry: {}", e);
                }
            } else {
                debug!(
                    "Scripts directory does not exist, skipping: {}",
                    scripts_dir.display()
                );
            }

            // Add the root directory for persistence
            if let Err(e) = add_to_registry_path(python_dir) {
                debug!("Failed to add python_dir to registry: {}", e);
            }

            Ok(())
        }
        _ => {
            // On non-Windows, Python is typically installed via package manager
            debug!(
                "Python PATH modification not needed on {}",
                std::env::consts::OS
            );
            Ok(())
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
/// * `tools_dir` - A PathBuf pointing to the directory where tools (git, python) should be installed.
///   On Windows, this is typically the tools directory in the user's home folder.
///
/// # Returns
///
/// * `Ok(())` - If the packages are successfully installed.
/// * `Err(String)` - If an error occurs during the installation process.
pub async fn install_prerequisites(
    packages_list: Vec<String>,
    tools_dir: PathBuf,
) -> Result<(), String> {
    match std::env::consts::OS {
        "linux" => {
            let install_args = linux_install_args(determine_package_manager())
                .ok_or_else(|| "Unsupported package manager".to_string())?;
            install_packages_with("sudo", install_args, packages_list).await
        }
        "macos" => install_packages_with("brew", &["install"], packages_list).await,
        "windows" => install_windows_prerequisites(packages_list, tools_dir).await,
        _ => Err(format!("Unsupported OS - {}", std::env::consts::OS)),
    }
}

/// Returns the `sudo` arguments (without the package name) used to install a
/// package with the given Linux package manager.
fn linux_install_args(package_manager: Option<&str>) -> Option<&'static [&'static str]> {
    match package_manager {
        Some("apt") => Some(&["apt", "install", "-y"]),
        Some("dnf") => Some(&["dnf", "install", "-y"]),
        Some("pacman") => Some(&["pacman", "-S", "--noconfirm"]),
        Some("zypper") => Some(&["zypper", "install", "-y"]),
        _ => None,
    }
}

/// Installs each package by running `program <base_args..> <package>`, panicking on failure.
async fn install_packages_with(
    program: &'static str,
    base_args: &'static [&'static str],
    packages_list: Vec<String>,
) -> Result<(), String> {
    for package in packages_list {
        let pkg = package.to_string();
        let output = tokio::task::spawn_blocking(move || {
            let mut args = base_args.to_vec();
            args.push(&pkg);
            command_executor::execute_command_direct(program, &args)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?;
        match output {
            Ok(_) => {
                debug!("Successfully installed {}", package);
            }
            Err(e) => panic!("Failed to install {}: {}", package, e),
        }
    }
    Ok(())
}

async fn install_windows_prerequisites(
    packages_list: Vec<String>,
    tools_dir: PathBuf,
) -> Result<(), String> {
    // Ensure tools directory exists
    if !tools_dir.exists() {
        std::fs::create_dir_all(&tools_dir)
            .map_err(|e| format!("Failed to create tools directory: {}", e))?;
    }

    for package in packages_list {
        if package.starts_with("python") {
            match install_python(tools_dir.clone(), None).await {
                Ok(install_path) => {
                    debug!("Successfully installed python to {:?}", install_path);
                }
                Err(e) => {
                    return Err(format!("Failed to install python: {}", e));
                }
            }
        } else if package == "git" {
            match install_git(tools_dir.clone(), None).await {
                Ok(install_path) => {
                    debug!("Successfully installed git to {:?}", install_path);
                }
                Err(e) => {
                    return Err(format!("Failed to install git: {}", e));
                }
            }
        } else {
            return Err(format!(
                "Unsupported package on Windows: '{}'. Only 'git' and 'python*' are supported.",
                package
            ));
        }
    }
    Ok(())
}

pub fn get_correct_powershell_command() -> String {
    match command_executor::execute_command_direct("pwsh", &["--version"]) {
        Ok(o) => {
            if o.status.success() {
                debug!("Powershell core is available: {:?}", o.stdout);
                "pwsh".to_string()
            } else {
                debug!(
                    "Powershell core check failed: {:?}, {:?}",
                    o.stdout, o.stderr
                );
                "powershell".to_string()
            }
        }
        Err(_) => {
            debug!("Powershell core not found, using powershell");
            "powershell".to_string()
        }
    }
}

/// Adds a new directory to the system's PATH environment variable.
///
/// This function appends the new directory to the current process PATH if it's not
/// already present. On Windows, it also persists the change to the user's registry
/// PATH by delegating to `add_to_registry_path`, which broadcasts `WM_SETTINGCHANGE`
/// so new processes pick up the change without requiring a logout.
///
/// Note: The currently-running terminal will not see the persisted change — only
/// processes spawned *after* this call will inherit the updated PATH.
///
/// # Parameters
///
/// * `new_path` - A string slice representing the new directory path to be added to the PATH.
///
/// # Returns
///
/// * `Ok(String)` - The updated in-process PATH string if the operation is successful.
/// * `Err(std::io::Error)` - If persisting the PATH to the registry fails on Windows.
pub fn add_to_path(new_path: &str) -> Result<String, std::io::Error> {
    let binding = env::var_os("PATH").unwrap_or_default();
    let paths = binding.to_string_lossy().to_string();

    let separator = if std::env::consts::OS == "windows" {
        ';'
    } else {
        ':'
    };

    // Case-insensitive dedup on Windows, case-sensitive elsewhere.
    let already_present = paths.split(separator).any(|p| {
        let a = p.trim_end_matches(['/', '\\']);
        let b = new_path.trim_end_matches(['/', '\\']);
        if std::env::consts::OS == "windows" {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    });

    let new_path_string = if already_present {
        paths.clone()
    } else if paths.is_empty() {
        new_path.to_string()
    } else {
        format!("{}{}{}", new_path, separator, paths)
    };

    // Update the current process PATH so the change takes effect immediately
    // for any child processes spawned from this one.
    if !already_present {
        env::set_var("PATH", &new_path_string);
        debug!("Added {} to current process PATH", new_path);
    } else {
        debug!("{} already in current process PATH, skipping", new_path);
    }

    // Persist to the user registry on Windows (no admin required).
    // This also broadcasts WM_SETTINGCHANGE so new processes see the update.
    if std::env::consts::OS == "windows" {
        let path_buf = PathBuf::from(new_path);
        add_to_registry_path(&path_buf).map_err(|e| {
            warn!("Failed to persist {} to registry PATH: {}", new_path, e);
            std::io::Error::other(format!("Failed to update registry PATH: {}", e))
        })?;
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
///   including the openocd rules file.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_distro_to_package_manager_debian() {
        let distros = vec![
            "debian",
            "ubuntu",
            "linuxmint",
            "pop",
            "elementary",
            "zorin",
            "kali",
            "raspbian",
            "neon",
            "deepin",
            "peppermint",
            "bodhi",
        ];
        for distro in distros {
            assert_eq!(
                map_distro_to_package_manager(distro),
                Some("apt"),
                "Expected 'apt' for distro: {}",
                distro
            );
        }
    }

    #[test]
    fn test_map_distro_to_package_manager_rpm() {
        let distros = vec![
            "fedora",
            "rhel",
            "centos",
            "rocky",
            "alma",
            "ol",
            "nobara",
            "ultramarine",
            "mageia",
        ];
        for distro in distros {
            assert_eq!(
                map_distro_to_package_manager(distro),
                Some("dnf"),
                "Expected 'dnf' for distro: {}",
                distro
            );
        }
    }

    #[test]
    fn test_map_distro_to_package_manager_arch() {
        let distros = vec![
            "arch",
            "manjaro",
            "endeavouros",
            "garuda",
            "artix",
            "cachyos",
        ];
        for distro in distros {
            assert_eq!(
                map_distro_to_package_manager(distro),
                Some("pacman"),
                "Expected 'pacman' for distro: {}",
                distro
            );
        }
    }

    #[test]
    fn test_map_distro_to_package_manager_suse() {
        let distros = vec![
            "opensuse",
            "opensuse-leap",
            "opensuse-tumbleweed",
            "sles",
            "sled",
        ];
        for distro in distros {
            assert_eq!(
                map_distro_to_package_manager(distro),
                Some("zypper"),
                "Expected 'zypper' for distro: {}",
                distro
            );
        }
    }

    #[test]
    fn test_map_distro_to_package_manager_unsupported() {
        let distros = vec!["unknown", "gentoo", "slackware", "nixos", "void", "freebsd"];
        for distro in distros {
            assert_eq!(
                map_distro_to_package_manager(distro),
                None,
                "Expected None for unsupported distro: {}",
                distro
            );
        }
    }

    #[test]
    fn test_prerequisites_check_result_is_satisfied() {
        let result = PrerequisitesCheckResult::new(vec![], true, false);
        assert!(result.is_satisfied());

        let result = PrerequisitesCheckResult::new(vec!["git", "cmake"], true, false);
        assert!(!result.is_satisfied());

        let result = PrerequisitesCheckResult::new(vec![], false, false);
        assert!(!result.is_satisfied());

        let result = PrerequisitesCheckResult::new(vec!["git"], false, true);
        assert!(!result.is_satisfied());
    }

    #[test]
    fn test_prerequisites_check_result_new() {
        let result = PrerequisitesCheckResult::new(vec!["git"], true, false);
        assert_eq!(result.missing, vec!["git"]);
        assert!(result.can_verify);
        assert!(!result.shell_failed);
    }

    // Test that check_tools_with_package_manager returns an error (not a panic)
    // when package_manager is None on Linux.
    #[test]
    #[cfg(target_os = "linux")]
    fn test_check_tools_installed_with_none_package_manager() {
        // On Linux, when package_manager is None (unsupported distro),
        // the function should return an Err, not panic
        let tools = vec!["git", "cmake"];
        let result = check_tools_with_package_manager(tools, None);

        // Should return error message about unsupported package manager
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("Unsupported package manager"),
            "Expected error message to contain 'Unsupported package manager', got: {}",
            err_msg
        );
    }

    // Test that check_tools_with_package_manager works correctly with a valid package manager
    #[test]
    #[cfg(target_os = "linux")]
    fn test_check_tools_installed_with_apt_package_manager() {
        // When package manager is apt, it should attempt to check tools
        // Note: This might return Ok with missing tools or an error from apt commands,
        // but it should NOT panic
        let tools = vec!["git", "cmake"];
        let result = check_tools_with_package_manager(tools, Some("apt"));

        // Result should be either Ok (with list of missing tools) or Err (if apt fails)
        // but it should never panic
        match result {
            Ok(missing) => {
                // This is fine - it means the tools were checked and some are missing
                assert!(missing.iter().all(|t| *t == "git" || *t == "cmake"));
            }
            Err(e) => {
                // This is also fine - could fail due to apt not being available
                // or other system issues
                assert!(
                    e.contains("Unsupported package manager") || !e.is_empty(),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    // Test that check_tools_with_package_manager returns error for unknown package manager
    #[test]
    #[cfg(target_os = "linux")]
    fn test_check_tools_installed_with_unknown_package_manager() {
        let tools = vec!["git", "cmake"];
        let result = check_tools_with_package_manager(tools, Some("unknown_pm"));

        // Should return error message about unsupported package manager
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("Unsupported package manager"),
            "Expected error message to contain 'Unsupported package manager', got: {}",
            err_msg
        );
    }

    #[test]
    fn test_package_query_prefix() {
        assert_eq!(
            package_query_prefix(Some("apt")),
            Some(("apt list --installed | grep ", true))
        );
        assert_eq!(
            package_query_prefix(Some("dnf")),
            Some(("rpm -qa | grep -i ", false))
        );
        assert_eq!(
            package_query_prefix(Some("pacman")),
            Some(("pacman -Qs | grep ", false))
        );
        assert_eq!(
            package_query_prefix(Some("zypper")),
            Some(("zypper se --installed-only ", false))
        );
        assert_eq!(package_query_prefix(Some("unknown_pm")), None);
        assert_eq!(package_query_prefix(None), None);
    }

    #[test]
    fn test_linux_install_args() {
        assert_eq!(
            linux_install_args(Some("apt")),
            Some(&["apt", "install", "-y"][..])
        );
        assert_eq!(
            linux_install_args(Some("dnf")),
            Some(&["dnf", "install", "-y"][..])
        );
        assert_eq!(
            linux_install_args(Some("pacman")),
            Some(&["pacman", "-S", "--noconfirm"][..])
        );
        assert_eq!(
            linux_install_args(Some("zypper")),
            Some(&["zypper", "install", "-y"][..])
        );
        assert_eq!(linux_install_args(Some("brew")), None);
        assert_eq!(linux_install_args(None), None);
    }

    #[test]
    #[cfg(unix)]
    fn test_command_succeeded() {
        use std::os::unix::process::ExitStatusExt;
        let output = |code: i32| std::process::Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: vec![],
            stderr: vec![],
        };
        assert!(command_succeeded("git", Ok(output(0)), true));
        assert!(!command_succeeded("git", Ok(output(1)), true));
        assert!(!command_succeeded("git", Ok(output(1)), false));
        assert!(!command_succeeded(
            "git",
            Err(std::io::Error::other("not found")),
            false
        ));
    }

    #[test]
    fn test_ensure_windows() {
        let result = ensure_windows("install_git");
        if std::env::consts::OS == "windows" {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result.unwrap_err().to_string(),
                "install_git is only supported on Windows"
            );
        }
    }

    #[test]
    fn test_find_exe_install_dir_in_subdirectory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sub = tmp.path().join("python3.11");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("python.exe"), b"").unwrap();

        let found = find_python_install_dir(&tmp.path().to_path_buf()).unwrap();
        assert_eq!(found, sub);
    }

    #[test]
    fn test_find_exe_install_dir_directly_in_tools_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(tmp.path().join("git.exe"), b"").unwrap();

        let err = find_git_install_dir(&tmp.path().to_path_buf())
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with("git.exe found directly in tools_dir"),
            "{}",
            err
        );
    }

    #[test]
    fn test_find_exe_install_dir_not_found() {
        let tmp = tempfile::TempDir::new().unwrap();

        let err = find_git_install_dir(&tmp.path().to_path_buf())
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            format!(
                "git.exe not found in {}. Git installation may have failed.",
                tmp.path().display()
            )
        );
        let err = find_python_install_dir(&tmp.path().to_path_buf())
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            format!(
                "python.exe not found in {}. Python installation may have failed.",
                tmp.path().display()
            )
        );
    }
}
