// Std lib imports
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::{self, Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

// Crate imports
use clap::Parser;
use fern::Dispatch;
use tempfile::TempDir;

// idf-im-lib imports
use idf_im_lib::command_executor::execute_command;
use idf_im_lib::{download_file, download_file_and_rename, ensure_path, get_log_directory};
use idf_im_lib::idf_tools::{get_list_of_tools_to_download, read_and_parse_tools_file, Download};
use idf_im_lib::idf_versions::{get_idf_names, get_stable_idf_names};
use idf_im_lib::logging;
use idf_im_lib::python_utils::download_constraints_file;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::{extract_zst_archive, parse_cmake_version};
use idf_im_lib::verify_file_checksum;
use idf_im_lib::git_tools::ProgressMessage;

// Log imports
use log::{debug, error, info, warn, LevelFilter};

// Other crates
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use tar::Builder as TarBuilder;
use zstd::encode_all;

// Constants
pub const PYTHON_VERSION: &str = "3.11";
pub const SUPPORTED_PYTHON_VERSIONS: &[&str] = &["3.10", "3.11", "3.12", "3.13"];

// ============== Data Structures ==============

/// Tracks all build information for summary generation
#[derive(Default, Debug)]
struct BuildSummary {
    version: String,
    platform: String,
    idf_downloaded: bool,
    idf_download_path: Option<PathBuf>,
    python_versions: Vec<PythonEnvSummary>,
    tools: Vec<ToolInfo>,
    components: Vec<String>,
    prerequisites: Vec<String>,
    requirements_files: Vec<String>,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Default, Debug, Clone)]
struct PythonEnvSummary {
    version: String,
    wheels_downloaded: bool,
    requirements_files_merged: Vec<String>,
    venv_path: Option<PathBuf>,
    wheel_dir: Option<PathBuf>,
}

#[derive(Default, Debug, Clone)]
struct ToolInfo {
    name: String,
    version: String,
    downloaded: bool,
    verified: bool,
}

// ============== Logging Setup ==============

pub fn setup_offline_installer(
    verbose: u8,
    custom_log_dir: Option<PathBuf>,
) -> Result<(), fern::InitError> {
    let log_level = match verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    let log_dir = custom_log_dir.or_else(get_log_directory).unwrap_or_else(|| PathBuf::from("."));
    let log_file_path = log_dir.join("offline_installer.log");

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory {}: {}", log_dir.display(), e);
    }

    Dispatch::new()
        .format(logging::formatter)
        .level(log_level)
        .chain(fern::log_file(&log_file_path)?)
        .chain(std::io::stdout())
        .apply()?;

    debug!("Offline installer logging initialized at level: {:?}", log_level);
    Ok(())
}

// ============== Progress Bar Helpers ==============

pub fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] ({eta})",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn FmtWrite| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        })
        .progress_chars("#>-"),
    );
    pb
}

pub fn update_progress_bar_number(pb: &ProgressBar, value: u64) {
    pb.set_position(value);
}

// ============== Summary Implementation ==============

impl BuildSummary {
    /// Print extended summary to console/file logs
    fn print_extended_summary(&self) {
        info!("=== EXTENDED BUILD SUMMARY ===");
        info!("IDF Version: {}", self.version);
        info!("Platform: {}", self.platform);
        info!(
            "IDF Download: {}",
            if self.idf_downloaded { "SUCCESS" } else { "FAILED" }
        );

        info!("\nPython Environments:");
        for py in &self.python_versions {
            info!(
                "  {}: {} wheels",
                py.version,
                if py.wheels_downloaded { "OK" } else { "FAILED" },
            );
        }

        info!("\nTools ({})", self.tools.len());
        for tool in &self.tools {
            let status = match (tool.downloaded, tool.verified) {
                (true, true) => "OK",
                (false, _) => "NOT DOWNLOADED",
                (_, false) => "CHECKSUM FAILED",
            };
            info!("  {} {} - {}", tool.name, tool.version, status);
        }

        if !self.components.is_empty() {
            info!("\nComponents ({})", self.components.len());
            info!("  {:?}", self.components);
        }

        info!("\n IDF Features:");
        for f in &self.requirements_files {
            info!("  {}", f.replace("requirements.", "").replace(".txt", ""));
        }

        if !self.prerequisites.is_empty() {
            info!("\nPrerequisites:");
            for p in &self.prerequisites {
                info!("  {}", p);
            }
        }

        if !self.warnings.is_empty() {
            warn!("\n=== WARNINGS ===");
            for w in &self.warnings {
                warn!("  {}", w);
            }
        }

        if !self.errors.is_empty() {
            error!("\n=== ERRORS ===");
            for e in &self.errors {
                error!("  {}", e);
            }
        }
    }

