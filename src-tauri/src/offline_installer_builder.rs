use clap::Parser;
use fern::Dispatch;
use idf_im_lib::cli_progress;
use idf_im_lib::command_executor::{
    execute_command, execute_command_with_dir, execute_command_with_env,
};
use idf_im_lib::download_file;
use idf_im_lib::download_file_and_rename;
use idf_im_lib::ensure_path;
use idf_im_lib::get_log_directory;
use idf_im_lib::git_tools::ProgressMessage;
use idf_im_lib::idf_tools::get_list_of_tools_to_download;
use idf_im_lib::idf_tools::Download;
use idf_im_lib::logging;
use idf_im_lib::offline_installer::merge_requirements_files;
use idf_im_lib::python_utils::download_constraints_file;
use idf_im_lib::settings::Settings;
use idf_im_lib::system_dependencies::get_latest_git_for_windows_url;
use idf_im_lib::utils::extract_zst_archive;
use idf_im_lib::utils::parse_cmake_version;
use idf_im_lib::verify_file_checksum;
use log::debug;
use log::error;
use log::info;
use log::warn;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::mpsc;
use std::thread;
use tar::Builder as TarBuilder;
use tempfile::TempDir;
use zstd::encode_all;

pub const PYTHON_VERSION: &str = "3.11";
pub const SUPPORTED_PYTHON_VERSIONS: &[&str] = &["3.10", "3.11", "3.12", "3.13", "3.14"];

const ESPRESSIF_PYPI_INDEX: &str = "https://dl.espressif.com/pypi/";
const PYPI_INDEX: &str = "https://pypi.org/simple";
const WINDOWS_PYTHON_URL: &str = "https://github.com/astral-sh/python-build-standalone/releases/download/20260414/cpython-3.11.15+20260414-x86_64-pc-windows-msvc-install_only.tar.gz";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PythonVersionResult {
    version: String,
    success: bool,
    error_message: Option<String>,
    source_built_packages: Vec<String>,
}

impl PythonVersionResult {
    fn failed(version: &str, error_message: String, source_built_packages: Vec<String>) -> Self {
        Self {
            version: version.to_string(),
            success: false,
            error_message: Some(error_message),
            source_built_packages,
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

    fn is_fully_successful(&self) -> bool {
        self.archive_created && self.all_python_successful()
    }

    fn status_icon(&self) -> &'static str {
        if self.is_fully_successful() {
            "✅"
        } else if self.archive_created && self.any_python_failed() {
            "⚠️"
        } else {
            "❌"
        }
    }

    fn to_github_summary(&self) -> String {
        let mut summary = format!(
            "{} **{}** (`{}`)\n",
            self.status_icon(),
            self.idf_version,
            self.architecture
        );

        if self.is_fully_successful() {
            self.push_success_details(&mut summary);
        } else {
            summary.push('\n');
            self.push_problem_details(&mut summary);
        }

        summary.push('\n');
        summary
    }

    fn push_success_details(&self, summary: &mut String) {
        summary.push_str(&format!(
            "- Python versions: {}\n",
            join_versions(&self.python_versions)
        ));
        if let Some(size) = self.archive_size {
            summary.push_str(&format!("- Archive size: {}\n", format_megabytes(size)));
        }

        let source_built: Vec<_> = self
            .python_versions
            .iter()
            .filter(|p| !p.source_built_packages.is_empty())
            .collect();
        if !source_built.is_empty() {
            summary.push_str("\n**Packages built from source:**\n");
            for pv in source_built {
                summary.push_str(&format!(
                    "- Python {}: {}\n",
                    pv.version,
                    pv.source_built_packages.join(", ")
                ));
            }
        }
    }

    fn push_problem_details(&self, summary: &mut String) {
        let (successful, failed): (Vec<_>, Vec<_>) =
            self.python_versions.iter().partition(|p| p.success);

        if !successful.is_empty() {
            summary.push_str(&format!(
                "**✅ Successful Python versions:** {}\n\n",
                join_versions(successful)
            ));
        }

        if !failed.is_empty() {
            summary.push_str(&format!(
                "**❌ Failed Python versions:** {}\n\n",
                join_versions(failed.iter().copied())
            ));

            summary.push_str("<details>\n<summary>Error Details</summary>\n\n");
            for pv in failed {
                summary.push_str(&format!(
                    "**Python {}:**\n```\n{}\n```\n\n",
                    pv.version,
                    pv.error_message.as_deref().unwrap_or("Unknown error")
                ));
            }
            summary.push_str("</details>\n\n");
        }

        if !self.archive_created {
            summary.push_str("**Status:** Archive creation failed\n");
        } else if let Some(size) = self.archive_size {
            summary.push_str(&format!("**Archive size:** {}\n", format_megabytes(size)));
        }
    }
}

fn join_versions<'a>(results: impl IntoIterator<Item = &'a PythonVersionResult>) -> String {
    results
        .into_iter()
        .map(|p| p.version.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_megabytes(size: u64) -> String {
    format!("{:.2} MB", size as f64 / 1_048_576.0)
}

fn all_builds_successful(summaries: &[BuildSummary]) -> bool {
    summaries.iter().all(BuildSummary::is_fully_successful)
}

fn overall_status_heading(summaries: &[BuildSummary]) -> Option<&'static str> {
    let any_warnings = summaries
        .iter()
        .any(|s| s.archive_created && s.any_python_failed());
    let any_failures = summaries
        .iter()
        .any(|s| !s.archive_created || s.all_python_failed());

    if all_builds_successful(summaries) {
        Some("## ✅ All builds successful\n\n")
    } else if any_failures {
        Some("## ❌ Build failures detected\n\n")
    } else if any_warnings {
        Some("## ⚠️ Builds completed with warnings\n\n")
    } else {
        None
    }
}

fn build_summary_markdown(summaries: &[BuildSummary]) -> String {
    let mut content = String::new();
    content.push_str("# Offline Installer Build Summary\n\n");

    if let Some(heading) = overall_status_heading(summaries) {
        content.push_str(heading);
    }

    for summary in summaries {
        content.push_str(&summary.to_github_summary());
    }
    content
}

fn write_github_summary(summaries: &[BuildSummary]) {
    let github_step_summary = std::env::var("GITHUB_STEP_SUMMARY");

    let output = github_step_summary.ok();

    let content = build_summary_markdown(summaries);

    // Write to file if in GitHub Actions
    if let Some(file_path) = output {
        if let Err(e) = fs::write(&file_path, &content) {
            error!(
                "Failed to write GitHub step summary to {}: {}",
                file_path, e
            );
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
    let log_dir = custom_log_dir
        .or_else(get_log_directory)
        .unwrap_or_else(|| PathBuf::from("."));
    let log_file_path = log_dir.join("offline_installer.log");

    // Ensure log directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "Failed to create log directory {}: {}",
            log_dir.display(),
            e
        );
    }

    Dispatch::new()
        .format(logging::formatter)
        .level(log_level)
        .chain(fern::log_file(&log_file_path)?)
        .chain(std::io::stderr())
        .apply()?;

    log::debug!(
        "Offline installer logging initialized at level: {:?}",
        log_level
    );
    Ok(())
}

fn python_env_dir_name(python_version: &str) -> String {
    format!("python_env_{}", python_version.replace('.', "_"))
}

fn wheel_dir_name(python_version: &str) -> String {
    format!("wheels_py{}", python_version.replace('.', ""))
}

fn venv_python_executable(python_env: &Path, os: &str) -> PathBuf {
    match os {
        "windows" => python_env.join("Scripts/python.exe"),
        _ => python_env.join("bin/python"),
    }
}

fn compote_executable_path(compote_env: &Path, os: &str) -> PathBuf {
    match os {
        "windows" => compote_env.join("Scripts").join("compote.exe"),
        _ => compote_env.join("bin").join("compote"),
    }
}

fn pip_download_args<'a>(
    requirements: &'a str,
    constraints: &'a str,
    wheel_dir: &'a str,
    binary_only: bool,
) -> Vec<&'a str> {
    let mut args = vec!["-m", "pip", "download"];
    if !binary_only {
        args.push("--verbose");
    }
    args.extend(["-r", requirements, "-c", constraints, "--dest", wheel_dir]);
    if binary_only {
        args.push("--only-binary=:all:");
    }
    args.extend([
        "--index-url",
        ESPRESSIF_PYPI_INDEX,
        "--extra-index-url",
        PYPI_INDEX,
    ]);
    args
}

