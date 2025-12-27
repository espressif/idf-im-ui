use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, trace, warn};
#[cfg(feature = "userustpython")]
use rustpython_vm as vm;
#[cfg(feature = "userustpython")]
use rustpython_vm::function::PosArgs;
use semver::{Version, VersionReq};
#[cfg(feature = "userustpython")]
use std::process::ExitCode;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, SystemTime},
    vec,
};
#[cfg(feature = "userustpython")]
use vm::{builtins::PyStrRef, Interpreter};

use crate::{
    command_executor, download_file, ensure_path, replace_unescaped_spaces_posix, replace_unescaped_spaces_win, settings::VersionPaths, system_dependencies::get_scoop_path, utils::{copy_dir_contents, parse_cmake_version, remove_after_second_dot, with_retry}
};

/// Runs a Python script from a specified file with optional arguments and environment variables.
/// todo: check documentation
/// # Parameters
///
/// * `path` - A reference to a string representing the path to the Python script file.
/// * `args` - An optional reference to a string representing the arguments to be passed to the Python script.
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
/// * `envs` - An optional reference to a vector of tuples representing environment variables to be set for the Python script.
///
/// # Returns
///
/// * `Result<String, String>` - On success, returns a `Result` containing the standard output of the Python script as a string.
///   On error, returns a `Result` containing the standard error of the Python script as a string.
pub fn run_python_script_from_file(
    path: &str,
    args: Option<&str>,
    python: Option<&str>,
    envs: Option<&Vec<(String, String)>>,
) -> Result<String, String> {
    let callable = if let Some(args) = args {
        format!("{} {} {}", python.unwrap_or("python3"), path, args)
    } else {
        format!("{} {}", python.unwrap_or("python3"), path)
    };
    let executor = command_executor::get_executor();

    let output = match envs {
        Some(envs) => {
            let envs_str = envs
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<Vec<(&str, &str)>>();

            match std::env::consts::OS {
                "windows" => executor.execute_with_env(
                    "powershell",
                    &[
                        "-Command",
                        python.unwrap_or("python3.exe"),
                        path,
                        args.unwrap_or(""),
                    ],
                    envs_str,
                ),
                _ => executor.execute_with_env("bash", &["-c", &callable], envs_str),
            }
        }
        None => match std::env::consts::OS {
            "windows" => executor.execute(
                "powershell",
                &[
                    "-Command",
                    python.unwrap_or("python3.exe"),
                    path,
                    args.unwrap_or(""),
                ],
            ),
            _ => executor.execute("bash", &["-c", &callable]),
        },
    };

    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(std::str::from_utf8(&out.stdout).unwrap().to_string())
            } else {
                Err(std::str::from_utf8(&out.stderr).unwrap().to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Downloads the ESP-IDF constraints file for a given IDF version.
///
/// This function constructs the appropriate constraints file name and URL based on the
/// provided `idf_version`. It first checks if the file already exists locally and
/// was downloaded within the last 24 hours. If it's fresh, the download is skipped.
/// Otherwise, it creates the necessary directories and proceeds to download the file.
///
/// # Arguments
///
/// * `idf_tools_path` - A reference to a `Path` indicating the directory where IDF tools
///   and associated files (like the constraints file) are stored.
/// * `idf_version` - A string slice representing the ESP-IDF version.
///   For example, "master", "v4.4", or "v5.1".
///
/// # Returns
///
/// A `Result` which is:
/// - `Ok(PathBuf)` containing the path to the downloaded (or existing fresh)
///   constraints file if the operation was successful.
/// - `Err(anyhow::Error)` if an error occurred during directory creation,
///   downloading, or other file operations.
///
/// # Errors
///
/// This function can return an error if:
/// - There are issues creating the parent directories for the constraints file.
/// - The download of the constraints file fails for any reason (e.g., network issues,
///   invalid URL, server errors).
/// - File system metadata cannot be accessed or modified times cannot be determined.
pub async fn download_constraints_file(idf_tools_path: &Path, idf_version: &str) -> Result<PathBuf> {
    let constraint_file = format!(
        "espidf.constraints.{}.txt",
        remove_after_second_dot(idf_version)
    );
    let constraint_path = idf_tools_path.join(&constraint_file);
    let constraint_url = format!("https://dl.espressif.com/dl/esp-idf/{}", constraint_file);

    // Check if file exists and is fresh (less than 1 day old)
    if constraint_path.exists() {
        if let Ok(metadata) = fs::metadata(&constraint_path) {
            if let Ok(modified) = metadata.modified() {
                let now = SystemTime::now();
                if now.duration_since(modified).unwrap_or_default() < Duration::from_secs(86400) {
                    info!(
                        "Skipping the download of {} because it was downloaded recently.",
                        constraint_path.display()
                    );
                    return Ok(constraint_path);
                }
            }
        }
    }

    // Create directory if needed
    if let Some(parent) = constraint_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Download the constraints file
    info!("Downloading constraints file from {}", constraint_url);

    match download_file(&constraint_url, idf_tools_path.to_str().unwrap(), None).await {
        Ok(_) => {
            info!(
                "Downloaded constraints file to {}",
                constraint_path.display()
            );
        }
        Err(e) => {
            error!("Failed to download constraints file: {}", e);
            return Err(anyhow!("Failed to download constraints file: {}", e));
        }
    }

    info!(
        "Downloaded constraints file to {}",
        constraint_path.display()
    );
    Ok(constraint_path)
}

/// Creates a Python virtual environment at the specified path.
///
/// This function executes the `python3 -m venv` command to create a new virtual
/// environment. It adapts the command based on the operating system to ensure
/// compatibility (using `powershell` on Windows and `bash` on other systems).
///
/// # Arguments
///
/// * `venv_path` - A string slice that specifies the desired path for the new
///   virtual environment.
///
/// # Returns
///
/// A `Result` which is:
/// - `Ok(String)` containing the standard output from the `venv` command if
///   the virtual environment was created successfully.
/// - `Err(String)` containing the standard error output or an error message
///   if the command failed to execute or if the `venv` creation was unsuccessful.
///
/// # Errors
///
/// This function can return an error if:
/// - The `python3` command is not found in the system's PATH.
/// - There are insufficient permissions to create directories at `venv_path`.
/// - Other system-level errors prevent the command from executing.
/// - The `python -m venv` command itself encounters an error (e.g., invalid path).
fn create_python_venv(venv_path: &str, python_executable: &str) -> Result<String, String> {
  info!("Creating Python virtual environment at: {}", venv_path);

    let output = match std::env::consts::OS {
        "windows" => command_executor::execute_command(
            "powershell",
            &["-Command", python_executable, "-m", "venv", venv_path],
        ),
        _ => command_executor::execute_command(
            "bash",
            &["-c", &format!("{} -m venv {}", python_executable, venv_path)],
        ),
    };
    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(std::str::from_utf8(&out.stdout).unwrap().to_string())
            } else {
                Err(std::str::from_utf8(&out.stderr).unwrap().to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Installs Python packages listed in a requirements file into a specified virtual environment
/// using pip.
///
/// This function activates the virtual environment and then runs `pip install -r` to install
/// the dependencies. It also handles an optional constraints file to manage package versions.
/// The function adjusts its command execution based on the operating system (Windows or Unix-like)
/// and temporarily disables `PIP_USER` if it's set, to ensure packages are installed into
/// the virtual environment.
///
/// # Arguments
///
/// * `venv_path` - A reference to a `Path` pointing to the root directory of the Python
///   virtual environment.
/// * `requirements_file` - A reference to a `Path` pointing to the `requirements.txt` file
///   containing the list of Python packages to install.
/// * `constraint_file` - An `Option<PathBuf>` that, if present, specifies the path to a
///   pip constraints file (`.txt`). This file can be used to pin package versions.
/// * `wheel_dir` - An `Option<PathBuf>` that, if present, enables offline installation mode
///   by specifying a directory containing wheel files.
/// * `pypi_mirror` - An `Option<String>` that, if present, specifies a custom PyPI mirror URL
///   to use as the package index (e.g., "https://pypi.tuna.tsinghua.edu.cn/simple").
///
/// # Returns
///
/// A `Result<(), std::io::Error>` which is:
/// - `Ok(())` if the pip installation was successful.
/// - `Err(std::io::Error)` if the command failed to execute, or if pip
///   returned a non-zero exit code (indicating an installation error). The error
///   will contain the standard error output from the pip command.
///
/// # Errors
///
/// This function can return an `std::io::Error` if:
/// - The `python` or `python.exe` executable within the virtual environment
///   cannot be found or executed.
/// - The `requirements_file` does not exist or is not readable.
/// - The `constraint_file` (if provided) does not exist or is not readable.
/// - There are network issues preventing pip from downloading packages.
/// - Dependency conflicts or other pip-related errors occur during installation.
pub fn pip_install_requirements(
    venv_path: &Path,
    requirements_file: &Path,
    constraint_file: &Option<PathBuf>,
    wheel_dir: &Option<PathBuf>,
    pypi_mirror: &Option<String>,
) -> Result<(), std::io::Error> {
    let python_location = match std::env::consts::OS {
        "windows" => venv_path.join("Scripts").join("python.exe"),
        _ => venv_path.join("bin").join("python3"),
    };
    std::env::set_var("VIRTUAL_ENV", venv_path.to_str().unwrap());
    if std::env::var("PIP_USER").unwrap_or_default() == "yes" {
        debug!("Found PIP_USER=\"yes\" in the environment. Disabling PIP_USER in this shell to install packages into a virtual environment.");
        std::env::set_var("PIP_USER", "no".to_string());
    }
    let constrain_path = match constraint_file {
        Some(path) => path.to_str().unwrap(),
        None => "",
    };

    match std::env::consts::OS {
        "windows" => {
            match if let Some(wheel_dir) = wheel_dir { // offline mode
                let mut args = vec![
                    "-m", "pip", "install", "-r",
                    requirements_file.to_str().unwrap(),
                    "--upgrade", "--constraint", constrain_path,
                    "--no-index", "--find-links", wheel_dir.to_str().unwrap()
                ];
                command_executor::execute_command_with_env(
                    python_location.to_str().unwrap(),
                    &args,
                    vec![("VIRTUAL_ENV", venv_path.to_str().unwrap())],
                )
            } else {
                let mut args = vec![
                    "-m", "pip", "install", "-r",
                    requirements_file.to_str().unwrap(),
                    "--upgrade", "--constraint", constrain_path
                ];

                // Add PyPI mirror if specified
                if let Some(mirror_url) = pypi_mirror {
                    args.push("--index-url");
                    args.push(mirror_url.as_str());
                }

                command_executor::execute_command_with_env(
                    python_location.to_str().unwrap(),
                    &args,
                    vec![("VIRTUAL_ENV", venv_path.to_str().unwrap())],
                )
            } {
                Ok(out) => {
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            std::str::from_utf8(&out.stderr).unwrap().to_string(),
                        ))
                    }
                }
                Err(e) => Err(e),
            }
        }
        _ => {
            match if let Some(wheel_dir) = wheel_dir {
                command_executor::execute_command_with_env(
                  "bash",
                  &vec![
                      "-c",
                      &format!(
                          "{} -m pip install -r {} --upgrade --constraint {} --no-index --find-links {}",
                          shlex::quote(python_location.to_str().unwrap()),
                          shlex::quote(requirements_file.to_str().unwrap()),
                          shlex::quote(constrain_path),
                          shlex::quote(wheel_dir.to_str().unwrap())
                      ),
                  ],
                  vec![("VIRTUAL_ENV", venv_path.to_str().unwrap())],
                )
            } else {
                let mut cmd = format!(
                    "{} -m pip install -r {} --upgrade --constraint {}",
                    shlex::quote(python_location.to_str().unwrap()),
                    shlex::quote(requirements_file.to_str().unwrap()),
                    shlex::quote(constrain_path)
                );

                // Add PyPI mirror if specified
                if let Some(mirror_url) = pypi_mirror {
                    cmd.push_str(&format!(" --index-url {}", shlex::quote(mirror_url)));
                }

                command_executor::execute_command_with_env(
                    "bash",
                    &vec!["-c", &cmd],
                    vec![("VIRTUAL_ENV", venv_path.to_str().unwrap())],
                )
            } {
                Ok(out) => {
                    if out.status.success() {
                        trace!(
                            "pip install output: {}",
                            std::str::from_utf8(&out.stdout).unwrap()
                        );
                        Ok(())
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            std::str::from_utf8(&out.stderr).unwrap().to_string(),
                        ))
                    }
                }
                Err(e) => Err(e),
            }
        }
    }
}

