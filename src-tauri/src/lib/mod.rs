use flate2::read::GzDecoder;
use git2::{FetchOptions, ObjectType, RemoteCallbacks, Repository, SubmoduleUpdateOptions};
use log::{error, info, trace, warn};
use reqwest::Client;
#[cfg(feature = "userustpython")]
use rustpython_vm::literal::char;
use sha2::{Digest, Sha256};
use std::io::BufReader;
use tar::Archive;
use tera::{Context, Tera};
use thiserror::Error;
use utils::{find_directories_by_name, make_long_path_compatible};
use zip::ZipArchive;

pub mod command_executor;
pub mod idf_config;
pub mod idf_tools;
pub mod idf_versions;
pub mod python_utils;
pub mod settings;
pub mod system_dependencies;
pub mod utils;
pub mod version_manager;
use std::fs::{set_permissions, File};
use std::{
    env,
    fs::{self},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

/// Creates an executable shell script with the given content and file path.
///
/// # Parameters
///
/// * `file_path`: A string representing the path where the shell script should be created.
/// * `content`: A string representing the content of the shell script.
///
/// # Return
///
/// * `Result<(), String>`: On success, returns `Ok(())`. On error, returns `Err(String)` containing the error message.
fn create_executable_shell_script(file_path: &str, content: &str) -> Result<(), String> {
    if std::env::consts::OS == "windows" {
        unimplemented!("create_executable_shell_script not implemented for Windows")
    } else {
        // Create and write to the file
        let mut file = File::create(file_path).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Set the file as executable (mode 0o755)
            let permissions = PermissionsExt::from_mode(0o755);
            set_permissions(Path::new(file_path), permissions).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Formats a vector of key-value pairs into a bash-compatible format for environment variables.
///
/// # Parameters
///
/// * `pairs` - A reference to a vector of tuples, where each tuple contains a key (String) and a value (String).
///
/// # Return
///
/// * A String representing the formatted environment variable pairs in bash-compatible format.
///   Each pair is enclosed in double quotes and separated by a newline.
///
fn format_bash_env_pairs(pairs: &[(String, String)]) -> String {
    let formatted_pairs: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("    \"{}:{}\"", key, value))
        .collect();

    format!("env_var_pairs=(\n{}\n)", formatted_pairs.join("\n"))
}

/// Formats a vector of key-value pairs into a PowerShell-compatible format for environment variables.
///
/// # Parameters
///
/// * `pairs`: A reference to a vector of tuples, where each tuple contains a key-value pair.
///
/// # Return
///
/// * A string representing the formatted environment variables in PowerShell-compatible format.
///
fn format_powershell_env_pairs(pairs: &[(String, String)]) -> String {
    let formatted_pairs: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("    \"{}\" = \"{}\"", key, value))
        .collect();

    format!("$env_var_pairs = @{{\n{}\n}}", formatted_pairs.join("\n"))
}

/// Creates an activation shell script for the ESP-IDF toolchain.
///
/// # Parameters
///
/// * `file_path`: A string representing the path where the activation script should be created.
/// * `idf_path`: A string representing the path to the ESP-IDF installation.
/// * `idf_tools_path`: A string representing the path to the ESP-IDF tools installation.
/// * `idf_version`: A string representing the version of the ESP-IDF toolchain.
/// * `export_paths`: A vector of strings representing additional paths to be added to the shell's PATH environment variable.
///
/// # Return
///
/// * `Result<(), String>`: On success, returns `Ok(())`. On error, returns `Err(String)` containing the error message.
pub fn create_activation_shell_script(
    file_path: &str,
    idf_path: &str,
    idf_tools_path: &str,
    idf_python_env_path: Option<&str>,
    idf_version: &str,
    export_paths: Vec<String>,
    env_var_pairs: Vec<(String, String)>,
) -> Result<(), String> {
    ensure_path(file_path).map_err(|e| e.to_string())?;
    let mut filename = PathBuf::from(file_path);
    filename.push(format!("activate_idf_{}.sh", idf_version));
    let template = include_str!("../../bash_scripts/activate_idf_template.sh");
    let mut tera = Tera::default();
    if let Err(e) = tera.add_raw_template("activate_idf_template", template) {
        error!("Failed to add template: {}", e);
        return Err(e.to_string());
    }
    let mut context = Context::new();
    let env_var_pairs_str = format_bash_env_pairs(&env_var_pairs);
    context.insert("env_var_pairs", &env_var_pairs_str);
    context.insert("idf_path", &idf_path);
    context.insert(
        "idf_path_escaped",
        &replace_unescaped_spaces_posix(idf_path),
    );

    context.insert("idf_tools_path", &idf_tools_path);
    context.insert(
        "idf_tools_path_escaped",
        &replace_unescaped_spaces_posix(idf_tools_path),
    );
    context.insert("idf_version", &idf_version);
    context.insert("addition_to_path", &export_paths.join(":"));
    if let Some(idf_python_env_path) = idf_python_env_path {
        context.insert("idf_python_env_path", &idf_python_env_path);
        context.insert(
            "idf_python_env_path_escaped",
            &replace_unescaped_spaces_posix(idf_python_env_path),
        );
    } else {
        context.insert("idf_python_env_path", &format!("{}/python", idf_tools_path));
        context.insert(
            "idf_python_env_path_escaped",
            &replace_unescaped_spaces_posix(&format!("{}/python", idf_tools_path)),
        );
    }
    let rendered = match tera.render("activate_idf_template", &context) {
        Err(e) => {
            error!("Failed to render template: {}", e);
            return Err(e.to_string());
        }
        Ok(text) => text,
    };

    create_executable_shell_script(filename.to_str().unwrap(), &rendered)?;
    Ok(())
}

// TODO: unify the replace_unescaped_spaces functions
pub fn replace_unescaped_spaces_posix(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&' ') {
            // If we see a backslash followed by a space, keep them as-is
            result.push(ch);
            result.push(chars.next().unwrap());
        } else if ch == ' ' {
            // If we see a space not preceded by a backslash, replace it
            result.push_str(r"\ ");
        } else {
            // For all other characters, just add them to the result
            result.push(ch);
        }
    }

    result
}

