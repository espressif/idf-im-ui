use clap::builder;
use clap::Parser;
use idf_im_lib::command_executor::execute_command;
use idf_im_lib::download_file;
use idf_im_lib::download_file_and_rename;
use idf_im_lib::ensure_path;
use idf_im_lib::idf_tools::get_list_of_tools_to_download;
use idf_im_lib::python_utils::download_constraints_file;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::extract_zst_archive;
use idf_im_lib::utils::parse_cmake_version;
use idf_im_lib::verify_file_checksum;
use idf_im_lib::ProgressMessage;
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
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use idf_im_lib::get_log_directory;

pub const PYTHON_VERSION: &str = "3.11";
pub const SUPPORTED_PYTHON_VERSIONS: &[&str] = &["3.10", "3.11", "3.12", "3.13"];

fn setup_logging(verbose: u8) -> anyhow::Result<()> {
    let log_file_name = get_log_directory()
        .map(|dir| dir.join("offline_installer.log"))
        .unwrap_or_else(|| {
            eprintln!("Failed to get log directory, using default offline_installer.log");
            std::path::PathBuf::from("offline_installer.log")
        });

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build(log_file_name)?;

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build();

    let console_log_level = match verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    let file_log_level = LevelFilter::Trace;

    let config = log4rs::Config::builder()
        .appender(
            Appender::builder()
                .build("file", Box::new(logfile)),
        )
        .appender(
            Appender::builder()
                .build("stdout", Box::new(stdout)),
        )
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Trace),
        )?;

    log4rs::init_config(config)?;

    log::debug!(
        "Logging initialized with console level: {:?}, file level: {:?}",
        console_log_level, file_log_level
    );
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