/// Package names from pip's "Building wheel for <pkg>" lines, deduplicated in first-seen order.
fn parse_source_built_packages(pip_output: &str) -> Vec<String> {
    let mut packages: Vec<String> = Vec::new();
    for line in pip_output.lines() {
        if !line.contains("Building wheel for") {
            continue;
        }
        let pkg = line
            .split("Building wheel for ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next());
        if let Some(pkg) = pkg {
            if !packages.contains(&pkg.to_string()) {
                packages.push(pkg.to_string());
            }
        }
    }
    packages
}

fn install_python_versions(python_versions: &[&str]) -> Vec<PythonVersionResult> {
    let mut results = Vec::new();
    for python_version in python_versions {
        info!("Installing Python {}...", python_version);
        match execute_command("uv", &["python", "install", python_version]) {
            Ok(output) if output.status.success() => {
                info!("Python {} installed successfully.", python_version);
            }
            Ok(output) => {
                warn!(
                    "Python {} might already be installed or failed to install: {}",
                    python_version,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(err) => {
                error!("Failed to install Python {}: {}", python_version, err);
                results.push(PythonVersionResult::failed(
                    python_version,
                    format!("Failed to install Python: {}", err),
                    vec![],
                ));
            }
        }
    }
    results
}

fn create_python_dirs(
    python_env: &Path,
    wheel_dir: &Path,
    python_version: &str,
) -> Result<(), String> {
    if let Err(e) = ensure_path(python_env.to_str().unwrap()) {
        error!(
            "Failed to create Python env directory for {}: {}",
            python_version, e
        );
        return Err(format!("Failed to create directory: {}", e));
    }
    if let Err(e) = ensure_path(wheel_dir.to_str().unwrap()) {
        error!(
            "Failed to create wheel directory for {}: {}",
            python_version, e
        );
        return Err(format!("Failed to create directory: {}", e));
    }
    Ok(())
}

fn create_python_venv(python_env: &Path, python_version: &str) -> Result<(), String> {
    info!(
        "Creating virtual environment for Python {}...",
        python_version
    );

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
        Ok(output) if !output.status.success() => {
            let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
            error!(
                "Failed to create Python {} virtual environment: {}",
                python_version, error_msg
            );
            Err(format!("venv creation failed: {}", error_msg))
        }
        Ok(_) => {
            info!(
                "Python {} virtual environment created successfully.",
                python_version
            );
            Ok(())
        }
        Err(err) => {
            error!(
                "Failed to create venv for Python {}: {}",
                python_version, err
            );
            Err(format!("venv creation error: {}", err))
        }
    }
}

fn install_pip(python_executable: &Path, python_version: &str) -> Result<(), String> {
    info!("Installing pip into venv for Python {}...", python_version);
    match execute_command(python_executable.to_str().unwrap(), &["-m", "ensurepip"]) {
        Ok(output) if output.status.success() => {
            info!("Pip installed into venv for Python {}.", python_version);
            Ok(())
        }
        Ok(output) => {
            error!(
                "Failed to install pip into venv: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Err(format!(
                "Failed to install pip: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
        Err(err) => {
            error!("Failed to install pip: {}", err);
            Err(format!("Failed to install pip: {}", err))
        }
    }
}

/// Creates the venv and wheel directory for one Python version and returns the venv's interpreter.
fn prepare_python_env(
    python_env: &Path,
    wheel_dir: &Path,
    python_version: &str,
) -> Result<PathBuf, String> {
    create_python_dirs(python_env, wheel_dir, python_version)?;
    create_python_venv(python_env, python_version)?;
    let python_executable = venv_python_executable(python_env, std::env::consts::OS);
    install_pip(&python_executable, python_version)?;
    Ok(python_executable)
}

/// Downloads binary wheels, falling back to allowing source builds; returns the final pip result
/// and the packages that had to be built from source.
fn download_python_packages(
    python_executable: &Path,
    requirements_path: &Path,
    constraint_file: &Path,
    wheel_dir: &Path,
    python_version: &str,
) -> (std::io::Result<Output>, Vec<String>) {
    info!("Downloading packages for Python {}...", python_version);

    info!(
        "STEP 1: Attempting binary-only download for Python {}...",
        python_version
    );

    std::env::set_var("PIP_MAX_ROUNDS", "200");

    let python = python_executable.to_str().unwrap();
    let requirements = requirements_path.to_str().unwrap();
    let constraints = constraint_file.to_str().unwrap();
    let wheels = wheel_dir.to_str().unwrap();

    let mut source_built = Vec::new();
    let mut result = execute_command_with_env(
        python,
        &pip_download_args(requirements, constraints, wheels, true),
        vec![("PIP_MAX_ROUNDS", "300")],
    );

    if result.is_err() || !result.as_ref().unwrap().status.success() {
        warn!("Binary-only download failed for Python {}", python_version);

        if let Ok(ref output) = result {
            let stderr = String::from_utf8_lossy(&output.stderr);
            info!(
                "Binary-only error: {}",
                stderr.lines().take(5).collect::<Vec<_>>().join(" | ")
            );
        }

        info!(
            "STEP 2: Retrying with source builds allowed for Python {}...",
            python_version
        );

        result = execute_command_with_env(
            python,
            &pip_download_args(requirements, constraints, wheels, false),
            vec![("PIP_MAX_ROUNDS", "300")],
        );

        if let Ok(ref output) = result {
            source_built = report_source_built_packages(output, python_version);
        }
    } else {
        info!(
            "Binary-only download succeeded for Python {}",
            python_version
        );
    }

    std::env::remove_var("PIP_MAX_ROUNDS");

    (result, source_built)
}

fn report_source_built_packages(output: &Output, python_version: &str) -> Vec<String> {
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let source_built = parse_source_built_packages(&combined);
    for pkg in &source_built {
        warn!("Building package from source: {}", pkg);
    }

    if !source_built.is_empty() {
        warn!(
            "Python {}: Built {} packages from source: {:?}",
            python_version,
            source_built.len(),
            source_built
        );
    }
    source_built
}

fn python_download_result(
    python_version: &str,
    result: std::io::Result<Output>,
    source_built: Vec<String>,
) -> PythonVersionResult {
    match result {
        Ok(output) if output.status.success() => {
            info!(
                "Python {} packages downloaded successfully.",
                python_version
            );
            if !source_built.is_empty() {
                info!(
                    "Packages built from source for Python {}: {:?}",
                    python_version, source_built
                );
            }
            PythonVersionResult {
                version: python_version.to_string(),
                success: true,
                error_message: None,
                source_built_packages: source_built,
            }
        }
        Ok(output) => {
            let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
            error!(
                "Failed to download Python {} packages: {}",
                python_version, error_msg
            );
            PythonVersionResult::failed(python_version, error_msg, source_built)
        }
        Err(err) => {
            error!(
                "Failed to download packages for Python {}: {}",
                python_version, err
            );
            PythonVersionResult::failed(python_version, err.to_string(), source_built)
        }
    }
}

fn download_wheels_for_python_version(
    archive_dir: &Path,
    requirements_path: &Path,
    constraint_file: &Path,
    python_version: &str,
) -> PythonVersionResult {
    let python_env = archive_dir.join(python_env_dir_name(python_version));
    let wheel_dir = archive_dir.join(wheel_dir_name(python_version));

    let python_executable = match prepare_python_env(&python_env, &wheel_dir, python_version) {
        Ok(executable) => executable,
        Err(message) => return PythonVersionResult::failed(python_version, message, vec![]),
    };

    info!("Upgrading pip for Python {}...", python_version);
    let _ = execute_command(
        python_executable.to_str().unwrap(),
        &["-m", "pip", "install", "--upgrade", "pip"],
    );

    let (result, source_built) = download_python_packages(
        &python_executable,
        requirements_path,
        constraint_file,
        &wheel_dir,
        python_version,
    );
    python_download_result(python_version, result, source_built)
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
    info!(
        "Downloading wheels for Python versions: {:?}",
        python_versions
    );

    let mut results = install_python_versions(python_versions);

    for python_version in python_versions {
        if results
            .iter()
            .any(|r| r.version == *python_version && !r.success)
        {
            continue;
        }

        info!("Processing Python version: {}", python_version);

        results.push(download_wheels_for_python_version(
            archive_dir,
            requirements_path,
            constraint_file,
            python_version,
        ));
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

#[derive(Debug, PartialEq)]
enum Mode<'a> {
    ListVersions,
    Create(&'a str),
    Extract(&'a Path),
    Missing,
}

fn select_mode(args: &Args) -> Mode<'_> {
    if args.list_versions {
        Mode::ListVersions
    } else if let Some(config) = args.create_from_config.as_deref() {
        Mode::Create(config)
    } else if let Some(archive) = args.archive.as_deref() {
        Mode::Extract(archive)
    } else {
        Mode::Missing
    }
}

fn resolve_wheel_python_versions(requested: Option<Vec<String>>) -> Vec<String> {
    requested.unwrap_or_else(|| {
        SUPPORTED_PYTHON_VERSIONS
            .iter()
            .map(|s| s.to_string())
            .collect()
    })
}

fn select_idf_versions(
    override_version: Option<String>,
    build_all_versions: bool,
    configured: Option<Vec<String>>,
    available: &[String],
) -> Vec<String> {
    if let Some(override_version) = override_version {
        info!("Using IDF version override: {}", override_version);
        vec![override_version]
    } else if build_all_versions {
        info!(
            "Building separate archives for all supported versions: {:?}",
            available
        );
        available.to_vec()
    } else {
        configured.unwrap_or(vec![available.first().unwrap().clone()])
    }
}

fn archive_output_path(idf_version: &str) -> PathBuf {
    PathBuf::from(format!("archive_{}.zst", idf_version))
}

fn extraction_dir(archive_path: &Path) -> PathBuf {
    let archive_stem = archive_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("extracted");

    archive_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}_extracted", archive_stem))
}

fn windows_prerequisite_downloads(git_url: String) -> Vec<(String, &'static str)> {
    vec![
        // Git - portable Windows distribution (fetched dynamically from GitHub)
        // Download directly with simple name for reliable extraction
        (git_url, "git.tar.bz2"),
        // Python - standalone Windows distribution (matches system_dependencies.rs)
        (WINDOWS_PYTHON_URL.to_string(), "python.tar.gz"),
    ]
}

fn version_settings(settings: &Settings, idf_version: &str, archive_dir: &Path) -> Settings {
    let mut version_settings = settings.clone();
    version_settings.idf_versions = Some(vec![idf_version.to_string()]);
    version_settings.config_file_save_path = Some(archive_dir.join("config.toml"));
    version_settings.idf_path = None;
    version_settings
}

fn load_settings(config_path: &str) -> Option<Settings> {
    if config_path == "default" {
        let settings = Settings::default();
        info!("Default settings loaded: {:?}", settings);
        return Some(settings);
    }

    let mut settings = Settings::default();
    match settings.load(config_path) {
        Ok(_) => {
            info!("Settings loaded from {}: {:?}", config_path, settings);
            Some(settings)
        }
        Err(e) => {
            error!("Failed to load settings from {}: {}", config_path, e);
            None
        }
    }
}

fn uv_is_installed() -> bool {
    match execute_command("uv", &["--version"]) {
        Ok(output) if output.status.success() => {
            info!("UV is installed: {:?}", output);
            true
        }
        Ok(output) => {
            error!("UV is not installed or not found: {:?}", output);
            false
        }
        Err(err) => {
            error!(
                "UV is not installed or not found: {}. Please install it and try again.",
                err
            );
            false
        }
    }
}

async fn download_windows_prerequisites() -> Option<TempDir> {
    let temp_shared = TempDir::new().expect("Failed to create shared prereq temp dir");
    let prereq_path = temp_shared.path();
    ensure_path(prereq_path.to_str().unwrap()).expect("Failed to create prereq dir");

    // Fetch latest Git for Windows portable URL dynamically
    let (git_url, _git_filename) = match get_latest_git_for_windows_url().await {
        Ok(url) => url,
        Err(err) => {
            error!("Failed to get latest Git for Windows URL: {}", err);
            return None;
        }
    };

    for (link, name) in windows_prerequisite_downloads(git_url) {
        info!("Downloading prerequisite: {} as {}", link, name);
        match download_file_and_rename(&link, prereq_path.to_str().unwrap(), None, Some(name), 3)
            .await
        {
            Ok(_) => info!("Downloaded: {}", name),
            Err(err) => {
                error!("Failed to download {}: {}", name, err);
                return None;
            }
        }
    }

    info!(
        "Shared Windows prerequisites downloaded to: {:?}",
        temp_shared.path()
    );
    Some(temp_shared)
}

/// `Err` aborts the whole run; `Ok(None)` means there is nothing to share on this OS.
async fn prepare_shared_prerequisites() -> Result<Option<TempDir>, ()> {
    match std::env::consts::OS {
        "windows" => download_windows_prerequisites().await.map(Some).ok_or(()),
        "linux" | "macos" => {
            info!("Detected Unix-like OS, prerequisites installation not implemented — skipping.");
            Ok(None)
        }
        os => {
            error!("Unsupported OS: {}", os);
            Err(())
        }
    }
}

fn show_download_progress(rx: mpsc::Receiver<ProgressMessage>) {
    cli_progress::show_download_progress(
        rx,
        |name, value| format!("{}: {}", name, value),
        |name| info!("submodule: {}", name),
    )
}

fn download_idf_repo(settings: &Settings, idf_version: &str, idf_path: &Path) -> bool {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || show_download_progress(rx));

    match idf_im_lib::git_tools::get_esp_idf(
        idf_path.to_str().unwrap(),
        settings.repo_stub.as_deref(),
        idf_version,
        settings.idf_mirror.as_deref(),
        true,
        tx,
    ) {
        Ok(_) => info!("ESP-IDF version {} downloaded successfully.", idf_version),
        Err(err) => {
            error!(
                "Failed to download ESP-IDF version {}: {}",
                idf_version, err
            );
            return false;
        }
    }
    handle.join().unwrap(); // Wait for progress bar thread to finish
    true
}

/// Logs `"{failed}: {stderr}"` for a non-zero exit, or `"{errored}: {err}"` if it could not run.
fn command_succeeded(result: std::io::Result<Output>, failed: &str, errored: &str) -> bool {
    match result {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            error!("{}: {}", failed, String::from_utf8_lossy(&output.stderr));
            false
        }
        Err(err) => {
            error!("{}: {}", errored, err);
            false
        }
    }
}

fn install_compote(compote_env: &Path, python_version: &str) -> Option<PathBuf> {
    info!("Creating virtual environment for compote...");
    let venv = execute_command(
        "uv",
        &[
            "venv",
            "--python",
            python_version,
            compote_env.to_str().unwrap(),
        ],
    );
    if !command_succeeded(
        venv,
        "Failed to create compote virtual environment",
        "Failed to create compote venv",
    ) {
        return None;
    }
    info!("Compote virtual environment created successfully.");

    info!("Installing idf-component-manager...");
    let install = execute_command(
        "uv",
        &[
            "pip",
            "install",
            "--python",
            compote_env.to_str().unwrap(),
            "idf-component-manager",
        ],
    );
    if !command_succeeded(
        install,
        "Failed to install idf-component-manager",
        "Failed to install idf-component-manager",
    ) {
        return None;
    }
    info!("idf-component-manager installed successfully.");

    let compote_executable = compote_executable_path(compote_env, std::env::consts::OS);
    if !compote_executable.exists() {
        error!("Compote executable not found at: {:?}", compote_executable);
        return None;
    }
    Some(compote_executable)
}

/// Compote failures are logged but never fail the build.
fn report_compote_sync(result: std::io::Result<Output>, what: &str) {
    match result {
        Ok(output) if output.status.success() => {
            info!("Successfully synced {}.", what);
            debug!(
                "Compote output: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        Ok(output) => {
            error!(
                "Failed to sync {}: {}",
                what,
                String::from_utf8_lossy(&output.stderr)
            );
            warn!("Component sync failed, continuing without {}...", what);
        }
        Err(err) => {
            error!("Failed to run compote: {}", err);
            warn!("Component sync failed, continuing without {}...", what);
        }
    }
}

fn compote_registry_sync_args(components_dir: &str) -> Vec<&str> {
    vec![
        "registry",
        "sync",
        "--resolution",
        "latest",
        "--recursive",
        components_dir,
    ]
}

/// Returns `false` when the compote tooling itself could not be set up; the version is then
/// skipped without being recorded in the build summary.
fn sync_components(archive_dir: &Path, idf_path: &Path, python_version: &str) -> bool {
    let compote_env = archive_dir.join("compote_env");
    ensure_path(compote_env.to_str().unwrap()).expect("Failed to create compote env directory");

    let Some(compote_executable) = install_compote(&compote_env, python_version) else {
        return false;
    };

    let components_dir = archive_dir.join("components");
    let required_components_dir = archive_dir.join("required_components");
    ensure_path(components_dir.to_str().unwrap()).expect("Failed to create components directory");
    ensure_path(required_components_dir.to_str().unwrap())
        .expect("Failed to create components directory");

    let compote_args = compote_registry_sync_args(components_dir.to_str().unwrap());

    info!("Syncing components to {:?}...", components_dir);
    debug!(
        "Compote command: {:?} {:?}",
        compote_executable, compote_args
    );

    report_compote_sync(
        execute_command_with_dir(
            compote_executable.to_str().unwrap(),
            &compote_args,
            idf_path.to_str().unwrap(),
        ),
        "components",
    );

    let compote_args_required_components: Vec<&str> = vec!["cooking", "stock"];

    debug!(
        "Compote command: {:?} {:?}",
        compote_executable, compote_args_required_components
    );

    let env_for_compote = vec![
        ("IDF_PATH", idf_path.to_str().unwrap()),
        ("IDF_TOOLS_PATH", required_components_dir.to_str().unwrap()),
    ];

    report_compote_sync(
        execute_command_with_env(
            compote_executable.to_str().unwrap(),
            &compote_args_required_components,
            env_for_compote,
        ),
        "Root Managed Components",
    );

    if let Err(e) = fs::remove_dir_all(&compote_env) {
        warn!("Failed to clean up compote environment: {}", e);
    }
    true
}

async fn download_tool(tool_path: &Path, tool_name: &str, version: &str, download_link: &Download) {
    info!(
        "Preparing tool: {} version: {} from: {}",
        tool_name, version, download_link.url
    );
    if let Err(err) = download_file(&download_link.url, tool_path.to_str().unwrap(), None).await {
        error!("Failed to download tool {}: {}", tool_name, err);
        return;
    }

    let filename = Path::new(&download_link.url)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let full_file_path = tool_path.join(filename);

    if verify_file_checksum(&download_link.sha256, full_file_path.to_str().unwrap()).unwrap() {
        info!(
            "Tool {} version {} downloaded and verified.",
            tool_name, version
        );
    } else {
        error!(
            "Checksum failed for tool {} version {}.",
            tool_name, version
        );
    }
}

async fn download_tools(settings: &Settings, idf_path: &Path, archive_dir: &Path) -> bool {
    let tools_json_file = idf_path
        .join(
            settings
                .tools_json_file
                .clone()
                .unwrap_or_else(|| Settings::default().tools_json_file.unwrap()),
        )
        .to_str()
        .expect("Failed to convert tools json path")
        .to_string();

    let tools = match idf_im_lib::idf_tools::read_and_parse_tools_file(&tools_json_file) {
        Ok(tools) => tools,
        Err(err) => {
            error!("Failed to read tools json file: {}", err);
            return false;
        }
    };

    let download_links = get_list_of_tools_to_download(
        tools.clone(),
        settings.clone().target.unwrap_or(vec!["all".to_string()]),
        settings.mirror.as_deref(),
    );

    let tool_path = archive_dir.join("dist");
    ensure_path(tool_path.to_str().unwrap()).expect("Failed to ensure tools path");

    for (tool_name, (version, download_link)) in download_links.iter() {
        download_tool(&tool_path, tool_name, version, download_link).await;
    }
    true
}

async fn download_constraints(
    archive_dir: &Path,
    idf_path: &Path,
    idf_version: &str,
) -> Option<PathBuf> {
    let constrains_idf_version = match parse_cmake_version(idf_path.to_str().unwrap()) {
        Ok((maj, min)) => format!("v{}.{}", maj, min),
        Err(e) => {
            warn!("Failed to parse CMake version: {}", e);
            idf_version.to_string()
        }
    };
    info!(
        "Using constraints IDF version: {} from CMake",
        constrains_idf_version
    );

    match download_constraints_file(archive_dir, &constrains_idf_version).await {
        Ok(file) => {
            info!("Downloaded constraints: {}", file.display());
            Some(file)
        }
        Err(e) => {
            error!("Failed to download constraints for {}: {}", idf_version, e);
            None
        }
    }
}

fn python_results_allow_archive(summary: &BuildSummary, idf_version: &str) -> bool {
    let has_any_success = summary.python_versions.iter().any(|p| p.success);
    if !has_any_success {
        error!(
            "All Python versions failed for {}. Skipping archive creation.",
            idf_version
        );
        return false;
    }
    if summary.any_python_failed() {
        warn!(
            "Some Python versions failed for {}, but continuing with archive creation",
            idf_version
        );
    }
    true
}

fn write_archive(archive_dir: &Path, output_path: &Path, idf_version: &str) -> bool {
    let mut output_file = match File::create(output_path) {
        Ok(f) => f,
        Err(e) => {
            error!(
                "Failed to create output file {}: {}",
                output_path.display(),
                e
            );
            return false;
        }
    };

    // Tar + Zstd compress
    let mut tar = TarBuilder::new(Vec::new());
    tar.follow_symlinks(false);
    if let Err(e) = tar.append_dir_all(".", archive_dir) {
        error!("Failed to create tar for {}: {}", idf_version, e);
        return false;
    }

    let tar_data = match tar.into_inner() {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to finalize tar for {}: {}", idf_version, e);
            return false;
        }
    };

    let compressed_data = match encode_all(&tar_data[..], 3) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to compress with zstd for {}: {}", idf_version, e);
            return false;
        }
    };

    if let Err(e) = output_file.write_all(&compressed_data) {
        error!("Failed to write compressed data for {}: {}", idf_version, e);
        return false;
    }
    true
}

struct VersionBuildContext<'a> {
    settings: &'a Settings,
    architecture: &'a str,
    shared_prereq_dir: Option<&'a TempDir>,
    global_python_version: &'a str,
    wheel_python_versions: &'a [String],
}