pub fn replace_unescaped_spaces_win(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '`' && chars.peek() == Some(&' ') {
            result.push(ch);
            result.push(chars.next().unwrap());
        } else if ch == ' ' {
            result.push_str(r"` ");
        } else {
            result.push(ch);
        }
    }

    result
}

/// Runs a PowerShell script and captures its output.
/// TODO: fix documentation
///
/// # Parameters
///
/// * `script`: A string containing the PowerShell script to be executed.
///
/// # Returns
///
/// * `Ok(String)`: If the PowerShell script executes successfully, the function returns a `Result` containing the script's output as a string.
/// * `Err(Box<dyn std::error::Error>)`: If an error occurs during the execution of the PowerShell script, the function returns a `Result` containing the error.
pub fn run_powershell_script(script: &str) -> Result<String, std::io::Error> {
    match std::env::consts::OS {
        "windows" => match command_executor::get_executor().run_script_from_string(script) {
            Ok(output) => {
                trace!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                trace!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                String::from_utf8(output.stdout)
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
            }
            Err(err) => Err(err),
        },
        _ => {
            let error_message = "run_powershell_script is only supported on Windows.";
            error!("{}", error_message);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                error_message,
            ))
        }
    }
}

/// Creates a PowerShell profile script for the ESP-IDF tools.
///
/// # Parameters
///
/// * `profile_path` - A string representing the path where the PowerShell profile script should be created.
/// * `idf_path` - A string representing the path to the ESP-IDF repository.
/// * `idf_tools_path` - A string representing the path to the ESP-IDF tools directory.
///
/// # Returns
///
/// * `Result<String, std::io::Error>` - On success, returns the path to the created PowerShell profile script.
///   On error, returns an `std::io::Error` indicating the cause of the error.
fn create_powershell_profile(
    profile_path: &str,
    idf_path: &str,
    idf_tools_path: &str,
    idf_python_env_path: Option<&str>,
    idf_version: &str,
    export_paths: Vec<String>,
    env_var_pairs: Vec<(String, String)>,
) -> Result<String, std::io::Error> {
    let profile_template = include_str!("../../powershell_scripts/idf_tools_profile_template.ps1");

    let mut tera = Tera::default();
    if let Err(e) = tera.add_raw_template("powershell_profile", profile_template) {
        error!("Failed to add template: {}", e);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to add template",
        ));
    }
    ensure_path(profile_path).expect("Unable to create directory");
    let mut context = Context::new();
    println!("idf_path: {}", replace_unescaped_spaces_win(idf_path));
    context.insert("idf_path", &replace_unescaped_spaces_win(idf_path));
    context.insert("idf_version", &idf_version);
    context.insert(
        "env_var_pairs",
        &format_powershell_env_pairs(&env_var_pairs),
    );

    context.insert(
        "idf_tools_path",
        &replace_unescaped_spaces_win(idf_tools_path),
    );
    if let Some(idf_python_env_path) = idf_python_env_path {
        context.insert(
            "idf_python_env_path",
            &replace_unescaped_spaces_win(idf_python_env_path),
        );
    } else {
        context.insert(
            "idf_python_env_path",
            &replace_unescaped_spaces_win(&format!("{}\\python", idf_tools_path)),
        );
    }
    context.insert("add_paths_extras", &export_paths.join(";"));
    let mut rendered = match tera.render("powershell_profile", &context) {
        Err(e) => {
            error!("Failed to render template: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to render template",
            ));
        }
        Ok(text) => text,
    };

    if std::env::consts::OS == "windows" {
        rendered = rendered.replace("\r\n", "\n").replace("\n", "\r\n");
    }
    let mut filename = PathBuf::from(profile_path);
    filename.push("Microsoft.PowerShell_profile.ps1");
    fs::write(&filename, rendered).expect("Unable to write file");
    Ok(filename.display().to_string())
}

/// Creates a desktop shortcut for the IDF tools using PowerShell on Windows.
///
/// # Parameters
///
/// * `idf_path` - A string representing the path to the ESP-IDF repository.
/// * `idf_tools_path` - A string representing the path to the IDF tools directory.
///
/// # Return Value
///
/// * `Result<String, std::io::Error>` - On success, returns a string indicating the output of the PowerShell script.
///   On error, returns an `std::io::Error` indicating the cause of the error.
pub fn create_desktop_shortcut(
    profile_path: &str,
    idf_path: &str,
    idf_version: &str,
    idf_tools_path: &str,
    idf_python_env_path: Option<&str>,
    export_paths: Vec<String>,
    env_var_pairs: Vec<(String, String)>,
) -> Result<String, std::io::Error> {
    match std::env::consts::OS {
        "windows" => {
            let filename = match create_powershell_profile(
                profile_path,
                idf_path,
                idf_tools_path,
                idf_python_env_path,
                idf_version,
                export_paths,
                env_var_pairs,
            ) {
                Ok(filename) => filename,
                Err(err) => {
                    error!("Failed to create PowerShell profile: {}", err);
                    return Err(err);
                }
            };
            let icon = include_bytes!("../../icons/eim.ico");
            let mut home = dirs::home_dir().unwrap();
            home.push("Icons");
            let _ = ensure_path(home.to_str().unwrap());
            home.push("eim.ico");
            fs::write(&home, icon).expect("Unable to write file");
            let powershell_script_template =
                include_str!("../../powershell_scripts/create_desktop_shortcut_template.ps1");
            // Create a new Tera instance
            let mut tera = Tera::default();
            if let Err(e) = tera.add_raw_template("powershell_script", powershell_script_template) {
                error!("Failed to add template: {}", e);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to add template",
                ));
            }
            let mut context = Context::new();
            context.insert("custom_profile_filename", &filename);
            context.insert("name", &idf_version);
            let rendered = match tera.render("powershell_script", &context) {
                Err(e) => {
                    error!("Failed to render template: {}", e);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Failed to render template",
                    ));
                }
                Ok(text) => text,
            };

            let output = match run_powershell_script(&rendered) {
                Ok(o) => o,
                Err(err) => {
                    error!("Failed to execute PowerShell script: {}", err);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Failed to execute PowerShell script",
                    ));
                }
            };

            Ok(output)
        }
        _ => {
            warn!("Creating desktop shortcut is only supported on Windows.");
            Ok("Unimplemented on this platform.".to_string())
        }
    }
}