    /// Generate GitHub Actions STEP_SUMMARY compatible markdown
    fn write_workflow_summary(&self, path: &Path) -> Result<(), io::Error> {
        let mut content = String::new();

        // Header with status emoji
        let idf_status = if self.idf_downloaded { "✅" } else { "❌" };
        content.push_str(&format!(
            "# ESP-IDF {} Offline Installer Build Summary\n\n",
            self.version
        ));
        content.push_str("| Property | Value |\n");
        content.push_str("|----------|-------|\n");
        content.push_str(&format!(
            "| **IDF Version** | {} {} |\n",
            idf_status, self.version
        ));
        content.push_str(&format!("| **Platform** | {} |\n", self.platform));
        content.push_str(&format!(
            "| **Python Versions** | {} |\n",
            self.python_versions
                .iter()
                .map(|p| p.version.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));

        // Counts section
        content.push_str("\n## Counts\n\n");
        content.push_str("| Component | Count | Status |\n");
        content.push_str("|-----------|-------|--------|\n");
        let tools_ok = self.tools.iter().filter(|t| t.downloaded && t.verified).count();
        let tools_failed = self.tools.len() - tools_ok;
        content.push_str(&format!(
            "| Tools | {} | {} ✅ | {} ❌ |\n",
            self.tools.len(), tools_ok, tools_failed
        ));
        content.push_str(&format!(
            "| Components | {} | - |\n",
            self.components.len()
        ));
        content.push_str(&format!(
            "| Prerequisites | {} | - |\n",
            self.prerequisites.len()
        ));

        // Python environments
        if !self.python_versions.is_empty() {
            content.push_str("\n## Python Environments\n\n");
            content.push_str("| Version | Status |\n");
            content.push_str("|---------|--------|\n");
            for py in &self.python_versions {
                let status = if py.wheels_downloaded { "✅" } else { "❌" };
                content.push_str(&format!("| {} | {} |\n", py.version, status));
            }
        }

        // Failed tools
        let failed_tools: Vec<_> = self
            .tools
            .iter()
            .filter(|t| !t.downloaded || !t.verified)
            .collect();
        if !failed_tools.is_empty() {
            content.push_str("\n## Failed Tools ⚠️\n\n");
            for tool in &failed_tools {
                content.push_str(&format!(
                    "- ⚠️ {} {} ({}verified)\n",
                    tool.name,
                    tool.version,
                    if tool.verified { "" } else { "not " }
                ));
            }
        }

        // Warnings
        if !self.warnings.is_empty() {
            content.push_str("\n## Warnings\n\n");
            for w in &self.warnings {
                content.push_str(&format!("- ⚠️ {}\n", w));
            }
        }

        // Errors
        if !self.errors.is_empty() {
            content.push_str("\n## Errors ❌\n\n");
            for e in &self.errors {
                content.push_str(&format!("- ❌ {}\n", e));
            }
        }

        // Final status
        content.push_str("\n---\n");
        let overall = if self.errors.is_empty() {
            "✅ SUCCESS"
        } else {
            "❌ FAILED"
        };
        content.push_str(&format!("**Build Status:** {}\n", overall));

        fs::write(path, &content)?;
        info!("Workflow summary written to: {:?}", path);
        Ok(())
    }
}

// ============== Helper Functions ==============

/// Finds all 'requirements.*' files in a given directory,
/// merges their content, and writes it to 'requirements.merged.txt'.
pub fn merge_requirements_files(
    folder_path: &Path,
) -> Result<Vec<String>, io::Error> {
    let mut merged_content = String::new();
    let mut requirements_found = false;
    let mut found_files = Vec::new();

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
                if file_name.starts_with("requirements.") {
                    requirements_found = true;
                    found_files.push(file_name.to_string());
                    debug!("Merging file: {}", path.display());
                    let mut file = fs::File::open(&path)?;
                    file.read_to_string(&mut merged_content)?;
                    if !merged_content.ends_with('\n') && !merged_content.is_empty() {
                        merged_content.push('\n');
                    }
                }
            }
        }
    }

    if !requirements_found {
        debug!(
            "No 'requirements.*' files found in {}",
            folder_path.display()
        );
        return Ok(found_files);
    }

    let output_file_path = folder_path.join("requirements.merged.txt");
    let mut output_file = fs::File::create(&output_file_path)?;
    output_file.write_all(merged_content.as_bytes())?;

    debug!(
        "Successfully merged requirements files to: {}",
        output_file_path.display()
    );

    Ok(found_files)
}