/// Builds the archive for one IDF version. `None` means the version is left out of the summary.
async fn build_version_archive(
    ctx: &VersionBuildContext<'_>,
    idf_version: String,
) -> Option<BuildSummary> {
    let mut summary = BuildSummary::new(idf_version.clone(), ctx.architecture.to_string());

    info!("=== Processing ESP-IDF version: {} ===", idf_version);

    // Create a fresh temp dir for this version
    let archive_dir = TempDir::new().expect("Failed to create version-specific temp dir");
    let version_path = archive_dir.path().join(&idf_version);
    ensure_path(version_path.to_str().unwrap()).expect("Failed to ensure version path");

    if let Some(shared_dir) = ctx.shared_prereq_dir {
        // Copy CONTENTS of shared_dir into archive_dir (not the dir itself)
        // Archives are already named simply (git.tar.bz2, python.tar.gz) for reliable lookup
        info!("Copying shared prerequisites to: {:?}", archive_dir.path());
        idf_im_lib::utils::copy_dir_contents(shared_dir.path(), archive_dir.path())
            .expect("Failed to copy shared prerequisites");
    }

    let idf_path = version_path.join("esp-idf");
    if !download_idf_repo(ctx.settings, &idf_version, &idf_path) {
        return Some(summary);
    }

    if !sync_components(archive_dir.path(), &idf_path, ctx.global_python_version) {
        return None;
    }

    if !download_tools(ctx.settings, &idf_path, archive_dir.path()).await {
        return Some(summary);
    }

    let Some(constraint_file) =
        download_constraints(archive_dir.path(), &idf_path, &idf_version).await
    else {
        return Some(summary);
    };

    let requirements_dir = idf_path.join("tools").join("requirements");
    if let Err(e) = merge_requirements_files(&requirements_dir) {
        error!("Failed to merge requirements: {}", e);
        return Some(summary);
    }

    let requirements_file = requirements_dir.join("requirements.merged.txt");

    let wheel_versions: Vec<&str> = ctx
        .wheel_python_versions
        .iter()
        .map(|s| s.as_str())
        .collect();
    summary.python_versions = download_wheels_for_python_versions(
        archive_dir.path(),
        &requirements_file,
        &constraint_file,
        &wheel_versions,
    )
    .await;

    if !python_results_allow_archive(&summary, &idf_version) {
        return Some(summary);
    }

    if let Err(e) = version_settings(ctx.settings, &idf_version, archive_dir.path()).save() {
        error!("Failed to save settings for {}: {}", idf_version, e);
        return Some(summary);
    }

    let output_path = archive_output_path(&idf_version);
    if !write_archive(archive_dir.path(), &output_path, &idf_version) {
        return Some(summary);
    }

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
    Some(summary)
}