/// Retrieves the path to the local data directory for storing logs.
///
/// This function uses the `dirs` crate to find the appropriate directory for storing logs.
/// If the local data directory is found, it creates a subdirectory named "logs" within it.
/// If the directory creation fails, it returns an error.
///
/// # Returns
///
/// * `Some(PathBuf)` if the local data directory and log directory are successfully created.
/// * `None` if the local data directory cannot be determined.
///
pub fn get_log_directory() -> Option<PathBuf> {
    // Use the dirs crate to find the local data directory
    dirs::data_local_dir().map(|data_dir| {
        // Create a subdirectory named "logs" within the local data directory
        let log_dir = data_dir.join("eim").join("logs");

        // Attempt to create the log directory
        std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

        // Return the path to the log directory
        log_dir
    })
}
/// Verifies the SHA256 checksum of a file against an expected checksum.
///
/// # Arguments
///
/// * `expected_checksum` - A string representing the expected SHA256 checksum.
/// * `file_path` - A string representing the path to the file to be verified.
///
/// # Returns
///
/// * `Ok(true)` if the file's checksum matches the expected checksum.
/// * `Ok(false)` if the file does not exist or its checksum does not match the expected checksum.
/// * `Err(io::Error)` if an error occurs while opening or reading the file.
pub fn verify_file_checksum(expected_checksum: &str, file_path: &str) -> Result<bool, io::Error> {
    if !Path::new(file_path).exists() {
        return Ok(false);
    }

    let mut file = File::open(file_path)?;

    let mut hasher = Sha256::new();

    let mut buffer = [0; 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    // Get the final hash
    let result = hasher.finalize();

    // Convert the hash to a hexadecimal string
    let computed_checksum = format!("{:x}", result);

    // Compare the computed checksum with the expected checksum
    Ok(computed_checksum == expected_checksum)
}

/// Sets up the environment variables required for the ESP-IDF build system.
///
/// # Parameters
///
/// * `tool_install_directory`: A reference to a `PathBuf` representing the directory where the ESP-IDF tools are installed.
/// * `idf_path`: A reference to a `PathBuf` representing the path to the ESP-IDF framework directory.
///
/// # Return
///
/// * `Result<Vec<(String, String)>, String>`:
///   - On success, returns a `Vec` of tuples containing the environment variable names and their corresponding values.
///   - On error, returns a `String` describing the error.
///
pub fn setup_environment_variables(
    tool_install_directory: &PathBuf,
    idf_path: &PathBuf,
) -> Result<Vec<(String, String)>, String> {
    let mut env_vars = vec![];

    // env::set_var("IDF_TOOLS_PATH", tool_install_directory);
    let instal_dir_string = tool_install_directory.to_str().unwrap().to_string();
    env_vars.push(("IDF_TOOLS_PATH".to_string(), instal_dir_string));
    let idf_path_string = idf_path.to_str().unwrap().to_string();
    env_vars.push(("IDF_PATH".to_string(), idf_path_string));
    env_vars.push((
        "ESP_ROM_ELF_DIR".to_string(),
        get_elf_rom_dir(tool_install_directory)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
    ));
    env_vars.push((
        "OPENOCD_SCRIPTS".to_string(),
        get_openocd_scripts_folder(tool_install_directory).unwrap(),
    ));

    let python_env_path_string = tool_install_directory
        .join("python")
        .to_str()
        .unwrap()
        .to_string();
    env_vars.push(("IDF_PYTHON_ENV_PATH".to_string(), python_env_path_string));

    Ok(env_vars)
}

/// Retrieves the path to the ELF (Executable and Linkable Format) ROM directory.
///
/// # Parameters
///
/// * `idf_tools_path` - A reference to a `PathBuf` representing the path to the IDF tools directory.
///
/// # Return
///
/// * `Result<PathBuf, std::io::Error>` - On success, returns a `PathBuf` representing the path to the ELF ROM directory.
///   On error, returns a `std::io::Error` indicating the cause of the error.
fn get_elf_rom_dir(idf_tools_path: &PathBuf) -> Result<PathBuf, std::io::Error> {
    let elf_rom_dir = idf_tools_path.join("esp-rom-elfs");
    if elf_rom_dir.exists() {
        let mut subdirs = vec![];
        // Read the entries in the elf_rom_dir
        for entry in fs::read_dir(&elf_rom_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Check if the entry is a directory and add it to the vector
            if path.is_dir() {
                subdirs.push(path);
            }
        }

        // Sort the subdirectories
        subdirs.sort();
        if let Some(last_subdir) = subdirs.last() {
            log::info!("ELF ROM directory found: {}", last_subdir.display());
            return Ok(last_subdir.clone());
        } else {
            log::warn!("No ELF ROM directories found in {}", elf_rom_dir.display());
        }
    } else {
        log::warn!("No ELF ROM directories found in {}", elf_rom_dir.display());
    }
    Ok(elf_rom_dir)
}

/// Retrieves the path to the OpenOCD scripts folder within the IDF tools directory.
///
/// # Parameters
///
/// * `idf_tools_path` - A reference to a `PathBuf` representing the path to the IDF tools directory.
///
/// # Return
///
/// * `Result<PathBuf, std::io::Error>` - On success, returns a `PathBuf` representing the path to the OpenOCD scripts folder.
///   On error, returns a `std::io::Error` indicating the cause of the error.
fn get_openocd_scripts_folder(idf_tools_path: &PathBuf) -> Result<String, std::io::Error> {
    let search_path = idf_tools_path.join("openocd-esp32");

    let result = find_directories_by_name(&search_path, "scripts");

    if result.is_empty() {
        log::warn!("No OpenOCD scripts found in {}", search_path.display());
        return Ok(String::new());
    } else if result.len() > 1 {
        log::warn!(
            "Multiple OpenOCD scripts found in {}, using the first one",
            search_path.display()
        );
    }

    Ok(result[0].clone())
}

pub enum DownloadProgress {
    Progress(u64, u64), // (downloaded, total)
    Complete,
    Error(String),
}

pub async fn download_file(
    url: &str,
    destination_path: &str,
    progress_sender: Sender<DownloadProgress>,
) -> Result<(), std::io::Error> {
    // Create a new HTTP client
    let client = Client::new();

    // Send a GET request to the specified URL
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Get the total size of the file being downloaded
    let total_size = response.content_length().ok_or_else(|| {
        let _ = progress_sender.send(DownloadProgress::Error(
            "Failed to get content length".into(),
        ));
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to get content length")
    })?;
    log::debug!("Downloading {} to {}", url, destination_path);

    // Extract the filename from the URL
    let filename = Path::new(&url).file_name().unwrap().to_str().unwrap();
    log::debug!(
        "Filename: {} and destination: {}",
        filename,
        destination_path
    );
    // Create a new file at the specified destination path
    let mut file = File::create(Path::new(&destination_path).join(Path::new(filename)))?;
    log::debug!("Created file at {}", destination_path);

    // Initialize the amount downloaded
    let mut downloaded: u64 = 0;

    // Download the file in chunks
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    {
        // Update the amount downloaded
        downloaded += chunk.len() as u64;

        // Write the chunk to the file
        file.write_all(&chunk)?;

        // Call the progress callback function
        if let Err(e) = progress_sender.send(DownloadProgress::Progress(downloaded, total_size)) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send progress: {}", e),
            ));
        }
    }
    let _ = progress_sender.send(DownloadProgress::Complete);

    // Return Ok(()) if the download was successful
    Ok(())
}