/// Download wheels for a single Python version
async fn download_wheels_for_python_version(
    archive_dir: &Path,
    requirements_path: &Path,
    constraint_file: &Path,
    python_version: &str,
    summary: &mut PythonEnvSummary,
) -> Result<(), String> {
    info!("Processing Python version: {}", python_version);

    // Create version-specific directories
    let python_env =
        archive_dir.join(format!("python_env_{}", python_version.replace('.', "_")));
    let wheel_dir = archive_dir.join(format!("wheels_py{}", python_version.replace('.', "")));

    summary.venv_path = Some(python_env.clone());
    summary.wheel_dir = Some(wheel_dir.clone());

    // Ensure directories exist
    ensure_path(python_env.to_str().unwrap())
        .map_err(|e| format!("Failed to create Python env directory for {}: {}", python_version, e))?;
    ensure_path(wheel_dir.to_str().unwrap())
        .map_err(|e| format!("Failed to create wheel directory for {}: {}", python_version, e))?;

    // Create virtual environment
    info!("Creating virtual environment for Python {}...", python_version);
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
            if output.status.success() {
                info!(
                    "Python {} virtual environment created successfully.",
                    python_version
                );
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                error!("Failed to create Python {} venv: {}", python_version, err);
                return Err(format!("Failed to create venv: {}", err));
            }
        }
        Err(err) => {
            error!("Failed to create venv for Python {}: {}", python_version, err);
            return Err(format!("Failed to create venv: {}", err));
        }
    }

    // Determine Python executable
    let python_executable = match std::env::consts::OS {
        "windows" => python_env.join("Scripts/python.exe"),
        _ => python_env.join("bin/python"),
    };

    // Ensure pip is available
    info!("Ensuring pip is available for Python {}...", python_version);
    match execute_command(
        python_executable.to_str().unwrap(),
        &["-m", "ensurepip", "--upgrade"],
    ) {
        Ok(output) => {
            if output.status.success() {
                info!("Successfully ensured pip for Python {}.", python_version);
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                warn!(
                    "Failed to upgrade pip for Python {}: {}",
                    python_version, err
                );
                // Try installing pip via get-pip.py as fallback
                info!("Attempting fallback pip installation for Python {}...", python_version);
                let get_pip_url = "https://bootstrap.pypa.io/get-pip.py";
                let get_pip_path = python_env.join("get-pip.py");
                if let Err(e) = download_file(get_pip_url, get_pip_path.to_str().unwrap(), None).await {
                    return Err(format!("Failed to download get-pip.py for {}: {}", python_version, e));
                }
                let output = execute_command(
                    python_executable.to_str().unwrap(),
                    &[get_pip_path.to_str().unwrap(), "--force-reinstall"],
                );
                match output {
                    Ok(out) => {
                        if !out.status.success() {
                            let err = String::from_utf8_lossy(&out.stderr);
                            return Err(format!("Failed to install pip for {}: {}", python_version, err));
                        }
                    }
                    Err(e) => return Err(format!("Failed to run get-pip.py for {}: {}", python_version, e)),
                }
            }
        }
        Err(err) => {
            error!("Failed to ensure pip for Python {}: {}", python_version, err);
            return Err(format!("Failed to ensure pip: {}", err));
        }
    }

    // Download wheels
    info!("Downloading wheels for Python {}...", python_version);
    match execute_command(
        python_executable.to_str().unwrap(),
        &[
            "-m",
            "pip",
            "download",
            "-r",
            requirements_path.to_str().unwrap(),
            "-c",
            constraint_file.to_str().unwrap(),
            "--dest",
            wheel_dir.to_str().unwrap(),
        ],
    ) {
        Ok(output) => {
            if output.status.success() {
                info!("Python {} packages downloaded successfully.", python_version);
                summary.wheels_downloaded = true;
                Ok(())
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                error!(
                    "Failed to download Python {} packages: {}",
                    python_version, err
                );
                Err(format!("Failed to download wheels: {}", err))
            }
        }
        Err(err) => {
            error!("Failed to download packages for Python {}: {}", python_version, err);
            Err(format!("Failed to download wheels: {}", err))
        }
    }
}