async fn create_archives(args: Args) {
    let architecture = get_architecture();

    info!(
        "Creating installation data from configuration file: {:?}",
        args.create_from_config
    );
    let Some(mut settings) = args.create_from_config.as_deref().and_then(load_settings) else {
        return;
    };

    let global_python_version = args
        .python_version
        .unwrap_or_else(|| PYTHON_VERSION.to_string());
    info!("Using Python version: {}", global_python_version);

    let wheel_python_versions = resolve_wheel_python_versions(args.wheel_python_versions);

    info!(
        "Will download wheels for Python versions: {:?}",
        wheel_python_versions
    );

    let versions = idf_im_lib::idf_versions::get_idf_names(false).await;
    let version_list = select_idf_versions(
        args.idf_version_override,
        args.build_all_versions,
        settings.idf_versions.clone(),
        &versions,
    );

    settings.idf_versions = Some(version_list.clone());

    if !uv_is_installed() {
        return;
    }

    let Ok(shared_prereq_dir) = prepare_shared_prerequisites().await else {
        return;
    };

    let ctx = VersionBuildContext {
        settings: &settings,
        architecture: &architecture,
        shared_prereq_dir: shared_prereq_dir.as_ref(),
        global_python_version: &global_python_version,
        wheel_python_versions: &wheel_python_versions,
    };

    let mut build_summaries = Vec::new();
    for idf_version in version_list {
        if let Some(summary) = build_version_archive(&ctx, idf_version).await {
            build_summaries.push(summary);
        }
    }

    write_github_summary(&build_summaries);

    if !all_builds_successful(&build_summaries) {
        error!("Some builds failed or had warnings. Check the summary above.");
    } else {
        info!("🎉 All requested versions processed successfully.");
    }
}