#[derive(Error, Debug)]
pub enum DecompressionError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Unsupported archive format")]
    UnsupportedFormat,
}

/// Decompresses an archive file into the specified destination directory.
///
/// # Parameters
///
/// * `archive_path`: A string representing the path to the archive file to be decompressed.
/// * `destination_path`: A string representing the path to the directory where the archive should be decompressed.
///
/// # Returns
///
/// * `Ok(())` if the decompression is successful.
/// * `Err(DecompressionError)` if an error occurs during the decompression process.
///
/// # Errors
///
/// This function can return the following errors:
///
/// * `DecompressionError::Io`: An error occurred while performing I/O operations.
/// * `DecompressionError::Zip`: An error occurred while decompressing a ZIP archive.
/// * `DecompressionError::UnsupportedFormat`: The specified archive format is not supported.
pub fn decompress_archive(
    archive_path: &str,
    destination_path: &str,
) -> Result<(), DecompressionError> {
    let archive_path_long = make_long_path_compatible(archive_path);
    let archive_path = Path::new(&archive_path_long);
    let destination_path_long = make_long_path_compatible(destination_path);
    let destination_path = Path::new(&destination_path_long);

    if !destination_path.exists() {
        std::fs::create_dir_all(destination_path)?;
    }

    match archive_path.extension().and_then(|ext| ext.to_str()) {
        Some("zip") => decompress_zip(archive_path, destination_path),
        Some("tar") => decompress_tar(archive_path, destination_path),
        Some("gz") | Some("tgz") => {
            if archive_path.to_str().unwrap_or("").ends_with(".tar.gz")
                || archive_path.extension().unwrap() == "tgz"
            {
                decompress_tar_gz(archive_path, destination_path)
            } else {
                Err(DecompressionError::UnsupportedFormat)
            }
        }
        Some("xz") => {
            if archive_path.to_str().unwrap_or("").ends_with(".tar.xz") {
                decompress_tar_xz(archive_path, destination_path)
            } else {
                Err(DecompressionError::UnsupportedFormat)
            }
        }
        _ => Err(DecompressionError::UnsupportedFormat),
    }
}

fn decompress_zip(archive_path: &Path, destination_path: &Path) -> Result<(), DecompressionError> {
    log::info!(
        "Decompressing {} to {}",
        archive_path.display(),
        destination_path.display()
    );

    if !Path::new(archive_path).exists() {
        log::error!("File does not exist.");
        return Err(DecompressionError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "Archive file not found",
        )));
    }

    // First, try using ZipArchive for all platforms
    let zip_result = (|| {
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => destination_path.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    })();

    // If ZipArchive failed and we're on Windows, fall back to PowerShell
    if let Err(err) = zip_result {
        if std::env::consts::OS == "windows" {
            log::warn!(
                "ZipArchive decompression failed: {}. Falling back to PowerShell approach.",
                err
            );

            let executor = crate::command_executor::get_executor();
            let archive_path_str = archive_path.to_string_lossy().to_string();
            let destination_path_str = destination_path.to_string_lossy().to_string();

            // Create a separate thread to run the PowerShell command
            let handle = std::thread::spawn(move || {
                let script = format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive_path_str, destination_path_str
                );

                executor.run_script_from_string(&script)
            });

            // Wait for the thread to complete
            match handle.join() {
                Ok(result) => match result {
                    Ok(output) => {
                        if !output.status.success() {
                            let error_message = String::from_utf8_lossy(&output.stderr);
                            log::error!("PowerShell decompression failed: {}", error_message);
                            return Err(DecompressionError::Io(io::Error::new(
                                io::ErrorKind::Other,
                                format!("PowerShell decompression failed: {}", error_message),
                            )));
                        }
                        Ok(())
                    }
                    Err(e) => {
                        log::error!("Failed to execute PowerShell command: {}", e);
                        Err(DecompressionError::Io(e))
                    }
                },
                Err(e) => {
                    log::error!("Thread panicked: {:?}", e);
                    Err(DecompressionError::Io(io::Error::new(
                        io::ErrorKind::Other,
                        "Thread panicked during decompression",
                    )))
                }
            }
        } else {
            // On non-Windows platforms, just return the original error
            Err(err)
        }
    } else {
        // ZipArchive succeeded
        Ok(())
    }
}

/// Decompresses a TAR archive into the specified destination directory.
///
/// # Parameters
///
/// * `archive_path`: A reference to a `Path` representing the path to the TAR archive.
/// * `destination_path`: A reference to a `Path` representing the destination directory where the archive should be decompressed.
///
/// # Return Value
///
/// * `Result<(), DecompressionError>`: On success, returns `Ok(())`. On error, returns a `DecompressionError` indicating the cause of the error.
fn decompress_tar(archive_path: &Path, destination_path: &Path) -> Result<(), DecompressionError> {
    let file = File::open(archive_path)?;
    let mut archive = Archive::new(file);
    archive.unpack(destination_path)?;
    Ok(())
}