/// Download wheels for multiple Python versions
async fn download_wheels_for_python_versions(
    archive_dir: &Path,
    requirements_path: &Path,
    constraint_file: &Path,
    python_versions: &[&str],
    summary: &mut BuildSummary,
) -> Result<(), String> {
    let mut successful_versions: Vec<String> = vec![];

    for python_version in python_versions {
        let mut py_summary = PythonEnvSummary {
            version: python_version.to_string(),
            ..Default::default()
        };

        match download_wheels_for_python_version(
            archive_dir,
            requirements_path,
            constraint_file,
            python_version,
            &mut py_summary,
        )
        .await
        {
            Ok(_) => {
                successful_versions.push(python_version.to_string());
            }
            Err(e) => {
                summary.warnings.push(format!(
                    "Python {} wheel download failed: {}",
                    python_version, e
                ));
            }
        }

        summary.python_versions.push(py_summary);
    }

    if successful_versions.is_empty() {
        Err("All Python versions failed to download wheels".to_string())
    } else {
        info!("Successfully processed {} Python version(s): {}",
              successful_versions.len(), successful_versions.join(", "));
        Ok(())
    }
}

/// Download a single tool with verification
async fn download_tool(
    tool_name: &str,
    version: &str,
    download_link: &Download,
    tool_path: &Path,
    summary: &mut BuildSummary,
) -> Result<(), String> {
    info!(
        "Preparing tool: {} version: {} from: {}",
        tool_name, version, download_link.url
    );

    match download_file(&download_link.url, tool_path.to_str().unwrap(), None).await {
        Ok(_) => {
            let filename = Path::new(&download_link.url)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            let full_file_path = tool_path.join(filename);

            let verified = verify_file_checksum(
                &download_link.sha256,
                full_file_path.to_str().unwrap(),
            )
            .unwrap_or(false);

            let tool_info = ToolInfo {
                name: tool_name.to_string(),
                version: version.to_string(),
                downloaded: true,
                verified,
            };

            if verified {
                info!("Tool {} version {} downloaded and verified.", tool_name, version);
            } else {
                error!("Checksum failed for tool {} version {}.", tool_name, version);
                summary.warnings.push(format!(
                    "Checksum failed for tool {} {}",
                    tool_name, version
                ));
            }

            summary.tools.push(tool_info);
            Ok(())
        }
        Err(err) => {
            error!("Failed to download tool {}: {}", tool_name, err);
            summary.tools.push(ToolInfo {
                name: tool_name.to_string(),
                version: version.to_string(),
                downloaded: false,
                verified: false,
            });
            Err(format!("Failed to download {}: {}", tool_name, err))
        }
    }
}