fn extract_archive(archive_path: &Path) {
    info!(
        "Extracting installation data from archive: {:?}",
        archive_path
    );

    if !archive_path.exists() {
        error!("Archive file does not exist: {:?}", archive_path);
        return;
    }

    let extract_dir = extraction_dir(archive_path);

    match extract_zst_archive(archive_path, &extract_dir) {
        Ok(_) => {
            info!("Successfully extracted archive to: {:?}", extract_dir);
            info!("You can now examine the contents for debugging purposes.");
        }
        Err(err) => {
            error!("Failed to extract archive: {}", err);
        }
    }
}

async fn print_versions() {
    let versions = idf_im_lib::idf_versions::get_stable_idf_names().await;
    for version in versions {
        println!("{}", version);
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if select_mode(&args) == Mode::ListVersions {
        print_versions().await;
        return;
    }

    // Setup logging using fern
    if let Err(e) = setup_offline_installer(args.verbose, args.log_dir.clone()) {
        error!("Failed to initialize logging: {e}");
    }

    match select_mode(&args) {
        Mode::Create(_) => create_archives(args).await,
        Mode::Extract(archive_path) => extract_archive(archive_path),
        Mode::ListVersions | Mode::Missing => {
            error!("Please specify either -c to create an archive or -a to extract one.");
            error!("Use --help for more information.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn py(
        version: &str,
        success: bool,
        error: Option<&str>,
        built: &[&str],
    ) -> PythonVersionResult {
        PythonVersionResult {
            version: version.to_string(),
            success,
            error_message: error.map(str::to_string),
            source_built_packages: built.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn summary(created: bool, size: Option<u64>, pys: Vec<PythonVersionResult>) -> BuildSummary {
        let mut s = BuildSummary::new("v5.4".to_string(), "linux-x64".to_string());
        s.archive_created = created;
        s.archive_size = size;
        s.python_versions = pys;
        s
    }

    fn parse(args: &[&str]) -> Args {
        Args::try_parse_from(
            std::iter::once("offline_installer_builder").chain(args.iter().copied()),
        )
        .unwrap()
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn github_summary_success_with_source_builds() {
        let s = summary(
            true,
            Some(2 * 1_048_576 + 524_288),
            vec![
                py("3.11", true, None, &["foo", "bar"]),
                py("3.12", true, None, &[]),
            ],
        );
        assert_eq!(
            s.to_github_summary(),
            "✅ **v5.4** (`linux-x64`)\n- Python versions: 3.11, 3.12\n- Archive size: 2.50 MB\n\n**Packages built from source:**\n- Python 3.11: foo, bar\n\n"
        );
    }

    #[test]
    fn github_summary_success_without_size() {
        let s = summary(true, None, vec![py("3.11", true, None, &[])]);
        assert_eq!(
            s.to_github_summary(),
            "✅ **v5.4** (`linux-x64`)\n- Python versions: 3.11\n\n"
        );
    }

    #[test]
    fn github_summary_partial_failure() {
        let s = summary(
            true,
            Some(1_048_576),
            vec![
                py("3.11", true, None, &[]),
                py("3.12", false, Some("boom"), &[]),
                py("3.13", false, None, &[]),
            ],
        );
        assert_eq!(
            s.to_github_summary(),
            "⚠️ **v5.4** (`linux-x64`)\n\n**✅ Successful Python versions:** 3.11\n\n**❌ Failed Python versions:** 3.12, 3.13\n\n<details>\n<summary>Error Details</summary>\n\n**Python 3.12:**\n```\nboom\n```\n\n**Python 3.13:**\n```\nUnknown error\n```\n\n</details>\n\n**Archive size:** 1.00 MB\n\n"
        );
    }

    #[test]
    fn github_summary_archive_not_created() {
        let s = summary(false, None, vec![]);
        assert_eq!(
            s.to_github_summary(),
            "❌ **v5.4** (`linux-x64`)\n\n**Status:** Archive creation failed\n\n"
        );
    }

    #[test]
    fn github_summary_archive_created_without_python_results() {
        let s = summary(true, None, vec![]);
        assert_eq!(s.to_github_summary(), "❌ **v5.4** (`linux-x64`)\n\n\n");
    }

    #[test]
    fn github_summary_all_failed_with_archive_flag_off() {
        let s = summary(false, Some(10), vec![py("3.10", false, Some("x"), &[])]);
        assert_eq!(
            s.to_github_summary(),
            "❌ **v5.4** (`linux-x64`)\n\n**❌ Failed Python versions:** 3.10\n\n<details>\n<summary>Error Details</summary>\n\n**Python 3.10:**\n```\nx\n```\n\n</details>\n\n**Status:** Archive creation failed\n\n"
        );
    }

    #[test]
    fn summary_markdown_all_successful() {
        let summaries = vec![summary(true, None, vec![py("3.11", true, None, &[])])];
        assert_eq!(
            build_summary_markdown(&summaries),
            "# Offline Installer Build Summary\n\n## ✅ All builds successful\n\n✅ **v5.4** (`linux-x64`)\n- Python versions: 3.11\n\n"
        );
    }

    #[test]
    fn summary_markdown_heading_priorities() {
        let ok = summary(true, None, vec![py("3.11", true, None, &[])]);
        let warning = summary(
            true,
            None,
            vec![py("3.11", true, None, &[]), py("3.12", false, None, &[])],
        );
        let failure = summary(false, None, vec![]);
        let empty_but_created = summary(true, None, vec![]);

        assert_eq!(
            overall_status_heading(&[ok.clone(), warning.clone()]),
            Some("## ⚠️ Builds completed with warnings\n\n")
        );
        assert_eq!(
            overall_status_heading(&[warning, failure]),
            Some("## ❌ Build failures detected\n\n")
        );
        assert_eq!(
            overall_status_heading(&[]),
            Some("## ✅ All builds successful\n\n")
        );
        assert_eq!(
            overall_status_heading(&[ok, empty_but_created.clone()]),
            None
        );
        assert_eq!(
            build_summary_markdown(&[empty_but_created]),
            "# Offline Installer Build Summary\n\n❌ **v5.4** (`linux-x64`)\n\n\n"
        );
    }

    #[test]
    fn all_builds_successful_requires_archive_and_python() {
        assert!(all_builds_successful(&[]));
        assert!(all_builds_successful(&[summary(
            true,
            None,
            vec![py("3.11", true, None, &[])]
        )]));
        assert!(!all_builds_successful(&[summary(true, None, vec![])]));
        assert!(!all_builds_successful(&[summary(
            false,
            None,
            vec![py("3.11", true, None, &[])]
        )]));
    }

    #[test]
    fn format_megabytes_rounds_to_two_decimals() {
        assert_eq!(format_megabytes(0), "0.00 MB");
        assert_eq!(format_megabytes(1_572_864), "1.50 MB");
        assert_eq!(format_megabytes(1_000), "0.00 MB");
    }

    #[test]
    fn python_dir_names() {
        assert_eq!(python_env_dir_name("3.11"), "python_env_3_11");
        assert_eq!(wheel_dir_name("3.11"), "wheels_py311");
        assert_eq!(wheel_dir_name("3.10"), "wheels_py310");
    }

    #[test]
    fn venv_and_compote_executables_per_os() {
        let env = Path::new("env");
        assert_eq!(
            venv_python_executable(env, "windows"),
            Path::new("env").join("Scripts/python.exe")
        );
        assert_eq!(
            venv_python_executable(env, "linux"),
            Path::new("env").join("bin/python")
        );
        assert_eq!(
            compote_executable_path(env, "windows"),
            Path::new("env").join("Scripts").join("compote.exe")
        );
        assert_eq!(
            compote_executable_path(env, "macos"),
            Path::new("env").join("bin").join("compote")
        );
    }

    #[test]
    fn pip_download_args_binary_only() {
        assert_eq!(
            pip_download_args("req.txt", "c.txt", "wheels", true),
            vec![
                "-m",
                "pip",
                "download",
                "-r",
                "req.txt",
                "-c",
                "c.txt",
                "--dest",
                "wheels",
                "--only-binary=:all:",
                "--index-url",
                "https://dl.espressif.com/pypi/",
                "--extra-index-url",
                "https://pypi.org/simple",
            ]
        );
    }

    #[test]
    fn pip_download_args_with_source_builds() {
        assert_eq!(
            pip_download_args("req.txt", "c.txt", "wheels", false),
            vec![
                "-m",
                "pip",
                "download",
                "--verbose",
                "-r",
                "req.txt",
                "-c",
                "c.txt",
                "--dest",
                "wheels",
                "--index-url",
                "https://dl.espressif.com/pypi/",
                "--extra-index-url",
                "https://pypi.org/simple",
            ]
        );
    }

    #[test]
    fn parse_source_built_packages_dedupes_in_order() {
        let output = "Collecting foo\n  Building wheel for foo (pyproject.toml) ... done\nBuilding wheel for bar (setup.py)\nBuilding wheel for foo again\nBuilding wheel for\nBuilding wheel for \nnothing here";
        assert_eq!(
            parse_source_built_packages(output),
            strings(&["foo", "bar"])
        );
        assert!(parse_source_built_packages("").is_empty());
    }

    #[test]
    fn compote_registry_sync_args_layout() {
        assert_eq!(
            compote_registry_sync_args("/tmp/components"),
            vec![
                "registry",
                "sync",
                "--resolution",
                "latest",
                "--recursive",
                "/tmp/components"
            ]
        );
    }

    #[test]
    fn archive_output_path_uses_version() {
        assert_eq!(
            archive_output_path("v5.4.1"),
            PathBuf::from("archive_v5.4.1.zst")
        );
    }

    #[test]
    fn extraction_dir_next_to_archive() {
        assert_eq!(
            extraction_dir(Path::new("/data/archive_v5.4.zst")),
            PathBuf::from("/data/archive_v5.4_extracted")
        );
        assert_eq!(
            extraction_dir(Path::new("archive.zst")),
            PathBuf::from("archive_extracted")
        );
        assert_eq!(
            extraction_dir(Path::new("/")),
            PathBuf::from("./extracted_extracted")
        );
    }

    #[test]
    fn resolve_wheel_python_versions_defaults_to_supported() {
        assert_eq!(
            resolve_wheel_python_versions(None),
            strings(SUPPORTED_PYTHON_VERSIONS)
        );
        assert_eq!(
            resolve_wheel_python_versions(Some(strings(&["3.12"]))),
            strings(&["3.12"])
        );
    }

    #[test]
    fn select_idf_versions_precedence() {
        let available = strings(&["v5.4", "v5.3"]);
        assert_eq!(
            select_idf_versions(
                Some("v5.1".into()),
                true,
                Some(strings(&["v5.2"])),
                &available
            ),
            strings(&["v5.1"])
        );
        assert_eq!(
            select_idf_versions(None, true, Some(strings(&["v5.2"])), &available),
            available
        );
        assert_eq!(
            select_idf_versions(None, false, Some(strings(&["v5.2"])), &available),
            strings(&["v5.2"])
        );
        assert_eq!(
            select_idf_versions(None, false, None, &available),
            strings(&["v5.4"])
        );
    }

    #[test]
    fn windows_prerequisites_names() {
        let list = windows_prerequisite_downloads("https://example.com/git.tar.bz2".into());
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0],
            ("https://example.com/git.tar.bz2".to_string(), "git.tar.bz2")
        );
        assert_eq!(list[1].0, WINDOWS_PYTHON_URL);
        assert_eq!(list[1].1, "python.tar.gz");
    }

    #[test]
    fn version_settings_pins_single_version() {
        let tmp = TempDir::new().unwrap();
        let base = Settings {
            idf_versions: Some(strings(&["v5.4", "v5.3"])),
            idf_path: Some(PathBuf::from("/somewhere")),
            ..Default::default()
        };

        let pinned = version_settings(&base, "v5.3", tmp.path());
        assert_eq!(pinned.idf_versions, Some(strings(&["v5.3"])));
        assert_eq!(
            pinned.config_file_save_path,
            Some(tmp.path().join("config.toml"))
        );
        assert_eq!(pinned.idf_path, None);
        assert_eq!(base.idf_versions, Some(strings(&["v5.4", "v5.3"])));
    }

    #[test]
    fn python_version_result_failed_constructor() {
        let r = PythonVersionResult::failed("3.12", "oops".into(), strings(&["pkg"]));
        assert_eq!(r.version, "3.12");
        assert!(!r.success);
        assert_eq!(r.error_message.as_deref(), Some("oops"));
        assert_eq!(r.source_built_packages, strings(&["pkg"]));
    }

    #[test]
    fn args_defaults_and_flags() {
        let args = parse(&[]);
        assert_eq!(
            args.install_dir,
            Some(PathBuf::from("/tmp/eim_install_data"))
        );
        assert_eq!(args.python_version.as_deref(), Some(PYTHON_VERSION));
        assert_eq!(args.wheel_python_versions, None);
        assert!(!args.build_all_versions);
        assert_eq!(select_mode(&args), Mode::Missing);

        let args = parse(&[
            "-c",
            "default",
            "--wheel-python-versions",
            "3.10,3.12",
            "--idf-version-override",
            "v5.1.2",
            "--build-all-versions",
            "-vv",
            "-p",
            "3.12",
            "--log-dir",
            "/tmp/logs",
        ]);
        assert_eq!(args.wheel_python_versions, Some(strings(&["3.10", "3.12"])));
        assert_eq!(args.idf_version_override.as_deref(), Some("v5.1.2"));
        assert!(args.build_all_versions);
        assert_eq!(args.verbose, 2);
        assert_eq!(args.python_version.as_deref(), Some("3.12"));
        assert_eq!(args.log_dir, Some(PathBuf::from("/tmp/logs")));
        assert_eq!(select_mode(&args), Mode::Create("default"));
    }

    #[test]
    fn select_mode_precedence() {
        assert_eq!(
            select_mode(&parse(&["--list-versions", "-c", "default", "-a", "x.zst"])),
            Mode::ListVersions
        );
        assert_eq!(
            select_mode(&parse(&["-c", "cfg.toml", "-a", "x.zst"])),
            Mode::Create("cfg.toml")
        );
        assert_eq!(
            select_mode(&parse(&["--archive", "x.zst"])),
            Mode::Extract(Path::new("x.zst"))
        );
    }

    #[test]
    fn load_settings_default_and_missing_file() {
        assert!(load_settings("default").is_some());
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("missing.toml");
        assert!(load_settings(missing.to_str().unwrap()).is_none());
    }
}