/// Decompresses a TAR.GZ archive into the specified destination directory.
///
/// # Parameters
///
/// * `archive_path`: A reference to a `Path` representing the path to the TAR.GZ archive.
/// * `destination_path`: A reference to a `Path` representing the destination directory where the archive should be decompressed.
///
/// # Return Value
///
/// * `Result<(), DecompressionError>`: On success, returns `Ok(())`. On error, returns a `DecompressionError` indicating the cause of the error.
fn decompress_tar_gz(
    archive_path: &Path,
    destination_path: &Path,
) -> Result<(), DecompressionError> {
    let file = File::open(archive_path)?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);
    archive.unpack(destination_path)?;
    Ok(())
}

/// Decompresses a TAR.XZ archive into the specified destination directory.
///
/// # Parameters
///
/// * `archive_path`: A reference to a `Path` representing the path to the TAR.XZ archive.
/// * `destination_path`: A reference to a `Path` representing the destination directory where the archive should be decompressed.
///
/// # Returns
///
/// * `Result<(), DecompressionError>`: On success, returns `Ok(())`. On error, returns a `DecompressionError` indicating the cause of the error.
fn decompress_tar_xz(
    archive_path: &Path,
    destination_path: &Path,
) -> Result<(), DecompressionError> {
    let file = File::open(archive_path)?;
    let mut reader = BufReader::new(file);
    let mut decompressed_data = Vec::new();

    // First decompress the XZ data
    lzma_rs::xz_decompress(&mut reader, &mut decompressed_data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Then process the tar archive from the decompressed data
    let cursor = std::io::Cursor::new(decompressed_data);
    let mut archive = Archive::new(cursor);
    archive.unpack(destination_path)?;
    Ok(())
}

/// Ensures that a directory exists at the specified path.
/// If the directory does not exist, it will be created.
///
/// # Arguments
///
/// * `directory_path` - A string representing the path to the directory to be ensured.
///
/// # Returns
///
/// * `Ok(())` if the directory was successfully created or already exists.
/// * `Err(std::io::Error)` if an error occurred while creating the directory.
pub fn ensure_path(directory_path: &str) -> std::io::Result<()> {
    let path = Path::new(directory_path);
    if !path.exists() {
        // If the directory does not exist, create it
        fs::create_dir_all(directory_path)?;
    }
    Ok(())
}

/// Adds a directory to the system's PATH environment variable.
/// If the directory is already present in the PATH, it will not be added again.
///
/// # Arguments
///
/// * `directory_path` - A string representing the path of the directory to be added to the PATH.
///
/// # Example
///
/// ```rust
/// use idf_im_lib::add_path_to_path;
///
/// add_path_to_path("/usr/local/bin");
/// ```
pub fn add_path_to_path(directory_path: &str) {
    // Retrieve the current PATH environment variable.
    // If it does not exist, use an empty string as the default value.
    let current_path = env::var("PATH").unwrap_or_default();

    // Check if the directory path is already present in the PATH.
    // If it is not present, construct a new PATH string with the directory path added.
    if !current_path.contains(directory_path) {
        let new_path = if current_path.is_empty() {
            directory_path.to_owned()
        } else {
            format!("{};{}", current_path, directory_path)
        };

        // Set the new PATH environment variable.
        env::set_var("PATH", new_path);
    }
}

/// Messages that can be sent to update the progress bar.
pub enum ProgressMessage {
    /// Update the progress bar with the given value.
    Update(u64),
    /// Finish the progress bar.
    Finish,
}

/// Performs a shallow clone of a Git repository.
///
/// # Arguments
///
/// * `url` - A string representing the URL of the Git repository to clone.
/// * `path` - A string representing the local path where the repository should be cloned.
/// * `branch` - An optional string representing the branch to checkout after cloning. If `None`, the default branch will be checked out.
/// * `tag` - An optional string representing the tag to checkout after cloning. If `None`, the repository will be cloned at the specified branch.
/// * `tx` - A channel sender for progress reporting.
///
/// # Returns
///
/// * `Ok(Repository)` if the cloning process is successful and the repository is opened.
/// * `Err(git2::Error)` if an error occurs during the cloning process.
///
fn shallow_clone(
    url: &str,
    path: &str,
    branch: Option<&str>,
    tag: Option<&str>,
    tx: std::sync::mpsc::Sender<ProgressMessage>,
    recurse_submodules: bool,
) -> Result<Repository, git2::Error> {
    // Initialize fetch options with depth 1 for shallow cloning
    let mut fo = FetchOptions::new();
    if tag.is_none() {
        fo.depth(1);
    }

    // Set up remote callbacks for progress reporting
    let mut callbacks = RemoteCallbacks::new();
    callbacks.transfer_progress(|stats| {
        let val =
            ((stats.received_objects() as f64) / (stats.total_objects() as f64) * 100.0) as u64;
        tx.send(ProgressMessage::Update(val)).unwrap();
        true
    });
    fo.remote_callbacks(callbacks);

    // Create a new repository builder with the fetch options
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    // Set the branch to checkout if specified
    if let Some(branch) = branch {
        builder.branch(branch);
    };

    // Clone the repository
    let repo = builder.clone(url, Path::new(path))?;

    // If a tag is specified, checkout the corresponding commit
    if let Some(tag) = tag {
        // Look up the tag reference
        let tag_ref = repo.find_reference(&format!("refs/tags/{}", tag))?;
        // Peel the tag reference to get the commit object
        let tag_obj = tag_ref.peel(ObjectType::Commit)?;

        // Checkout the commit that the tag points to
        repo.checkout_tree(&tag_obj, None)?;
        repo.set_head_detached(tag_obj.id())?;
    };

    // If a branch is specified, checkout the corresponding branch
    if let Some(branch) = branch {
        // Rev-parse the branch reference to get the commit object
        let obj = repo.revparse_single(&format!("origin/{}", branch))?;
        // Checkout the commit that the branch points to
        repo.checkout_tree(&obj, None)?;
        repo.set_head(&format!("refs/heads/{}", branch))?;
    };

    if recurse_submodules {
        let mut sfo = FetchOptions::new();
        let mut callbacks = RemoteCallbacks::new();
        info!("Fetching submodules");
        callbacks.transfer_progress(|stats| {
            let val =
                ((stats.received_objects() as f64) / (stats.total_objects() as f64) * 100.0) as u64;
            tx.send(ProgressMessage::Update(val)).unwrap();
            true
        });
        sfo.remote_callbacks(callbacks);
        tx.send(ProgressMessage::Finish).unwrap();
        update_submodules(&repo, sfo, tx.clone())?;
        info!("Finished fetching submodules");
    }
    // Return the opened repository
    Ok(repo)
}

/// Updates submodules in the given repository using the provided fetch options.//+
/////+
/// # Parameters//+
/////+
/// * `repo`: A reference to the `git2::Repository` object representing the repository.//+
/// * `fetch_options`: A `git2::FetchOptions` object containing the fetch options to be used.//+
/// * `tx`: A `std::sync::mpsc::Sender<ProgressMessage>` object for sending progress messages.//+
/////+
/// # Returns//+
/////+
/// * `Result<(), git2::Error>`: On success, returns `Ok(())`. On error, returns a `git2::Error` indicating the cause of the error.//+
fn update_submodules(
    repo: &Repository,
    fetch_options: FetchOptions,
    tx: std::sync::mpsc::Sender<ProgressMessage>,
) -> Result<(), git2::Error> {
    let mut submodule_update_options = git2::SubmoduleUpdateOptions::new();
    submodule_update_options.fetch(fetch_options);

    fn update_submodules_recursive(
        repo: &Repository,
        path: &Path,
        fetch_options: &mut SubmoduleUpdateOptions,
        tx: std::sync::mpsc::Sender<ProgressMessage>,
    ) -> Result<(), git2::Error> {
        let submodules = repo.submodules()?;
        for mut submodule in submodules {
            tx.send(ProgressMessage::Finish).unwrap();
            submodule.update(true, Some(fetch_options))?;
            let sub_repo = submodule.open()?;
            update_submodules_recursive(
                &sub_repo,
                &path.join(submodule.path()),
                fetch_options,
                tx.clone(),
            )?;
        }
        Ok(())
    }

    update_submodules_recursive(
        repo,
        repo.workdir().unwrap(),
        &mut submodule_update_options,
        tx.clone(),
    )
}

// This function is not used right now  because of limited scope of the POC
// It gets specific fork of rustpython with build in libraries needed for IDF
#[cfg(feature = "userustpython")]
pub fn get_rustpython_fork(
    custom_path: &str,
    tx: std::sync::mpsc::Sender<ProgressMessage>,
) -> Result<String, git2::Error> {
    let output = shallow_clone(
        "https://github.com/Hahihula/RustPython.git",
        custom_path,
        Some("test-rust-build"),
        None,
        tx,
        false,
    );
    match output {
        Ok(repo) => Ok(repo.path().to_str().unwrap().to_string()),
        Err(e) => Err(e),
    }
}

// kept for pure reference how the IDF tools shouldc be runned using rustpython
pub fn run_idf_tools_using_rustpython(custom_path: &str) -> Result<String, std::io::Error> {
    let script_path = "esp-idf/tools/idf_tools.py";
    // env::set_var("RUSTPYTHONPATH", "/tmp/test-directory/RustPython/Lib"); // this is not needed as the standart library is bakend into the binary
    let output = std::process::Command::new("rustpython") // this works only on my machine (needs to point to the rustpython executable)
        .current_dir(custom_path)
        .arg(script_path)
        .arg("--idf-path")
        .arg(format!("{}/esp-idf", custom_path))
        .arg("--tools-json")
        .arg(format!("{}/esp-idf/tools/tools.json", custom_path))
        .arg("install")
        .arg("--targets")
        .arg("all")
        .arg("all")
        .output();
    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(std::str::from_utf8(&out.stdout).unwrap().to_string())
            } else {
                Ok(std::str::from_utf8(&out.stderr).unwrap().to_string())
            }
        }
        Err(e) => Err(e),
    }
}