/// Detects the Python version being used in the virtual environment
///
/// # Arguments
/// * `python_executable` - Path to the Python executable
///
/// # Returns
/// * `Result<String, String>` - Python version string (e.g., "3.11") or error
fn detect_python_version(python_executable: &str) -> Result<String, String> {
    use crate::command_executor::execute_command;

    match execute_command(python_executable, &["--version"]) {
        Ok(output) => {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                // Parse "Python 3.11.x" to get "3.11"
                if let Some(version_part) = version_output.split_whitespace().nth(1) {
                    let version_parts: Vec<&str> = version_part.split('.').collect();
                    if version_parts.len() >= 2 {
                        return Ok(format!("{}.{}", version_parts[0], version_parts[1]));
                    }
                }
                Err(format!("Could not parse Python version from: {}", version_output))
            } else {
                Err(format!("Failed to get Python version: {}", String::from_utf8_lossy(&output.stderr)))
            }
        }
        Err(e) => Err(format!("Failed to execute Python version check: {}", e))
    }
}

/// Finds the appropriate wheel directory for the detected Python version
///
/// # Arguments
/// * `offline_archive_dir` - Base offline archive directory
/// * `python_version` - Python version string (e.g., "3.11")
///
/// # Returns
/// * `Option<PathBuf>` - Path to the appropriate wheel directory, or None if not found
fn find_wheel_directory(offline_archive_dir: &Path, python_version: &str) -> Option<PathBuf> {
    // Try different naming formats
    let possible_formats = vec![
        format!("wheels_py{}", python_version.replace('.', "")),
        format!("wheels_py{}", python_version.replace('.', "_")),
        format!("wheels_py{}", python_version.replace(".", "_")),
    ];

    for format_name in possible_formats {
        let versioned_wheel_dir = offline_archive_dir.join(&format_name);
        debug!("Looking for wheels in: {}", versioned_wheel_dir.display());

        if versioned_wheel_dir.exists() {
            info!("Found Python {}-specific wheel directory: {}", python_version, versioned_wheel_dir.display());
            return Some(versioned_wheel_dir);
        }
    }

    // Fallback: try the old "wheels" directory for backward compatibility
    let legacy_wheel_dir = offline_archive_dir.join("wheels");
    if legacy_wheel_dir.exists() {
        warn!("Using legacy wheel directory (may not be compatible): {}", legacy_wheel_dir.display());
        return Some(legacy_wheel_dir);
    }

    // List available wheel directories for debugging
    if let Ok(entries) = std::fs::read_dir(offline_archive_dir) {
        let wheel_dirs: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_name().to_string_lossy().starts_with("wheels_")
            })
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();

        if !wheel_dirs.is_empty() {
            warn!("Available wheel directories: {:?}", wheel_dirs);
            warn!("Could not find wheels for Python {}, you might need to rebuild the offline archive with this Python version", python_version);
        }
    }

    None
}