/// Download all tools for the build
async fn download_tools(
    tools_json_file: &Path,
    targets: &[String],
    mirror: Option<&str>,
    tool_path: &Path,
    summary: &mut BuildSummary,
) -> Result<(), String> {
    let tools = match read_and_parse_tools_file(tools_json_file.to_str().unwrap()) {
        Ok(t) => t,
        Err(err) => {
            error!("Failed to read tools json file: {}", err);
            return Err(format!("Failed to read tools.json: {}", err));
        }
    };

    let download_links = get_list_of_tools_to_download(tools, targets.to_vec(), mirror);

    ensure_path(tool_path.to_str().unwrap())
        .map_err(|e| format!("Failed to create tools path: {}", e))?;

    let mut all_success = true;
    for (tool_name, (version, download_link)) in download_links.iter() {
        if let Err(e) = download_tool(tool_name, version, download_link, tool_path, summary).await {
            summary.warnings.push(format!("Tool {} download error: {}", tool_name, e));
            all_success = false;
        }
    }

    if !all_success {
        Err("Some tools failed to download".to_string())
    } else {
        Ok(())
    }
}

/// Create compressed archive from directory
fn create_archive(source_dir: &Path, version: &str) -> Result<PathBuf, String> {
    let output_path = PathBuf::from(format!("archive_{}.zst", version));

    let mut output_file = File::create(&output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;

    let mut tar = TarBuilder::new(Vec::new());
    tar.append_dir_all(".", source_dir)
        .map_err(|e| format!("Failed to create tar: {}", e))?;

    let tar_data = tar
        .into_inner()
        .map_err(|e| format!("Failed to finalize tar: {}", e))?;

    let compressed_data = encode_all(&tar_data[..], 3)
        .map_err(|e| format!("Failed to compress: {}", e))?;

    output_file
        .write_all(&compressed_data)
        .map_err(|e| format!("Failed to write archive: {}", e))?;

    Ok(output_path)
}

/// Download Windows prerequisites using Scoop
async fn download_windows_prerequisites(
    shared_prereq_dir: &TempDir,
) -> Result<(), String> {
    let scoop_path = shared_prereq_dir.path().join("scoop");
    ensure_path(scoop_path.to_str().unwrap())
        .map_err(|e| format!("Failed to create scoop dir: {}", e))?;

    let scoop_list = vec![
        (
            "https://github.com/ScoopInstaller/Scoop/archive/master.zip",
            "scoop-master.zip",
        ),
        (
            "https://github.com/ScoopInstaller/Main/archive/master.zip",
            "main-master.zip",
        ),
        (
            "https://github.com/git-for-windows/git/releases/download/v2.50.1.windows.1/PortableGit-2.50.1-64-bit.7z.exe",
            "PortableGit-2.50.1-64-bit.7z.exe",
        ),
        (
            "https://www.python.org/ftp/python/3.11.9/python-3.11.9-amd64.exe",
            "python-3.11.9-amd64.exe",
        ),
        (
            "https://raw.githubusercontent.com/ScoopInstaller/Main/master/scripts/python/install-pep-514.reg",
            "install-pep-514.reg",
        ),
        (
            "https://raw.githubusercontent.com/ScoopInstaller/Main/master/scripts/python/uninstall-pep-514.reg",
            "uninstall-pep-514.reg",
        ),
        ("https://www.7-zip.org/a/7z2501-x64.msi", "7z2501-x64.msi"),
        (
            "https://raw.githubusercontent.com/ScoopInstaller/Binary/master/dark/dark-3.14.1.zip",
            "dark-3.14.1.zip",
        ),
    ];

    for (link, name) in &scoop_list {
        info!("Downloading Scoop prereq: {} as {}", link, name);
        match download_file_and_rename(link, scoop_path.to_str().unwrap(), None, Some(name)).await {
            Ok(_) => {
                info!("Downloaded: {}", name);
            }
            Err(err) => {
                error!("Failed to download {}: {}", name, err);
                return Err(format!("Failed to download {}: {}", name, err));
            }
        }
    }

    // Write install script
    let scoop_install_script = include_str!("../powershell_scripts/install_scoop_offline.ps1");
    fs::write(
        scoop_path.join("install_scoop_offline.ps1"),
        scoop_install_script,
    )
    .map_err(|e| format!("Failed to write install script: {}", e))?;

    info!(
        "Shared Windows prerequisites downloaded to: {:?}",
        shared_prereq_dir.path()
    );

    Ok(())
}

/// Process a single IDF version and create its archive
async fn process_idf_version(
    idf_version: &str,
    settings: &Settings,
    shared_prereq_dir: Option<&TempDir>,
    wheel_python_versions: &[String],
    summary: &mut BuildSummary,
) -> Result<PathBuf, String> {
    summary.version = idf_version.to_string();
    summary.platform = format!(
        "{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    info!("=== Processing ESP-IDF version: {} ===", idf_version);

    let archive_dir = TempDir::new().expect("Failed to create version-specific temp dir");
    let version_path = archive_dir.path().join(idf_version);
    ensure_path(version_path.to_str().unwrap()).expect("Failed to ensure version path");

    // Copy shared prerequisites if on Windows
    if let Some(shared_dir) = shared_prereq_dir {
        let dest_scoop = archive_dir.path().join("scoop");
        info!(
            "Copying shared prerequisites to: {:?}",
            dest_scoop
        );
        fs_extra::dir::copy(
            shared_dir.path().join("scoop"),
            &dest_scoop,
            &fs_extra::dir::CopyOptions::new().overwrite(true).copy_inside(true),
        )
        .expect("Failed to copy shared prerequisites");
        summary.prerequisites =
            vec!["scoop".to_string(), "git".to_string(), "python".to_string(), "7zip".to_string()];
    }

    // Download IDF
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
                    println!("submodule: {}", name);
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
        idf_version,
        settings.idf_mirror.as_deref(),
        true,
        tx,
    ) {
        Ok(_) => {
            info!(
                "ESP-IDF version {} downloaded successfully.",
                idf_version
            );
            summary.idf_downloaded = true;
            summary.idf_download_path = Some(idf_path.clone());
        }
        Err(err) => {
            error!(
                "Failed to download ESP-IDF version {}: {}",
                idf_version, err
            );
            summary.errors.push(format!("IDF download failed: {}", err));
            return Err(format!("IDF download failed: {}", err));
        }
    }
    handle.join().unwrap();

    // Read tools.json and download tools
    let tools_json_file = idf_path.join(
        settings
            .tools_json_file
            .clone()
            .unwrap_or_else(|| Settings::default().tools_json_file.unwrap()),
    );

    let tool_path = archive_dir.path().join("dist");
    if let Err(e) = download_tools(
        &tools_json_file,
        &settings.target.clone().unwrap_or(vec!["all".to_string()]),
        settings.mirror.as_deref(),
        &tool_path,
        summary,
    )
    .await
    {
        summary.errors.push(e.clone());
        return Err(e);
    }

    // Download constraints file
    let constrains_idf_version = match parse_cmake_version(idf_path.to_str().unwrap()) {
        Ok((maj, min)) => format!("v{}.{}", maj, min),
        Err(e) => {
            warn!("Failed to parse CMake version: {}", e);
            summary.warnings.push(format!("Failed to parse CMake version: {}", e));
            idf_version.to_string()
        }
    };
    info!(
        "Using constraints IDF version: {} from CMake",
        constrains_idf_version
    );

    let constraint_file = match download_constraints_file(&archive_dir.path(), &constrains_idf_version).await {
        Ok(file) => {
            info!("Downloaded constraints: {}", file.display());
            file
        }
        Err(e) => {
            error!("Failed to download constraints for {}: {}", idf_version, e);
            summary.errors.push(format!("Constraints download failed: {}", e));
            return Err(format!("Constraints download failed: {}", e));
        }
    };

    // Merge requirements
    let requirements_dir = idf_path.join("tools").join("requirements");
    match merge_requirements_files(&requirements_dir) {
        Ok(files) => summary.requirements_files = files,
        Err(e) => {
            error!("Failed to merge requirements: {}", e);
            summary.warnings.push(format!("Requirements merge failed: {}", e));
        }
    };

    let requirements_file = requirements_dir.join("requirements.merged.txt");

    // Download wheels for Python versions
    let wheel_versions: Vec<&str> = wheel_python_versions.iter().map(|s| s.as_str()).collect();
    if let Err(e) = download_wheels_for_python_versions(
        archive_dir.path(),
        &requirements_file,
        &constraint_file,
        &wheel_versions,
        summary,
    )
    .await
    {
        summary.warnings.push(format!("Wheel download: {}", e));
    }

    // Save settings
    let mut version_settings = settings.clone();
    version_settings.idf_versions = Some(vec![idf_version.to_string()]);
    version_settings.config_file_save_path = Some(archive_dir.path().join("config.toml"));
    version_settings.idf_path = None;

    if let Err(e) = version_settings.save() {
        error!("Failed to save settings for {}: {}", idf_version, e);
        summary.warnings.push(format!("Settings save failed: {}", e));
    }

    // Create archive
    let archive_path = create_archive(archive_dir.path(), idf_version)?;
    info!(
        "✅ Archive for {} saved to: {:?}",
        idf_version, archive_path
    );

    Ok(archive_path)
}

// ============== CLI Args ==============

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

    /// Number of python version to use, default is 3.11
    #[arg(short = 'p', long, default_value = PYTHON_VERSION)]
    python_version: Option<String>,

    /// Python versions to download wheels for (comma-separated, e.g., "3.10,3.11,3.12")
    #[arg(long, value_delimiter = ',')]
    wheel_python_versions: Option<Vec<String>>,

    /// Override IDF version
    #[arg(long)]
    idf_version_override: Option<String>,

    /// Build separate archives for all supported IDF versions
    #[arg(long)]
    build_all_versions: bool,

    /// List all supported IDF versions in machine-readable format and exit
    #[arg(long)]
    list_versions: bool,

    /// Custom log directory
    #[arg(long, value_name = "DIR")]
    log_dir: Option<PathBuf>,
}

