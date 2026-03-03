use clap::builder;
use clap::Parser;
use idf_im_lib::command_executor::{execute_command_with_dir, execute_command,execute_command_with_env};
use idf_im_lib::download_file;
use idf_im_lib::download_file_and_rename;
use idf_im_lib::ensure_path;
use idf_im_lib::idf_tools::get_list_of_tools_to_download;
use idf_im_lib::python_utils::download_constraints_file;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::extract_zst_archive;
use idf_im_lib::utils::{parse_cmake_version, find_by_name_and_extension};
use idf_im_lib::verify_file_checksum;
use idf_im_lib::offline_installer::merge_requirements_files;
use idf_im_lib::git_tools::ProgressMessage;
use idf_im_lib::logging;
use idf_im_lib::get_log_directory;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::debug;
use log::error;
use log::info;
use log::warn;
use std::fmt::Write;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{Read, Write as otherwrite};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use tar::Builder as TarBuilder;
use tar::Archive;
use tempfile::TempDir;
use zstd::{encode_all, decode_all};
use fern::Dispatch;
use log::LevelFilter;
use serde::{Serialize, Deserialize};

pub const PYTHON_VERSION: &str = "3.11";
pub const SUPPORTED_PYTHON_VERSIONS: &[&str] = &["3.10", "3.11", "3.12", "3.13", "3.14"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythonVersionResult {
    version: String,
    success: bool,
    error_message: Option<String>,
    source_built_packages: Vec<String>,
}

impl PythonVersionResult {
    fn categorize_error(error_msg: &str) -> (String, String) {
        if error_msg.contains("ModuleNotFoundError: No module named 'imp'") {
            ("Python 3.12+ Incompatibility".to_string(),
             "Package requires deprecated 'imp' module. Consider excluding from Python 3.12+".to_string())
        } else if error_msg.contains("resolution-too-deep") {
            ("Dependency Resolution Timeout".to_string(),
             "Pip dependency graph too complex. Try using constraints file with pinned versions".to_string())
        } else if error_msg.contains("gobject-introspection-1.0") {
            ("Missing System Dependencies".to_string(),
             "PyGObject requires system libraries (gobject-introspection-1.0). Install via apt or skip binary-only downloads".to_string())
        } else if error_msg.contains("KeyError: '__version__'") {
            ("Package Build Error".to_string(),
             "Package metadata issue during build. May need source build or different version".to_string())
        } else if error_msg.contains("--only-binary") || error_msg.contains("binary") {
            ("Binary Package Unavailable".to_string(),
             "No prebuilt wheel available for this platform/Python version".to_string())
        } else if error_msg.contains("subprocess-exited-with-error") {
            ("Build Subprocess Failed".to_string(),
             "Package compilation failed. Check if system build dependencies are installed".to_string())
        } else {
            ("Unknown Error".to_string(), error_msg.lines().take(3).collect::<Vec<_>>().join(" | "))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildSummary {
    idf_version: String,
    architecture: String,
    python_versions: Vec<PythonVersionResult>,
    archive_created: bool,
    archive_path: Option<String>,
    archive_size: Option<u64>,
}

impl BuildSummary {
    fn new(idf_version: String, architecture: String) -> Self {
        Self {
            idf_version,
            architecture,
            python_versions: Vec::new(),
            archive_created: false,
            archive_path: None,
            archive_size: None,
        }
    }

    fn all_python_successful(&self) -> bool {
        !self.python_versions.is_empty() && self.python_versions.iter().all(|p| p.success)
    }

    fn any_python_failed(&self) -> bool {
        self.python_versions.iter().any(|p| !p.success)
    }

    fn all_python_failed(&self) -> bool {
        !self.python_versions.is_empty() && self.python_versions.iter().all(|p| !p.success)
    }

    fn to_github_summary(&self) -> String {
        let status_icon = if self.archive_created && self.all_python_successful() {
            "✅"
        } else if self.archive_created && self.any_python_failed() {
            "⚠️"
        } else {
            "❌"
        };

        let mut summary = String::new();

        if self.archive_created && self.all_python_successful() {
            // Brief success format
            summary.push_str(&format!("{} **{}** (`{}`)\n", status_icon, self.idf_version, self.architecture));
            summary.push_str(&format!("- Python versions: {}\n",
                self.python_versions.iter()
                    .map(|p| p.version.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")));
            if let Some(size) = self.archive_size {
                summary.push_str(&format!("- Archive size: {:.2} MB\n", size as f64 / 1_048_576.0));
            }

            // Show packages that needed source builds
            let source_built: Vec<_> = self.python_versions.iter()
                .filter(|p| !p.source_built_packages.is_empty())
                .collect();
            if !source_built.is_empty() {
                summary.push_str("\n**Packages built from source:**\n");
                for pv in source_built {
                    summary.push_str(&format!("- Python {}: {}\n", pv.version, pv.source_built_packages.join(", ")));
                }
            }
        } else {
            // Detailed warning/error format
            summary.push_str(&format!("{} **{}** (`{}`)\n", status_icon, self.idf_version, self.architecture));
            summary.push_str("\n");

            let successful: Vec<_> = self.python_versions.iter().filter(|p| p.success).collect();
            let failed: Vec<_> = self.python_versions.iter().filter(|p| !p.success).collect();

            if !successful.is_empty() {
                summary.push_str(&format!("**✅ Successful Python versions:** {}\n\n",
                    successful.iter().map(|p| p.version.as_str()).collect::<Vec<_>>().join(", ")));
            }

            if !failed.is_empty() {
                summary.push_str(&format!("**❌ Failed Python versions:** {}\n\n",
                    failed.iter().map(|p| p.version.as_str()).collect::<Vec<_>>().join(", ")));

                summary.push_str("<details>\n<summary>Error Details</summary>\n\n");
                for pv in failed {
                    summary.push_str(&format!("**Python {}:**\n```\n{}\n```\n\n",
                        pv.version,
                        pv.error_message.as_deref().unwrap_or("Unknown error")));
                }
                summary.push_str("</details>\n\n");
            }

            if !self.archive_created {
                summary.push_str("**Status:** Archive creation failed\n");
            } else if let Some(size) = self.archive_size {
                summary.push_str(&format!("**Archive size:** {:.2} MB\n", size as f64 / 1_048_576.0));
            }
        }

        summary.push_str("\n");
        summary
    }
}

fn write_github_summary(summaries: &[BuildSummary]) {
    let github_step_summary = std::env::var("GITHUB_STEP_SUMMARY");

    let output = if let Ok(summary_file) = github_step_summary {
        Some(summary_file)
    } else {
        None
    };

    let mut content = String::new();
    content.push_str("# Offline Installer Build Summary\n\n");

    let all_success = summaries.iter().all(|s| s.archive_created && s.all_python_successful());
    let any_warnings = summaries.iter().any(|s| s.archive_created && s.any_python_failed());
    let any_failures = summaries.iter().any(|s| !s.archive_created || s.all_python_failed());

    if all_success {
        content.push_str("## ✅ All builds successful\n\n");
    } else if any_failures {
        content.push_str("## ❌ Build failures detected\n\n");
    } else if any_warnings {
        content.push_str("## ⚠️ Builds completed with warnings\n\n");
    }

    for summary in summaries {
        content.push_str(&summary.to_github_summary());
    }

    // Write to file if in GitHub Actions
    if let Some(file_path) = output {
        if let Err(e) = fs::write(&file_path, &content) {
            error!("Failed to write GitHub step summary to {}: {}", file_path, e);
        } else {
            info!("GitHub step summary written to {}", file_path);
        }
    }

    // Also write to a local file for debugging
    if let Err(e) = fs::write("build_summary.md", &content) {
        warn!("Failed to write local build summary: {}", e);
    }

    // Print to stdout
    println!("\n{}", content);
}

/// Setup logging for the offline installer builder.
///
/// # Arguments
/// * `verbose` - Verbosity level (0=Info, 1=Debug, 2+=Trace)
/// * `custom_log_dir` - Optional custom directory for the log file
///
/// The log file will be named "offline_installer.log" in the specified directory.
///
/// # Log Level Behavior
/// | verbose | Log Level |
/// |---------|-----------|
/// | 0       | Info      |
/// | 1       | Debug     |
/// | 2+      | Trace     |
pub fn setup_offline_installer(
    verbose: u8,
    custom_log_dir: Option<PathBuf>,
) -> Result<(), fern::InitError> {
    let log_level = match verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    // Determine log file path
    let log_dir = custom_log_dir.or_else(get_log_directory).unwrap_or_else(|| PathBuf::from("."));
    let log_file_path = log_dir.join("offline_installer.log");

    // Ensure log directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory {}: {}", log_dir.display(), e);
    }

    Dispatch::new()
        .format(logging::formatter)
        .level(log_level)
        .chain(fern::log_file(&log_file_path)?)
        .chain(std::io::stdout())
        .apply()?;

    log::debug!("Offline installer logging initialized at level: {:?}", log_level);
    Ok(())
}


pub fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] ({eta})",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        })
        .progress_chars("#>-"),
    );
    pb
}

pub fn update_progress_bar_number(pb: &ProgressBar, value: u64) {
    pb.set_position(value);
}

/// Downloads wheels for multiple Python versions
///
/// # Arguments
/// * `archive_dir` - Base directory for the archive
/// * `requirements_path` - Path to the requirements file
/// * `constraint_file` - Path to the constraints file
/// * `python_versions` - List of Python versions to download wheels for
///
/// # Returns
/// `Vec<PythonVersionResult>` - Results for each Python version
async fn download_wheels_for_python_versions(
    archive_dir: &Path,
    requirements_path: &Path,
    constraint_file: &Path,
    python_versions: &[&str],
) -> Vec<PythonVersionResult> {
    info!("Downloading wheels for Python versions: {:?}", python_versions);

    let mut results = Vec::new();

    // First, ensure all Python versions are installed
    for python_version in python_versions {
        info!("Installing Python {}...", python_version);
        match execute_command("uv", &["python", "install", python_version]) {
            Ok(output) => {
                if output.status.success() {
                    info!("Python {} installed successfully.", python_version);
                } else {
                    warn!(
                        "Python {} might already be installed or failed to install: {}",
                        python_version,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Err(err) => {
                error!("Failed to install Python {}: {}", python_version, err);
                results.push(PythonVersionResult {
                    version: python_version.to_string(),
                    success: false,
                    error_message: Some(format!("Failed to install Python: {}", err)),
                    source_built_packages: vec![],
                });
                continue;
            }
        }
    }

    for python_version in python_versions {
        // Skip if already marked as failed
        if results.iter().any(|r| r.version == *python_version && !r.success) {
            continue;
        }

        info!("Processing Python version: {}", python_version);

        // Create version-specific directories
        let python_env = archive_dir.join(format!("python_env_{}", python_version.replace('.', "_")));
        let wheel_dir = archive_dir.join(format!("wheels_py{}", python_version.replace('.', "")));

        // Ensure directories exist
        if let Err(e) = ensure_path(python_env.to_str().unwrap()) {
            error!("Failed to create Python env directory for {}: {}", python_version, e);
            results.push(PythonVersionResult {
                version: python_version.to_string(),
                success: false,
                error_message: Some(format!("Failed to create directory: {}", e)),
                source_built_packages: vec![],
            });
            continue;
        }
        if let Err(e) = ensure_path(wheel_dir.to_str().unwrap()) {
            error!("Failed to create wheel directory for {}: {}", python_version, e);
            results.push(PythonVersionResult {
                version: python_version.to_string(),
                success: false,
                error_message: Some(format!("Failed to create directory: {}", e)),
                source_built_packages: vec![],
            });
            continue;
        }

        info!("Creating virtual environment for Python {}...", python_version);

        // Create virtual environment for this Python version
        match execute_command(
            "uv",
            &[
                "venv",
                "--relocatable",
                "--python",
                python_version,
                python_env.to_str().unwrap(),
            ],
        ) {
            Ok(output) => {
                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    error!("Failed to create Python {} virtual environment: {}", python_version, error_msg);
                    results.push(PythonVersionResult {
                        version: python_version.to_string(),
                        success: false,
                        error_message: Some(format!("venv creation failed: {}", error_msg)),
                        source_built_packages: vec![],
                    });
                    continue;
                }
                info!("Python {} virtual environment created successfully.", python_version);
            }
            Err(err) => {
                error!("Failed to create venv for Python {}: {}", python_version, err);
                results.push(PythonVersionResult {
                    version: python_version.to_string(),
                    success: false,
                    error_message: Some(format!("venv creation error: {}", err)),
                    source_built_packages: vec![],
                });
                continue;
            }
        }

        // Determine Python executable path
        let python_executable = match std::env::consts::OS {
            "windows" => python_env.join("Scripts/python.exe"),
            _ => python_env.join("bin/python"),
        };

        // Install pip into the virtual environment
        info!("Installing pip into venv for Python {}...", python_version);
        match execute_command(
            python_executable.to_str().unwrap(),
            &["-m", "ensurepip"],
        ) {
            Ok(output) => {
                if output.status.success() {
                    info!("Pip installed into venv for Python {}.", python_version);
                } else {
                    error!("Failed to install pip into venv: {}", String::from_utf8_lossy(&output.stderr));
                    results.push(PythonVersionResult {
                        version: python_version.to_string(),
                        success: false,
                        error_message: Some(format!("Failed to install pip: {}", String::from_utf8_lossy(&output.stderr))),
                        source_built_packages: vec![],
                    });
                    continue;
                }
            }
            Err(err) => {
                error!("Failed to install pip: {}", err);
                results.push(PythonVersionResult {
                    version: python_version.to_string(),
                    success: false,
                    error_message: Some(format!("Failed to install pip: {}", err)),
                    source_built_packages: vec![],
                });
                continue;
            }
        }

        // Upgrade pip to latest version
        info!("Upgrading pip for Python {}...", python_version);
        let _ = execute_command(
            python_executable.to_str().unwrap(),
            &["-m", "pip", "install", "--upgrade", "pip"],
        );

        // Download wheels for this Python version - TWO STEP APPROACH
        info!("Downloading packages for Python {}...", python_version);

        // STEP 1: Try binary-only first
        info!("STEP 1: Attempting binary-only download for Python {}...", python_version);

        std::env::set_var("PIP_MAX_ROUNDS", "200");

        let binary_result = execute_command_with_env(
            python_executable.to_str().unwrap(),
            &vec![
                "-m", "pip", "download",
                "-r", requirements_path.to_str().unwrap(),
                "-c", constraint_file.to_str().unwrap(),
                "--dest", wheel_dir.to_str().unwrap(),
                "--only-binary=:all:",
                "--index-url", "https://dl.espressif.com/pypi/",
                "--extra-index-url", "https://pypi.org/simple",
            ],
            vec!(("PIP_MAX_ROUNDS", "300"))
        );

        let mut source_built = Vec::new();
        let mut result = binary_result;

        // STEP 2: If binary-only failed, retry allowing source builds
        if result.is_err() || !result.as_ref().unwrap().status.success() {
            warn!("Binary-only download failed for Python {}", python_version);

            if let Ok(ref output) = result {
                let stderr = String::from_utf8_lossy(&output.stderr);
                info!("Binary-only error: {}", stderr.lines().take(5).collect::<Vec<_>>().join(" | "));
            }

            info!("STEP 2: Retrying with source builds allowed for Python {}...", python_version);

            result = execute_command_with_env(
                python_executable.to_str().unwrap(),
                &vec!(
                    "-m", "pip", "download", "--verbose",
                    "-r", requirements_path.to_str().unwrap(),
                    "-c", constraint_file.to_str().unwrap(),
                    "--dest", wheel_dir.to_str().unwrap(),
                    "--index-url", "https://dl.espressif.com/pypi/",
                    "--extra-index-url", "https://pypi.org/simple",
                ),
                vec!(("PIP_MAX_ROUNDS", "300"))
            );

            // Parse output to find packages that were built from source
            if let Ok(ref output) = result {
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );

                for line in combined.lines() {
                    // Pip shows "Building wheel for" when building from source
                    if line.contains("Building wheel for") {
                        if let Some(pkg_name) = line.split("Building wheel for ").nth(1) {
                            if let Some(pkg) = pkg_name.split_whitespace().next() {
                                if !source_built.contains(&pkg.to_string()) {
                                    source_built.push(pkg.to_string());
                                    warn!("Building package from source: {}", pkg);
                                }
                            }
                        }
                    }
                }

                if !source_built.is_empty() {
                    warn!("Python {}: Built {} packages from source: {:?}",
                          python_version, source_built.len(), source_built);
                }
            }
        } else {
            info!("Binary-only download succeeded for Python {}", python_version);
        }

        std::env::remove_var("PIP_MAX_ROUNDS");

        match result {
            Ok(output) => {
                if output.status.success() {
                    info!("Python {} packages downloaded successfully.", python_version);
                    if !source_built.is_empty() {
                        info!("Packages built from source for Python {}: {:?}", python_version, source_built);
                    }
                    results.push(PythonVersionResult {
                        version: python_version.to_string(),
                        success: true,
                        error_message: None,
                        source_built_packages: source_built,
                    });
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    error!("Failed to download Python {} packages: {}", python_version, error_msg);
                    results.push(PythonVersionResult {
                        version: python_version.to_string(),
                        success: false,
                        error_message: Some(error_msg),
                        source_built_packages: source_built,
                    });
                }
            }
            Err(err) => {
                error!("Failed to download packages for Python {}: {}", python_version, err);
                results.push(PythonVersionResult {
                    version: python_version.to_string(),
                    success: false,
                    error_message: Some(err.to_string()),
                    source_built_packages: source_built,
                });
            }
        }
    }

    results
}

fn get_architecture() -> String {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x64".to_string(),
        ("linux", "aarch64") => "linux-aarch64".to_string(),
        ("windows", "x86_64") => "windows-x64".to_string(),
        ("macos", "x86_64") => "macos-x64".to_string(),
        ("macos", "aarch64") => "macos-aarch64".to_string(),
        (os, arch) => format!("{}-{}", os, arch),
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "offline_installer_builder",
    about = "Offline installer builder for ESP-IDF Installation Manager"
)]
struct Args {
    /// Path to the installation data file
    #[arg(short, long, value_name = "FILE")]
    archive: Option<PathBuf>,

    /// Installation directory where the temporary data will be extracted
    #[arg(
        short,
        long,
        value_name = "DIR",
        default_value = "/tmp/eim_install_data"
    )]
    install_dir: Option<PathBuf>,

    /// Create installation data from the specified configuration file use "default" to use the default settings
    #[arg(short, long, value_name = "CONFIG")]
    create_from_config: Option<String>,

    /// Increase output verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Number of python version to use, default is 3.10
    #[arg(
        short = 'p',
        long,
        default_value = PYTHON_VERSION,
    )]
    python_version: Option<String>,

    /// Python versions to download wheels for (comma-separated, e.g., "3.10,3.11,3.12")
    /// If not specified, uses all supported versions for POSIX systems, single version for Windows
    #[arg(long, value_delimiter = ',')]
    wheel_python_versions: Option<Vec<String>>,

    /// Override IDF version (e.g., "v5.1.2", "v5.0.4")
    /// If specified, only this version will be processed instead of versions from config
    #[arg(long)]
    idf_version_override: Option<String>,

    /// Build separate archives for all supported IDF versions
    /// Each version will create its own .zst archive file
    #[arg(long)]
    build_all_versions: bool,

    /// List all supported IDF versions in machine-readable format and exit
    /// Output format: one version per line
    #[arg(long)]
    list_versions: bool,

    /// Custom log directory (default: system log directory)
    #[arg(long, value_name = "DIR")]
    log_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.list_versions {
        let versions = idf_im_lib::idf_versions::get_stable_idf_names().await;
        for version in versions {
            println!("{}", version);
        }
        return;
    }

    // Setup logging using fern
    if let Err(e) = setup_offline_installer(args.verbose, args.log_dir.clone()) {
        error!("Failed to initialize logging: {e}");
    }

    let architecture = get_architecture();

    if args.create_from_config.is_some() {
        info!(
            "Creating installation data from configuration file: {:?}",
            args.create_from_config
        );
        let mut settings = match args.create_from_config {
            Some(ref config_path) if config_path == "default" => {
                // Load default settings
                let settings = Settings::default();
                info!("Default settings loaded: {:?}", settings);
                settings
            }
            Some(config_path) => {
                // Load settings from the configuration file
                let mut settings = Settings::default();
                match settings.load(&config_path) {
                    Ok(_) => {
                    info!("Settings loaded from {}: {:?}", config_path, settings);
                    }
                    Err(e) => {
                    error!("Failed to load settings from {}: {}", config_path, e);
                        return;
                    }
                }
                settings
            }
            None => {
                error!("No configuration file provided for creating installation data.");
                return;
            }
        };
        let archive_dir = TempDir::new().expect("Failed to create temporary directory");
        let global_python_version = args.python_version.unwrap_or_else(|| PYTHON_VERSION.to_string());
        info!("Using Python version: {}", global_python_version);

        // Determine which Python versions to download wheels for
        let wheel_python_versions: Vec<String> = if let Some(versions) = args.wheel_python_versions {
            versions
        } else {
            // Default behavior: for Windows, use single version; for POSIX, use all supported
            match std::env::consts::OS {
                "windows" => vec![global_python_version.clone()],
                _ => SUPPORTED_PYTHON_VERSIONS.iter().map(|s| s.to_string()).collect(),
            }
        };

        info!("Will download wheels for Python versions: {:?}", wheel_python_versions);

        let versions = idf_im_lib::idf_versions::get_idf_names(false).await;
        let version_list = if let Some(override_version) = args.idf_version_override {
            info!("Using IDF version override: {}", override_version);
            vec![override_version]
        } else if args.build_all_versions {
            info!("Building separate archives for all supported versions: {:?}", versions);
            versions.clone() // We'll iterate over all
        } else {
            settings
                .idf_versions
                .clone()
                .unwrap_or(vec![versions.first().unwrap().clone()])
        };

        settings.idf_versions = Some(version_list.clone());

        // Check if uv is installed
        match execute_command("uv", &["--version"]) {
            Ok(output) => {
                if output.status.success() {
                    info!("UV is installed: {:?}", output);
                } else {
                    error!("UV is not installed or not found: {:?}", output);
                    return;
                }
            }
            Err(err) => {
                error!("UV is not installed or not found: {}. Please install it and try again.", err);
                return;
            }
        }

        // DOWNLOAD SHARED PREREQUISITES ONCE (Windows only)
        let shared_prereq_dir: Option<TempDir> = if std::env::consts::OS == "windows" {
            let temp_shared = TempDir::new().expect("Failed to create shared prereq temp dir");
            let scoop_path = temp_shared.path().join("scoop");
            ensure_path(scoop_path.to_str().unwrap()).expect("Failed to create scoop dir");

            let scoop_list = vec![
                ("https://github.com/ScoopInstaller/Scoop/archive/master.zip", "scoop-master.zip"),
                ("https://github.com/ScoopInstaller/Main/archive/master.zip", "main-master.zip"),
                ("https://github.com/git-for-windows/git/releases/download/v2.50.1.windows.1/PortableGit-2.50.1-64-bit.7z.exe", "PortableGit-2.50.1-64-bit.7z.exe"),
                ("https://www.python.org/ftp/python/3.11.9/python-3.11.9-amd64.exe", "python-3.11.9-amd64.exe"),
                ("https://raw.githubusercontent.com/ScoopInstaller/Main/master/scripts/python/install-pep-514.reg", "install-pep-514.reg"),
                ("https://raw.githubusercontent.com/ScoopInstaller/Main/master/scripts/python/uninstall-pep-514.reg", "uninstall-pep-514.reg"),
                ("https://www.7-zip.org/a/7z2501-x64.msi", "7z2501-x64.msi"),
                ("https://raw.githubusercontent.com/ScoopInstaller/Binary/master/dark/dark-3.14.1.zip", "dark-3.14.1.zip"),
            ];

            for (link, name) in scoop_list {
                info!("Downloading Scoop prereq: {} as {}", link, name);
                match download_file_and_rename(link, scoop_path.to_str().unwrap(), None, Some(name)).await {
                    Ok(_) => info!("Downloaded: {}", name),
                    Err(err) => {
                        error!("Failed to download {}: {}", name, err);
                        return;
                    }
                }
            }

            let scoop_install_script = include_str!("../powershell_scripts/install_scoop_offline.ps1");
            fs::write(scoop_path.join("install_scoop_offline.ps1"), scoop_install_script)
                .expect("Failed to write install_scoop_offline.ps1 script");

            info!("Shared Windows prerequisites downloaded to: {:?}", temp_shared.path());
            Some(temp_shared)
        } else if std::env::consts::OS == "linux" || std::env::consts::OS == "macos" {
            info!("Detected Unix-like OS, prerequisites installation not implemented — skipping.");
            None
        } else {
            error!("Unsupported OS: {}", std::env::consts::OS);
            return;
        };

        // ITERATE OVER EACH VERSION AND BUILD SEPARATE ARCHIVE
        let mut build_summaries = Vec::new();
        for idf_version in version_list {
            let mut summary = BuildSummary::new(idf_version.clone(), architecture.clone());

            info!("=== Processing ESP-IDF version: {} ===", idf_version);

            // Create a fresh temp dir for this version
            let archive_dir = TempDir::new().expect("Failed to create version-specific temp dir");
            let version_path = archive_dir.path().join(&idf_version);
            ensure_path(version_path.to_str().unwrap()).expect("Failed to ensure version path");

            // 👇 COPY SHARED PREREQUISITES (if any)
            if let Some(ref shared_dir) = shared_prereq_dir {
                let dest_scoop = archive_dir.path().join("scoop");
                info!("Copying shared prerequisites to: {:?}", dest_scoop);
                fs_extra::dir::copy(
                    shared_dir.path().join("scoop"),
                    &dest_scoop,
                    &fs_extra::dir::CopyOptions::new().overwrite(true).copy_inside(true),
                )
                .expect("Failed to copy shared prerequisites");
            }

            // Download ESP-IDF for this version
            let (tx, rx) = mpsc::channel();
            let handle = thread::spawn(move || {
                let mut progress_bar = create_progress_bar();
                loop {
                    match rx.recv() {
                        Ok(ProgressMessage::Finish) => {
                            update_progress_bar_number(&progress_bar, 100);
                            progress_bar.finish();
                            progress_bar = create_progress_bar();
                        }
                        Ok(ProgressMessage::Update(value)) => {
                            update_progress_bar_number(&progress_bar, value);
                        }
                        Ok(ProgressMessage::SubmoduleUpdate((name, value))) => {
                            let message = format!("{}: {}", name, value);
                            progress_bar.set_message(message);
                            progress_bar.set_position(value);
                        }
                        Ok(ProgressMessage::SubmoduleFinish(name)) => {
                            let message = format!("{}: {}", name, 100);
                            progress_bar.set_message(message);
                            progress_bar.finish();
                            info!("submodule: {}", name);
                            progress_bar = create_progress_bar();
                        }
                        Err(_) => break,
                    }
                }
            });

            let idf_path = version_path.join("esp-idf");
            match idf_im_lib::git_tools::get_esp_idf(
                idf_path.to_str().unwrap(),
                settings.repo_stub.as_deref(),
                &idf_version,
                settings.idf_mirror.as_deref(),
                true,
                tx,
            ) {
                Ok(_) => info!("ESP-IDF version {} downloaded successfully.", idf_version),
                Err(err) => {
                    error!("Failed to download ESP-IDF version {}: {}", idf_version, err);
                    build_summaries.push(summary);
                    continue; // Skip to next version
                }
            }
            handle.join().unwrap(); // Wait for progress bar thread to finish


            // Create a temporary venv for compote
            let compote_env = archive_dir.path().join("compote_env");
            ensure_path(compote_env.to_str().unwrap())
                .expect("Failed to create compote env directory");

            // Create virtual environment for compote using uv
            info!("Creating virtual environment for compote...");
            match execute_command(
                "uv",
                &[
                    "venv",
                    "--python",
                    &global_python_version,
                    compote_env.to_str().unwrap(),
                ],
            ) {
                Ok(output) => {
                    if !output.status.success() {
                        error!(
                            "Failed to create compote virtual environment: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        continue;
                    }
                    info!("Compote virtual environment created successfully.");
                }
                Err(err) => {
                    error!("Failed to create compote venv: {}", err);
                    continue;
                }
            }

            // Install idf-component-manager using uv pip
            info!("Installing idf-component-manager...");
            match execute_command(
                "uv",
                &[
                    "pip",
                    "install",
                    "--python",
                    compote_env.to_str().unwrap(),
                    "idf-component-manager",
                ],
            ) {
                Ok(output) => {
                    if !output.status.success() {
                        error!(
                            "Failed to install idf-component-manager: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        continue;
                    }
                    info!("idf-component-manager installed successfully.");
                }
                Err(err) => {
                    error!("Failed to install idf-component-manager: {}", err);
                    continue;
                }
            }

            // Determine compote executable path
            let compote_executable = match std::env::consts::OS {
                "windows" => compote_env.join("Scripts").join("compote.exe"),
                _ => compote_env.join("bin").join("compote"),
            };

            if !compote_executable.exists() {
                error!("Compote executable not found at: {:?}", compote_executable);
                continue;
            }

            // Create components download directory
            let components_dir = archive_dir.path().join("components");
            ensure_path(components_dir.to_str().unwrap())
                .expect("Failed to create components directory");

            // Build compote command arguments
            // Format: compote registry sync --component name==version --component name2==version2 --recursive [dest]
            let mut compote_args: Vec<String> = vec![
                "registry".to_string(),
                "sync".to_string(),
                "--resolution".to_string(),
                "latest".to_string(),
                "--recursive".to_string(),
            ];

            compote_args.push(components_dir.to_str().unwrap().to_string());

            info!(
                "Syncing components to {:?}...",
                components_dir
            );
            debug!("Compote command: {:?} {:?}", compote_executable, compote_args);

            match execute_command_with_dir(
                compote_executable.to_str().unwrap(),
                &compote_args.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
                idf_path.to_str().unwrap(),
            ) {
                Ok(output) => {
                    if output.status.success() {
                        info!(
                            "Successfully synced components.",
                        );
                        debug!("Compote output: {}", String::from_utf8_lossy(&output.stdout));
                    } else {
                        error!(
                            "Failed to sync components: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        // Don't fail the entire build, just warn
                        warn!("Component sync failed, continuing without components...");
                    }
                }
                Err(err) => {
                    error!("Failed to run compote: {}", err);
                    warn!("Component sync failed, continuing without components...");
                }
            }

            if let Err(e) = fs::remove_dir_all(&compote_env) {
                warn!("Failed to clean up compote environment: {}", e);
            }

            // Read tools.json
            let tools_json_file = idf_path
                .join(settings.tools_json_file.clone().unwrap_or_else(|| Settings::default().tools_json_file.unwrap()))
                .to_str()
                .expect("Failed to convert tools json path")
                .to_string();

            let tools = match idf_im_lib::idf_tools::read_and_parse_tools_file(&tools_json_file) {
                Ok(tools) => tools,
                Err(err) => {
                    error!("Failed to read tools json file: {}", err);
                    build_summaries.push(summary);
                    continue;
                }
            };

            let download_links = get_list_of_tools_to_download(
                tools.clone(),
                settings.clone().target.unwrap_or(vec!["all".to_string()]),
                settings.mirror.as_deref(),
            );

            let tool_path = archive_dir.path().join("dist");
            ensure_path(tool_path.to_str().unwrap()).expect("Failed to ensure tools path");

            for (tool_name, (version, download_link)) in download_links.iter() {
                info!("Preparing tool: {} version: {} from: {}", tool_name, version, download_link.url);
                match download_file(&download_link.url, tool_path.to_str().unwrap(), None).await {
                    Ok(_) => {
                        let filename = Path::new(&download_link.url).file_name().unwrap().to_str().unwrap();
                        let full_file_path = tool_path.join(filename);

                        if verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap()).unwrap() {
                            info!("Tool {} version {} downloaded and verified.", tool_name, version);
                        } else {
                            error!("Checksum failed for tool {} version {}.", tool_name, version);
                            continue;
                        }
                    }
                    Err(err) => {
                        error!("Failed to download tool {}: {}", tool_name, err);
                        continue;
                    }
                }
            }

            // Download constraints file
            let constrains_idf_version = match parse_cmake_version(idf_path.to_str().unwrap()) {
                Ok((maj, min)) => format!("v{}.{}", maj, min),
                Err(e) => {
                    warn!("Failed to parse CMake version: {}", e);
                    idf_version.to_string()
                }
            };
            info!("Using constraints IDF version: {} from CMake", constrains_idf_version);

            let constraint_file = match download_constraints_file(&archive_dir.path(), &constrains_idf_version).await {
                Ok(file) => {
                    info!("Downloaded constraints: {}", file.display());
                    file
                }
                Err(e) => {
                    error!("Failed to download constraints for {}: {}", idf_version, e);
                    build_summaries.push(summary);
                    continue;
                }
            };

            // Merge requirements
            let requirements_dir = idf_path.join("tools").join("requirements");
            if let Err(e) = merge_requirements_files(&requirements_dir) {
                error!("Failed to merge requirements: {}", e);
                build_summaries.push(summary);
                continue;
            }

            let requirements_file = requirements_dir.join("requirements.merged.txt");

            let wheel_versions: Vec<&str> = wheel_python_versions.iter().map(|s| s.as_str()).collect();
            let python_results = download_wheels_for_python_versions(
                archive_dir.path(),
                &requirements_file,
                &constraint_file,
                &wheel_versions,
            ).await;

            summary.python_versions = python_results;

            // Check if we have at least one successful Python version
            let has_any_success = summary.python_versions.iter().any(|p| p.success);
            if !has_any_success {
                error!("All Python versions failed for {}. Skipping archive creation.", idf_version);
                build_summaries.push(summary);
                continue;
            } else if summary.any_python_failed() {
                warn!("Some Python versions failed for {}, but continuing with archive creation", idf_version);
            }

            // Save settings for this version
            let mut version_settings = settings.clone();
            version_settings.idf_versions = Some(vec![idf_version.clone()]);
            version_settings.config_file_save_path = Some(archive_dir.path().join("config.toml"));
            version_settings.idf_path = None;

            if let Err(e) = version_settings.save() {
                error!("Failed to save settings for {}: {}", idf_version, e);
                build_summaries.push(summary);
                continue;
            }

            // CREATE INDIVIDUAL ARCHIVE PER VERSION
            let output_path = PathBuf::from(format!("archive_{}.zst", idf_version));
            let mut output_file = match File::create(&output_path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create output file {}: {}", output_path.display(), e);
                    build_summaries.push(summary);
                    continue;
                }
            };

            // Tar + Zstd compress
            let mut tar = TarBuilder::new(Vec::new());
            if let Err(e) = tar.append_dir_all(".", archive_dir.path()) {
                error!("Failed to create tar for {}: {}", idf_version, e);
                build_summaries.push(summary);
                continue;
            }

            let tar_data = match tar.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to finalize tar for {}: {}", idf_version, e);
                    build_summaries.push(summary);
                    continue;
                }
            };

            let compressed_data = match encode_all(&tar_data[..], 3) {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to compress with zstd for {}: {}", idf_version, e);
                    build_summaries.push(summary);
                    continue;
                }
            };

            if let Err(e) = output_file.write_all(&compressed_data) {
                error!("Failed to write compressed data for {}: {}", idf_version, e);
                build_summaries.push(summary);
                continue;
            }

            // Get archive size
            let archive_size = match fs::metadata(&output_path) {
                Ok(metadata) => Some(metadata.len()),
                Err(e) => {
                    warn!("Failed to get archive size for {}: {}", idf_version, e);
                    None
                }
            };

            summary.archive_created = true;
            summary.archive_path = Some(output_path.display().to_string());
            summary.archive_size = archive_size;

            info!("✅ Archive for {} saved to: {:?}", idf_version, output_path);
            build_summaries.push(summary);
        }

        // Write GitHub Actions summary
        write_github_summary(&build_summaries);

        // Determine exit code based on results
        let all_successful = build_summaries.iter().all(|s| s.archive_created && s.all_python_successful());
        if !all_successful {
            error!("Some builds failed or had warnings. Check the summary above.");
        } else {
            info!("🎉 All requested versions processed successfully.");
        }
    } else if let Some(archive_path) = args.archive {
        // Extract installation data from archive
        info!("Extracting installation data from archive: {:?}", archive_path);

        if !archive_path.exists() {
            error!("Archive file does not exist: {:?}", archive_path);
            return;
        }

        // Create extraction directory next to the archive
        let archive_stem = archive_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted");

        let extract_dir = archive_path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{}_extracted", archive_stem));

        match extract_zst_archive(&archive_path, &extract_dir) {
            Ok(_) => {
                info!("Successfully extracted archive to: {:?}", extract_dir);
                info!("You can now examine the contents for debugging purposes.");
            }
            Err(err) => {
                error!("Failed to extract archive: {}", err);
            }
        }
    } else {
        error!("Please specify either -c to create an archive or -a to extract one.");
        error!("Use --help for more information.");
    }
}