/// Installs or updates the Python virtual environment for a specific ESP-IDF version.
///
/// This asynchronous function orchestrates the creation of a Python virtual environment,
/// downloads the necessary constraints file, and then installs all required Python
/// packages based on the ESP-IDF version and specified features. It can optionally
/// reinstall the environment if it already exists.
///
/// # Arguments
///
/// * `idf_version` - A string slice representing the ESP-IDF version (e.g., "v5.1", "master").
/// * `idf_tools_path` - A reference to a `Path` where ESP-IDF tools, including the
///   Python virtual environment, should be stored.
/// * `reinstall` - A boolean indicating whether to remove an existing virtual
///   environment before creating a new one. If `true`, the existing `venv` will be
///   deleted.
/// * `idf_path` - A reference to a `Path` pointing to the root directory of the
///   ESP-IDF installation, used to locate `requirements.txt` files.
/// * `features` - A slice of `String`s, where each string represents an additional
///   feature whose Python requirements should be installed (e.g., "esp_gh_action").
///   These correspond to files like `requirements_esp_gh_action.txt`.
/// * `offline_archive_dir` - Optional path to offline archive directory containing
///   pre-downloaded wheels and constraints files.
///
/// # Returns
///
/// A `Result<(), String>` which is:
/// - `Ok(())` if the Python environment was installed or updated successfully.
/// - `Err(String)` if any step of the installation process fails, containing
///   a descriptive error message.
///
/// # Errors
///
/// This function can return an error for various reasons, including but not limited to:
/// - Failure to create the virtual environment.
/// - Issues removing an existing virtual environment during a reinstall operation.
/// - Failure to download the constraints file.
/// - Failure to install any of the required Python packages from the `requirements.txt`
///   files using pip.
pub async fn install_python_env(
    paths: &VersionPaths,
    idf_version: &str,
    idf_tools_path: &Path,
    reinstall: bool,
    features: &[String],
    offline_archive_dir: Option<&Path>,
    pypi_mirror: &Option<String>
) -> Result<(), String> {
    let mut offline_mode = false;
    let venv_path = paths.python_venv_path.clone();

    // if reinstall is true, remove the existing venv
    if venv_path.exists() && reinstall {
        debug!("venv already exists, removing it");
        match std::fs::remove_dir_all(&venv_path) {
            Ok(_) => {
                debug!("venv removed");
            }
            Err(e) => {
                warn!("failed to remove venv: {}, trying to proceed nonetheless", e);
            }
        }
    }
    match ensure_path(venv_path.to_str().unwrap()){
        Ok(_) => {
            debug!("venv path ensured: {}", venv_path.display());
        }
        Err(e) => {
            error!("failed to ensure venv path: {}", e);
            return Err(format!("failed to ensure venv path: {}", e));
        }
    }
    if let Some(_offline_dir) = offline_archive_dir {
        offline_mode = true;
    } else {
        debug!("No offline archive directory provided, skipping copying contents.");
    }

    let python_executable = match std::env::consts::OS {
            "windows" => {
              if let Some(scoop_shims_path) = get_scoop_path() {
                // Use the Scoop shims path for the Python executable
                let python_executable_path = PathBuf::from(scoop_shims_path).join("python3.exe");
                match python_executable_path.try_exists() {
                    Ok(true) => python_executable_path.to_string_lossy().into_owned(),
                    Ok(false) => "python3.exe".to_string(),
                    Err(e) => {
                        warn!("Failed to check if Python executable exists: {}", e);
                        "python3.exe".to_string()
                    }
                }
              } else {
                "python3.exe".to_string()
              }
            },
            _ => "python3".to_string(),
        };

    // create the venv
    match create_python_venv(venv_path.to_str().unwrap(), &python_executable) {
        Ok(_) => {
            debug!("venv created");
        }
        Err(e) => {
            error!("failed to create venv: {}", e);
            return Err(format!("failed to create venv: {}", e));
        }
    }

    // install the requirements
    let mut requirements_file_list = vec![];

    let base_requirements_path = paths.idf_path.join("tools").join("requirements");
    requirements_file_list.push(base_requirements_path.join("requirements.core.txt"));
    // prepare list of requirements files
    for feature in features {
        let requirements_file =
            base_requirements_path.join(format!("requirements.{}.txt", feature));
        if requirements_file.exists() {
            requirements_file_list.push(requirements_file);
        } else {
            warn!(
                "requirements file not found: {}",
                requirements_file.display()
            );
        }
    }
    let constrains_idf_version = match parse_cmake_version(paths.idf_path.to_str().unwrap()) {
        Ok((maj,min)) => format!("v{}.{}", maj, min),
        Err(e) => {
            warn!("Failed to parse CMake version: {}", e);
            idf_version.to_string()
        }
    };

    let constraint_file = if offline_mode {
      let filename = format!("espidf.constraints.{}.txt", remove_after_second_dot(&constrains_idf_version));
      let src_path = offline_archive_dir.unwrap().join(filename.clone());
      let dest_path = idf_tools_path.join(filename.clone());
      fs::copy(
          src_path.clone(),
          dest_path.clone(),
      ).map_err(|e| format!("Failed to copy constraints file: {} error: {}", src_path.display(), e))?;
      Some(dest_path)
    } else {
      match download_constraints_file(idf_tools_path, &constrains_idf_version)
          .await
          .context("Failed to download constraints file")
      {
          Ok(constraint_file) => {
              info!("Downloaded constraints file: {}", constraint_file.display());
              Some(constraint_file)
          }
          Err(e) => {
              warn!("Failed to download constraints file: {}", e);
              None
          }
      }
    };

    // Determine the appropriate wheel directory for offline mode
    let wheel_dir = if offline_mode {
        let archive_dir = offline_archive_dir.unwrap();

        // Detect the Python version being used
        let python_version = detect_python_version(&python_executable)
            .map_err(|e| format!("Failed to detect Python version: {}", e))?;

        info!("Detected Python version: {}", python_version);

        // Find the appropriate wheel directory
        match find_wheel_directory(archive_dir, &python_version) {
            Some(wheel_path) => Some(wheel_path),
            None => {
                return Err(format!(
                    "No compatible wheel directory found for Python {}. Available wheel directories should be named like 'wheels_py311', 'wheels_py312', etc.",
                    python_version
                ));
            }
        }
    } else {
        None
    };

    // install the requirements from files
    for requirements_file in requirements_file_list {
        match pip_install_requirements(&venv_path, &requirements_file, &constraint_file, &wheel_dir, pypi_mirror) {
            Ok(_) => {
                debug!("requirements installed: {}", requirements_file.display());
            }
            Err(e) => {
                error!(
                    "failed to install requirements from file {:?}: {}",
                    requirements_file, e
                );
                return Err(format!(
                    "failed to install requirements from file {:?}: {}",
                    requirements_file, e
                ));
            }
        }
    }
    info!("Python environment installed successfully");
    Ok(())
}