/// Clones the ESP-IDF repository from the specified URL, tag, or branch,
/// using the provided progress function for reporting cloning progress.
///
/// # Parameters
///
/// * `path`: A reference to a string representing the local path where the repository should be cloned.
/// * `version`: A reference to a string representing the version of ESP-IDF to clone.
/// * `mirror`: An optional reference to a string representing the URL of a mirror to use for cloning the repository.
/// * `tx`: A `std::sync::mpsc::Sender<ProgressMessage>` object for sending progress messages.
/// * `with_submodules`: A boolean indicating whether to clone the ESP-IDF repository with submodules.
///
/// # Return Value
///
/// * `Result<std::string::String, git2::Error>`: On success, returns a `Result` containing the path of the cloned repository as a string.
///   On error, returns a `Result` containing a `git2::Error` indicating the cause of the error.
pub fn get_esp_idf_by_version_and_mirror(
    path: &str,
    version: &str,
    mirror: Option<&str>,
    tx: std::sync::mpsc::Sender<ProgressMessage>,
    with_submodules: bool,
) -> Result<std::string::String, git2::Error> {
    let tag = if version == "master" {
        None
    } else {
        Some(version)
    };
    let group_name = mirror
        .and_then(|m| {
            if m.contains("https://gitee.com/") {
                Some("EspressifSystems")
            } else {
                None
            }
        });
    get_esp_idf_by_tag_name(
        path,
        tag,
        tx,
        mirror,
        group_name,
        with_submodules,
    )
}