// ============== Main ==============

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // List versions and exit
    if args.list_versions {
        let versions = get_stable_idf_names().await;
        for version in versions {
            println!("{}", version);
        }
        return;
    }

    // Setup logging
    if let Err(e) = setup_offline_installer(args.verbose, args.log_dir.clone()) {
        error!("Failed to initialize logging: {e}");
    }

    if args.create_from_config.is_some() {
        info!(
            "Creating installation data from configuration file: {:?}",
            args.create_from_config
        );

        let mut settings = match args.create_from_config {
            Some(ref config_path) if config_path == "default" => {
                info!("Loading default settings");
                Settings::default()
            }
            Some(config_path) => {
                let mut settings = Settings::default();
                match settings.load(&config_path) {
                    Ok(_) => info!("Settings loaded from {}", config_path),
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

        // Determine Python versions for wheels
        let wheel_python_versions: Vec<String> = if let Some(versions) = args.wheel_python_versions {
            versions
        } else {
            match std::env::consts::OS {
                "windows" => vec![args.python_version.unwrap_or_else(|| PYTHON_VERSION.to_string())],
                _ => SUPPORTED_PYTHON_VERSIONS.iter().map(|s| s.to_string()).collect(),
            }
        };

        info!(
            "Will download wheels for Python versions: {:?}",
            wheel_python_versions
        );

        // Get version list
        let versions = get_idf_names(false).await;
        let version_list = if let Some(override_version) = args.idf_version_override {
            info!("Using IDF version override: {}", override_version);
            vec![override_version]
        } else if args.build_all_versions {
            info!(
                "Building separate archives for all supported versions: {:?}",
                versions
            );
            versions
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
                    info!("UV is installed");
                } else {
                    error!("UV is not installed or not found");
                    return;
                }
            }
            Err(err) => {
                error!(
                    "UV is not installed or not found: {}. Please install it and try again.",
                    err
                );
                return;
            }
        }

        // Download Windows prerequisites once
        let shared_prereq_dir: Option<TempDir> = if std::env::consts::OS == "windows" {
            match TempDir::new() {
                Ok(dir) => {
                    if let Err(e) = download_windows_prerequisites(&dir).await {
                        error!("Failed to download Windows prerequisites: {}", e);
                        return;
                    }
                    Some(dir)
                }
                Err(e) => {
                    error!("Failed to create temp dir: {}", e);
                    return;
                }
            }
        } else {
            None
        };

        // Process each version
        let mut failed_versions = Vec::new();
        let mut build_summaries = Vec::new();

        for idf_version in version_list {
            let mut summary = BuildSummary::default();

            match process_idf_version(
                &idf_version,
                &settings,
                shared_prereq_dir.as_ref(),
                &wheel_python_versions,
                &mut summary,
            )
            .await
            {
                Ok(_) => {
                    summary.print_extended_summary();
                    build_summaries.push(summary);
                }
                Err(e) => {
                    error!("Version {} failed: {}", idf_version, e);
                    failed_versions.push(idf_version.clone());
                    build_summaries.push(summary);
                }
            }
        }

        // Write summaries
        let summary_path = PathBuf::from("github_summary.out");
        let json_path = PathBuf::from("build_summary.json");

        // Aggregate final summary
        let mut final_summary = BuildSummary::default();
        for s in &build_summaries {
            if s.idf_downloaded {
                final_summary.version = s.version.clone();
                final_summary.idf_downloaded = true;
            }
            final_summary.tools.extend(s.tools.clone());
            final_summary.python_versions.extend(s.python_versions.clone());
            final_summary.warnings.extend(s.warnings.clone());
            final_summary.errors.extend(s.errors.clone());
        }
        final_summary.platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);

        // Write workflow summary
        if let Err(e) = final_summary.write_workflow_summary(&summary_path) {
            error!("Failed to write workflow summary: {}", e);
        }

        // Final status
        if failed_versions.is_empty() {
            info!("🎉 All requested versions processed successfully.");
            info!("📄 Summary written to: {:?}", summary_path);
            info!("📄 JSON summary written to: {:?}", json_path);
        } else {
            error!(
                "Some versions failed to process: {:?}",
                failed_versions
            );
            std::process::exit(1);
        }
    } else if let Some(archive_path) = args.archive {
        // Extract mode
        info!(
            "Extracting installation data from archive: {:?}",
            archive_path
        );

        if !archive_path.exists() {
            error!("Archive file does not exist: {:?}", archive_path);
            return;
        }

        let archive_stem = archive_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted");

        let extract_dir = archive_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{}_extracted", archive_stem));

        match extract_zst_archive(&archive_path, &extract_dir) {
            Ok(_) => {
                info!(
                    "Successfully extracted archive to: {:?}",
                    extract_dir
                );
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