/// Finds all 'requirements.*' files in a given directory,
/// merges their content, and writes it to 'requirements.merged.txt'.
///
/// # Arguments
/// * `folder_path` - The path to the directory to search.
///
/// # Returns
/// `Result<(), io::Error>` - Ok(()) on success, or an io::Error on failure.
pub fn merge_requirements_files(folder_path: &Path) -> Result<(), io::Error> {
    let mut merged_content = String::new();
    let mut requirements_found = false;

    // Ensure the folder exists and is a directory
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
                    println!("Merging file: {}", path.display());
                    let mut file = fs::File::open(&path)?;
                    file.read_to_string(&mut merged_content)?;
                    // Add a newline to separate content from different files, if they don't end with one
                    if !merged_content.ends_with('\n') && !merged_content.is_empty() {
                        merged_content.push('\n');
                    }
                }
            }
        }
    }

    if !requirements_found {
        println!("No 'requirements.*' files found in {}", folder_path.display());
        return Ok(()); // Or return an error if you consider it an error
    }

    let output_file_path = folder_path.join("requirements.merged.txt");
    let mut output_file = fs::File::create(&output_file_path)?;
    output_file.write_all(merged_content.as_bytes())?;

    println!("Successfully merged requirements files to: {}", output_file_path.display());

    Ok(())
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
/// `Result<(), String>` - Ok(()) on success, error message on failure
async fn download_wheels_for_python_versions(
    archive_dir: &Path,
    requirements_path: &Path,
    constraint_file: &Path,
    python_versions: &[&str],
) -> Result<(), String> {
    info!("Downloading wheels for Python versions: {:?}", python_versions);

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
                    // Continue anyway, it might already be installed
                }
            }
            Err(err) => {
                error!("Failed to install Python {}: {}", python_version, err);
                return Err(format!("Failed to install Python {}: {}", python_version, err));
            }
        }
    }

    for python_version in python_versions {
        info!("Processing Python version: {}", python_version);

        // Create version-specific directories
        let python_env = archive_dir.join(format!("python_env_{}", python_version.replace('.', "_")));
        let wheel_dir = archive_dir.join(format!("wheels_py{}", python_version.replace('.', "")));

        // Ensure directories exist
        ensure_path(python_env.to_str().unwrap())
            .map_err(|e| format!("Failed to create Python env directory for {}: {}", python_version, e))?;
        ensure_path(wheel_dir.to_str().unwrap())
            .map_err(|e| format!("Failed to create wheel directory for {}: {}", python_version, e))?;

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
                if output.status.success() {
                    info!("Python {} virtual environment created successfully.", python_version);
                } else {
                    error!(
                        "Failed to create Python {} virtual environment: {}",
                        python_version,
                        String::from_utf8_lossy(&output.stderr)
                    );
                    continue; // Skip this version and continue with others
                }
            }
            Err(err) => {
                error!("Failed to create venv for Python {}: {}", python_version, err);
                continue; // Skip this version and continue with others
            }
        }

        // Determine Python executable path
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
                    warn!(
                        "Failed to upgrade pip for Python {}: {}",
                        python_version,
                        String::from_utf8_lossy(&output.stderr)
                    );
                    // Continue anyway, pip might already be available
                }
            }
            Err(err) => {
                warn!("Failed to ensure pip for Python {}: {}", python_version, err);
                // Continue anyway
            }
        }

        // Download wheels for this Python version
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
                } else {
                    error!(
                        "Failed to download Python {} packages: {}",
                        python_version,
                        String::from_utf8_lossy(&output.stderr)
                    );
                    // Continue with other versions even if one fails
                }
            }
            Err(err) => {
                error!("Failed to download packages for Python {}: {}", python_version, err);
                // Continue with other versions even if one fails
            }
        }
    }

    Ok(())
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

    // Setup logging
    if let Err(e) = setup_logging(args.verbose) {
        error!("Failed to initialize logging: {e}");
    }



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
        for idf_version in version_list {
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
                            println!("submodule: {}", name);
                            progress_bar = create_progress_bar();
                        }
                        Err(_) => break,
                    }
                }
            });

            let idf_path = version_path.join("esp-idf");
            match idf_im_lib::get_esp_idf(
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
                    continue; // Skip to next version
                }
            }
            handle.join().unwrap(); // Wait for progress bar thread to finish

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

            let constraint_file = match download_constraints_file(&archive_dir.path(), &constrains_idf_version).await {
                Ok(file) => {
                    info!("Downloaded constraints: {}", file.display());
                    file
                }
                Err(e) => {
                    error!("Failed to download constraints for {}: {}", idf_version, e);
                    continue;
                }
            };

            // Merge requirements
            let requirements_dir = idf_path.join("tools").join("requirements");
            if let Err(e) = merge_requirements_files(&requirements_dir) {
                error!("Failed to merge requirements: {}", e);
                continue;
            }

            let requirements_file = requirements_dir.join("requirements.merged.txt");

            let wheel_versions: Vec<&str> = wheel_python_versions.iter().map(|s| s.as_str()).collect();
            if let Err(e) = download_wheels_for_python_versions(
                archive_dir.path(),
                &requirements_file,
                &constraint_file,
                &wheel_versions,
            ).await {
                error!("Failed to download wheels for {}: {}", idf_version, e);
                continue;
            }

            // Save settings for this version
            let mut version_settings = settings.clone();
            version_settings.idf_versions = Some(vec![idf_version.clone()]);
            version_settings.config_file_save_path = Some(archive_dir.path().join("config.toml"));
            version_settings.idf_path = None;

            if let Err(e) = version_settings.save() {
                error!("Failed to save settings for {}: {}", idf_version, e);
                continue;
            }

            // CREATE INDIVIDUAL ARCHIVE PER VERSION
            let output_path = PathBuf::from(format!("archive_{}.zst", idf_version));
            let mut output_file = match File::create(&output_path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to create output file {}: {}", output_path.display(), e);
                    continue;
                }
            };

            // Tar + Zstd compress
            let mut tar = TarBuilder::new(Vec::new());
            if let Err(e) = tar.append_dir_all(".", archive_dir.path()) {
                error!("Failed to create tar for {}: {}", idf_version, e);
                continue;
            }

            let tar_data = match tar.into_inner() {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to finalize tar for {}: {}", idf_version, e);
                    continue;
                }
            };

            let compressed_data = match encode_all(&tar_data[..], 3) {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to compress with zstd for {}: {}", idf_version, e);
                    continue;
                }
            };

            if let Err(e) = output_file.write_all(&compressed_data) {
                error!("Failed to write compressed data for {}: {}", idf_version, e);
                continue;
            }

            info!("✅ Archive for {} saved to: {:?}", idf_version, output_path);
        }

        info!("🎉 All requested versions processed.");
        return; // Exit after creating archives
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