/// Runs the IDF tools Python installation script.
///
/// This function prepares the environment to run a Python installation script for
/// IDF tools by ensuring that the path is properly escaped based on the operating
/// system. It then executes the installation script followed by the Python environment
/// setup script.
///
/// # Parameters
///
/// - `idf_tools_path`: A string slice that represents the path to the IDF tools.
/// - `environment_variables`: A vector of tuples containing environment variable names
///   and their corresponding values, which will be passed to the installation scripts.
///
/// # Returns
///
/// This function returns a `Result<String, String>`. On success, it returns an `Ok`
/// containing the output of the Python environment setup script. On failure, it returns
/// an `Err` containing an error message.
///
/// # Example
///
/// ```rust
/// use idf_im_lib::python_utils::run_idf_tools_py;
/// let path = "path/to/idf_tools";
/// let env_vars = vec![("VAR_NAME".to_string(), "value".to_string())];
/// match run_idf_tools_py(path, &env_vars) {
///     Ok(output) => println!("Success: {}", output),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn run_idf_tools_py(
    // todo: rewrite functionality to rust
    idf_tools_path: &str,
    environment_variables_install: &Vec<(String, String)>,
    environment_variables_python: &Vec<(String, String)>,
) -> Result<String, String> {
    run_idf_tools_py_with_features(
        idf_tools_path,
        environment_variables_install,
        environment_variables_python,
        &[],
    )
}