/// Clones the ESP-IDF repository from the specified URL, tag, or branch,
/// using the provided progress function for reporting cloning progress.
///
/// # Parameters
///
/// * `custom_path`: A string representing the local path where the repository should be cloned.
/// * `tag`: An optional string representing the tag to checkout after cloning. If `None`, the repository will be cloned at the specified branch.
/// * `progress_function`: A closure or function that will be called to report progress during the cloning process.
/// * `mirror`: An optional string representing the URL of a mirror to use for cloning the repository. If `None`, the default GitHub URL will be used.
/// * `group_name`: An optional string representing the group name for the repository. If `None`, the default group name "espressif" will be used.
///
/// # Returns
///
/// * `Result<String, git2::Error>`: On success, returns a `Result` containing the path of the cloned repository as a string.
///   On error, returns a `Result` containing a `git2::Error` indicating the cause of the error.
///
///
pub fn get_esp_idf_by_tag_name(
    custom_path: &str,
    tag: Option<&str>,
    tx: std::sync::mpsc::Sender<ProgressMessage>,
    mirror: Option<&str>,
    group_name: Option<&str>,
    with_submodules: bool,
) -> Result<String, git2::Error> {
    let group = group_name.unwrap_or("espressif");
    let url = match mirror {
        Some(url) => {
            format!("https://github.com/{}/esp-idf.git", group).replace("https://github.com", url)
        }
        None => "https://github.com/espressif/esp-idf.git".to_string(),
    };

    let _ = ensure_path(custom_path);
    let output = match tag {
        Some(tag) => shallow_clone(&url, custom_path, None, Some(tag), tx, with_submodules),
        None => shallow_clone(&url, custom_path, Some("master"), None, tx, with_submodules),
    };
    match output {
        Ok(repo) => Ok(repo.path().to_str().unwrap().to_string()),
        Err(e) => Err(e),
    }
}

/// Expands a tilde (~) in a given path to the user's home directory.
///
/// This function takes a reference to a `Path` and returns a `PathBuf` representing the expanded path.
/// If the input path starts with a tilde (~), the function replaces the tilde with the user's home directory.
/// If the input path does not start with a tilde, the function returns the input path as is.
///
/// # Parameters
///
/// * `path`: A reference to a `Path` representing the path to be expanded.
///
/// # Return Value
///
/// * A `PathBuf` representing the expanded path.
///
pub fn expand_tilde(path: &Path) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home_dir) = dirs::home_dir() {
            if path.to_str().unwrap() == "~" {
                home_dir
            } else {
                home_dir.join(path.strip_prefix("~").unwrap())
            }
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    }
}

/// Performs post-installation tasks for a single version of ESP-IDF.
///
/// This function creates a desktop shortcut on Windows systems and generates an activation shell script
/// for other operating systems. The desktop shortcut is created using the `create_desktop_shortcut` function,
/// and the activation shell script is generated using the `create_activation_shell_script` function.
///
/// # Parameters
///
/// * `version_instalation_path`: A reference to a string representing the path where the ESP-IDF version is installed.
/// * `idf_path`: A reference to a string representing the path to the ESP-IDF repository.
/// * `idf_version`: A reference to a string representing the version of ESP-IDF being installed.
/// * `tool_install_directory`: A reference to a string representing the directory where the ESP-IDF tools will be installed.
/// * `export_paths`: A vector of strings representing the paths that need to be exported for the ESP-IDF tools.
pub fn single_version_post_install(
    version_instalation_path: &str,
    idf_path: &str,
    idf_version: &str,
    tool_install_directory: &str,
    export_paths: Vec<String>,
) {
    let env_vars = setup_environment_variables(
        &PathBuf::from(tool_install_directory),
        &PathBuf::from(idf_path),
    )
    .unwrap_or_default();
    match std::env::consts::OS {
        "windows" => {
            // Creating desktop shortcut
            if let Err(err) = create_desktop_shortcut(
                version_instalation_path,
                idf_path,
                idf_version,
                tool_install_directory,
                None,
                export_paths,
                env_vars,
            ) {
                error!(
                    "{} {:?}",
                    "Failed to create desktop shortcut",
                    err.to_string()
                )
            } else {
                info!("Desktop shortcut created successfully")
            }
        }
        _ => {
            let install_folder = PathBuf::from(version_instalation_path);
            let install_path = install_folder.parent().unwrap().to_str().unwrap();
            let _ = create_activation_shell_script(
                // todo: handle error
                install_path,
                idf_path,
                tool_install_directory,
                None,
                idf_version,
                export_paths,
                env_vars,
            );
        }
    }
}

/// Returns a list of available IDF mirrors.
///
/// # Purpose
///
/// This function provides a list of URLs that can be used as mirrors for cloning the ESP-IDF repository.
///
/// # Parameters
///
/// None.
///
/// # Return Value
///
/// A reference to a static array of static strings, where each string represents a mirror URL.
///
pub fn get_idf_mirrors_list() -> &'static [&'static str] {
    &["https://github.com", "https://jihulab.com/esp-mirror"]
}