pub fn run_idf_tools_py_with_features(
    idf_tools_path: &str,
    _environment_variables_install: &Vec<(String, String)>,
    environment_variables_python: &Vec<(String, String)>,
    features: &[String],
) -> Result<String, String> {
    let escaped_path = if std::env::consts::OS == "windows" {
        replace_unescaped_spaces_win(idf_tools_path)
    } else {
        replace_unescaped_spaces_posix(idf_tools_path)
    };
    run_install_python_env_script_with_features(
        &escaped_path,
        environment_variables_python,
        features,
    )
}

fn run_install_python_env_script(
    idf_tools_path: &str,
    environment_variables: &Vec<(String, String)>,
) -> Result<String, String> {
    let output =
        run_install_python_env_script_with_features(idf_tools_path, environment_variables, &[]);

    trace!("idf_tools.py install-python-env output:\n{:?}", output);

    output
}

fn run_install_python_env_script_with_features(
    idf_tools_path: &str,
    environment_variables: &Vec<(String, String)>,
    features: &[String],
) -> Result<String, String> {
    let mut args = "install-python-env".to_string();
    if !features.is_empty() {
        args = format!("{} --features {}", args, features.join(","));
    }
    let output = run_python_script_from_file(
        idf_tools_path,
        Some(&args),
        None,
        Some(environment_variables),
    );

    trace!("idf_tools.py install-python-env output:\n{:?}", output);

    output
}

/// Executes a Python script using the provided Python interpreter and returns the script's output.
///
/// # Parameters
///
/// * `script` - A reference to a string representing the Python script to be executed.
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
///   If `None`, the function will default to using "python3".
///
/// # Returns
///
/// * `Result<String, String>` - On success, returns a `Result` containing the standard output of the Python script as a string.
///   On error, returns a `Result` containing the standard error of the Python script as a string.
pub fn run_python_script(script: &str, python: Option<&str>) -> Result<String, String> {
    let output = command_executor::execute_command(python.unwrap_or("python3"), &["-c", script]);
    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(std::str::from_utf8(&out.stdout).unwrap().to_string())
            } else {
                Err(std::str::from_utf8(&out.stderr).unwrap().to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Performs a series of sanity checks for the Python interpreter.
///
/// This function executes various Python scripts and checks for the availability of essential Python modules,
/// such as pip, venv, and the standard library. It also verifies the functionality of the ctypes module.
///
/// # Parameters
///
/// * `python` - An optional reference to a string representing the Python interpreter to be used.
///   If `None`, the function will default to using "python3".
///
/// # Returns
///
/// * `Vec<Result<String, String>>` - A vector of results. Each result represents the output or error message
///   of a specific Python script execution. If the script execution is successful, the result will be `Ok`
///   containing the standard output as a string. If the script execution fails, the result will be `Err`
///   containing the standard error as a string.
pub fn python_sanity_check(python: Option<&str>) -> Vec<Result<String, String>> {
    let mut outputs = Vec::new();
    // Check Python version
    let version_output = command_executor::execute_command(
        python.unwrap_or("python3"),
        &["--version"],
    );
    match version_output {
        Ok(out) if out.status.success() => {
            let version_str = String::from_utf8_lossy(&out.stdout)
                .trim()
                .replace("Python ", "");
            match Version::parse(&version_str) {
                Ok(version) => {
                    let req = VersionReq::parse(">=3.10.0, <3.14.0").unwrap();
                    if req.matches(&version) {
                        outputs.push(Ok(format!("Python version {} is supported", version)));
                    } else {
                        outputs.push(Err(format!(
                            "Python version {} is not supported. Required: >=3.10.0, <3.14.0",
                            version
                        )));
                    }
                }
                Err(_) => outputs.push(Err("Failed to parse Python version".to_string())),
            }
        }
        Ok(out) => outputs.push(Err(String::from_utf8_lossy(&out.stderr).to_string())),
        Err(e) => outputs.push(Err(e.to_string())),
    }
    // check pip
    let output =
        command_executor::execute_command(python.unwrap_or("python3"), &["-m", "pip", "--version"]);
    match output {
        Ok(out) => {
            if out.status.success() {
                outputs.push(Ok(String::from_utf8_lossy(&out.stdout).to_string()));
            } else {
                outputs.push(Err(String::from_utf8_lossy(&out.stderr).to_string()));
            }
        }
        Err(e) => outputs.push(Err(e.to_string())),
    }
    // check venv
    let output_2 =
        command_executor::execute_command(python.unwrap_or("python3"), &["-m", "venv", "-h"]);
    match output_2 {
        Ok(out) => {
            if out.status.success() {
                outputs.push(Ok(String::from_utf8_lossy(&out.stdout).to_string()));
            } else {
                outputs.push(Err(String::from_utf8_lossy(&out.stderr).to_string()));
            }
        }
        Err(e) => outputs.push(Err(e.to_string())),
    }
    // check standard library
    let script = include_str!("../../python_scripts/sanity_check/import_standard_library.py");
    outputs.push(run_python_script(script, python));
    // check ctypes
    let script = include_str!("../../python_scripts/sanity_check/ctypes_check.py");
    outputs.push(run_python_script(script, python));
    // check https
    let script = include_str!("../../python_scripts/sanity_check/import_standard_library.py");
    outputs.push(run_python_script(script, python));
    outputs
}

#[cfg(feature = "userustpython")]
pub fn run_python_script_with_rustpython(script: &str) -> String {
    vm::Interpreter::without_stdlib(Default::default()).enter(|vm| {
        let scope = vm.new_scope_with_builtins();
        let code_opbject = vm
            .compile(script, vm::compiler::Mode::Exec, "<embeded>".to_owned())
            .map_err(|err| format!("error: {:?}", err))
            .unwrap();
        let output = vm.run_code_obj(code_opbject, scope).unwrap();
        format!("output: {:?}", output)
        // Ok(output)
    });
    "".to_string()
}

#[cfg(feature = "userustpython")]
pub fn py_main_idf(interp: &Interpreter) -> vm::PyResult<PyStrRef> {
    interp.enter(|vm| {
        // Add local library path
        vm.insert_sys_path(vm.new_pyobj("examples"))
            .expect("add examples to sys.path failed, why?");

        // select the idf_tools module
        let module = vm.import("idf_tools", 0)?;
        // running straight the action_install
        let name_func = module.get_attr("action_install", vm)?;
        // we will get the params from the user in the future
        let quiet = vm.ctx.false_value.clone();
        let non_interactive = vm.ctx.new_bool(false);
        let tools_json = vm.ctx.new_str("./examples/tools.json");
        let idf_path = vm.ctx.none();
        let tools = vm.ctx.new_list(vec![vm.ctx.new_str("all").into()]);
        let targets = vm.ctx.new_str("all");

        let pos_args: PosArgs = PosArgs::new(vec![
            quiet.into(),
            non_interactive.into(),
            tools_json.into(),
            idf_path,
            tools.into(),
            targets.into(),
        ]);

        let result = name_func.call(pos_args, vm)?;
        let result_str = result.str(vm)?;
        let result_pystrref: PyStrRef = result_str;
        // let result: PyStrRef = result.get_attr("name", vm)?.try_into_value(vm)?;
        vm::PyResult::Ok(result_pystrref)
    })
}

#[cfg(feature = "userustpython")]
// in the future we will accept params what to actually install ;-)
pub fn run_idf_tools() -> ExitCode {
    let mut settings = vm::Settings::default();
    settings.path_list.push("Lib".to_owned()); // addng folder lib in current directory
    if let Ok(path) = env::var("RUSTPYTHONPATH") {
        settings
            .path_list
            .extend(path.split(':').map(|s| s.to_owned()));
    }
    let interp = vm::Interpreter::with_init(settings, |vm| {
        vm.add_native_modules(rustpython_stdlib::get_module_inits());
    });

    let result = py_main_idf(&interp);
    let result = result.map(|result| {
        println!("name: {result}");
    });
    ExitCode::from(interp.run(|_vm| result))
}