/// Returns a list of available IDF tools mirrors.
///
/// This function provides a list of URLs that can be used as mirrors for cloning the ESP-IDF tools repository.
///
/// # Parameters
///
/// None.
///
/// # Return Value
///
/// A reference to a static array of static strings, where each string represents a mirror URL.
///
pub fn get_idf_tools_mirrors_list() -> &'static [&'static str] {
    &[
        "https://github.com",
        "https://dl.espressif.com/github_assets",
        "https://dl.espressif.cn/github_assets",
    ]
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    use flate2::read::GzEncoder;

    fn create_test_file(content: &str) -> (TempDir, String) {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        (dir, file_path.to_string_lossy().into_owned())
    }

    fn create_zip_archive(content: &str) -> (TempDir, String) {
        let (_source_dir, _source_path) = create_test_file(content);
        let dest_dir = TempDir::new().unwrap();
        let zip_path = dest_dir.path().join("archive.zip");

        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("test.txt", options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
        zip.finish().unwrap();

        (dest_dir, zip_path.to_string_lossy().into_owned())
    }

    fn create_tar_archive(content: &str) -> (TempDir, String) {
        let (_source_dir, source_path) = create_test_file(content);
        let dest_dir = TempDir::new().unwrap();
        let tar_path = dest_dir.path().join("archive.tar");

        let file = File::create(&tar_path).unwrap();
        let mut builder = tar::Builder::new(file);
        builder
            .append_path_with_name(&source_path, "test.txt")
            .unwrap();
        builder.finish().unwrap();

        (dest_dir, tar_path.to_string_lossy().into_owned())
    }

    #[test]
    fn test_decompress_zip() {
        let content = "test content";
        let (_archive_dir, archive_path) = create_zip_archive(content);
        let extract_dir = TempDir::new().unwrap();

        decompress_archive(&archive_path, extract_dir.path().to_str().unwrap()).unwrap();

        let extracted_content = fs::read_to_string(extract_dir.path().join("test.txt")).unwrap();
        assert_eq!(extracted_content, content);
    }

    #[test]
    fn test_decompress_tar() {
        let content = "test content";
        let (_archive_dir, archive_path) = create_tar_archive(content);
        let extract_dir = TempDir::new().unwrap();

        decompress_archive(&archive_path, extract_dir.path().to_str().unwrap()).unwrap();

        let extracted_content = fs::read_to_string(extract_dir.path().join("test.txt")).unwrap();
        assert_eq!(extracted_content, content);
    }

    #[test]
    fn test_invalid_format() {
        let (_dir, file_path) = create_test_file("test content");
        let extract_dir = TempDir::new().unwrap();

        let result = decompress_archive(&file_path, extract_dir.path().to_str().unwrap());
        assert!(matches!(result, Err(DecompressionError::UnsupportedFormat)));
    }

    #[test]
    fn test_nonexistent_file() {
        let extract_dir = TempDir::new().unwrap();
        let result = decompress_archive("nonexistent.zip", extract_dir.path().to_str().unwrap());
        assert!(matches!(result, Err(DecompressionError::Io(_))));
    }

    #[test]
    fn test_verify_file_checksum_with_valid_file() {
        let file_path = "test_file.txt";
        let expected_checksum = "e2d0fe1585a63ec6009c8016ff8dda8b17719a637405a4e23c0ff81339148249";

        // Create a test file with the expected content
        fs::write(file_path, "This is a test file").unwrap();

        let result = verify_file_checksum(expected_checksum, file_path);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Clean up the test file
        fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_verify_file_checksum_with_invalid_checksum() {
        let file_path = "test_file_inv.txt";
        let expected_checksum = "invalid_checksum";

        // Create a test file with the expected content
        fs::write(file_path, "This is a test file").unwrap();

        let result = verify_file_checksum(expected_checksum, file_path);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);

        // Clean up the test file
        fs::remove_file(file_path).unwrap();
    }
    #[test]
    fn test_verify_file_checksum_with_nonexistent_file() {
        let file_path = "nonexistent_file.txt";
        let expected_checksum = "6a266d99f1729281c1b7a079793898292837a659";

        let result = verify_file_checksum(expected_checksum, file_path);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_verify_file_checksum_with_empty_file() {
        let file_path = "empty_file.txt";
        let expected_checksum = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        // Create an empty test file
        fs::File::create(file_path).unwrap();

        let result = verify_file_checksum(expected_checksum, file_path);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Clean up the test file
        fs::remove_file(file_path).unwrap();
    }
    #[test]
    fn test_verify_file_checksum_with_large_file() {
        let file_path = "large_file.txt";
        let expected_checksum = "ef2e29e83198cfd2d1edd7b8c1508235d16a78d2d3a00e493c9c0bdebce8eecc";

        // Create a large test file with the expected content
        let mut file = fs::File::create(file_path).unwrap();
        for _ in 0..1000000 {
            file.write_all(b"This is a test file").unwrap();
        }

        let result = verify_file_checksum(expected_checksum, file_path);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Clean up the test file
        fs::remove_file(file_path).unwrap();
    }
    #[test]
    fn test_ensure_path_with_special_characters() {
        let directory_path = "/tmp/path/to/directory with spaces and@special#characters";

        // Remove the directory if it already exists
        fs::remove_dir_all(directory_path).ok();

        let result = ensure_path(directory_path);

        assert!(result.is_ok());

        // Clean up the directory
        fs::remove_dir_all(directory_path).unwrap();
    }
    #[test]
    fn test_ensure_path_with_existing_directory() {
        let directory_path = "./python_scripts";

        // Create the existing directory
        fs::create_dir_all(directory_path).unwrap();

        let result = ensure_path(directory_path);

        assert!(result.is_ok());
    }
    #[test]
    fn test_expand_tilde() {
        let home_dir = dirs::home_dir().unwrap();
        let tilde_path = Path::new("~/test_directory");
        let expanded_path = expand_tilde(tilde_path);

        assert_eq!(expanded_path, home_dir.join("test_directory"));
    }

    #[test]
    fn test_get_elf_rom_dir_with_valid_structure() -> Result<(), Box<dyn std::error::Error>> {
        // Create a temporary directory that will be automatically cleaned up
        let temp_dir = TempDir::new()?;
        let idf_tools_path = temp_dir.path().to_path_buf();

        // Create the directory structure
        let tools_dir = idf_tools_path.join("tools");
        let esp_rom_dir = tools_dir.join("esp-rom-elfs");
        let version_dir = esp_rom_dir.join("20243982");

        fs::create_dir_all(&version_dir)?;

        // Call the function
        let result = get_elf_rom_dir(&idf_tools_path)?;

        // Verify the result
        assert_eq!(result, version_dir);

        Ok(())
    }
    #[test]
    fn test_get_elf_rom_dir_with_empty_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let idf_tools_path = temp_dir.path().to_path_buf();

        // Create empty directory structure
        let tools_dir = idf_tools_path.join("tools");
        let esp_rom_dir = tools_dir.join("esp-rom-elfs");
        fs::create_dir_all(&esp_rom_dir)?;

        // Call the function
        let result = get_elf_rom_dir(&idf_tools_path)?;

        // Should return the esp-rom-elfs directory even if empty
        assert_eq!(result, esp_rom_dir);

        Ok(())
    }

    #[test]
    fn test_get_elf_rom_dir_with_nonexistent_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let idf_tools_path = temp_dir.path().to_path_buf();

        // Don't create any directories

        // Call the function
        let result = get_elf_rom_dir(&idf_tools_path)?;

        // Should return a path to the (nonexistent) esp-rom-elfs directory
        assert_eq!(result, idf_tools_path.join("tools").join("esp-rom-elfs"));

        Ok(())
    }
}
