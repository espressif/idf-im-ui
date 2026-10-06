use crate::gui::{
    app_state::{self, update_settings},
    commands::idf_tools::setup_tools,
    ui::{
        emit_installation_event, emit_log_message, InstallationProgress, InstallationStage,
        MessageLevel,
    },
    utils::{
        compare_versions, format_bytes, get_mirror_to_use, get_offline_archive_cache_dir,
        is_stable_version, swap_windows_drive, MirrorType,
    },
};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};
use tauri::{AppHandle, Emitter, Manager};

use idf_im_lib::{
    git_tools::ProgressMessage,
    idf_config::{IdfConfig, InstallationStatus, IDF_CONFIG_FILE_NAME},
    version_manager::get_default_config_path,
};
use log::{debug, error, info, warn};

use crate::gui::{
    app_state::{get_locked_settings, get_settings_non_blocking, set_installation_status},
    ui::{send_message, ProgressBar},
};
use idf_im_lib::settings::Settings;

use super::{prerequisites, settings};

#[cfg(not(target_os = "windows"))]
mod batch;
mod checks;
#[cfg(any(target_os = "windows", test))]
mod cli_installer;
mod offline;
mod progress;
mod repair;
mod simple;

use progress::{emit_error, emit_progress};

/// Offline package as presented to the GUI (one build for the current platform).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OfflineArchive {
    pub version: String,
    pub platform: String,
    pub filename: String,
    pub size: u64,
    pub url: String,
}

const OFFLINE_MANIFEST_URL: &str = "https://dl.espressif.com/dl/eim/offline_archives.json";
const OFFLINE_ARCHIVE_BASE_URL: &str = "https://dl.espressif.com/dl/eim/";

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallationPlan {
    pub total_versions: usize,
    pub versions: Vec<String>,
    pub current_version_index: Option<usize>,
}

pub fn emit_installation_plan(app_handle: &AppHandle, plan: InstallationPlan) {
    let _ = app_handle.emit("installation-plan", plan);
}

// Checks if an installation is currently in progress
#[tauri::command]
pub fn is_installing(app_handle: AppHandle) -> bool {
    app_state::is_installation_in_progress(&app_handle)
}

/// Spawns a progress monitor thread for installation
pub fn spawn_progress_monitor(
    app_handle: AppHandle,
    version: String,
    rx: mpsc::Receiver<ProgressMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let progress = ProgressBar::new(
            app_handle.clone(),
            &format!(
                "{} {}",
                rust_i18n::t!("gui.installation.progress.installing_idf"),
                version
            ),
        );

        while let Ok(message) = rx.recv() {
            match message {
                ProgressMessage::Finish => {
                    progress.update(100, None);
                }
                ProgressMessage::Update(value) => {
                    progress.update(
                        value,
                        Some(&format!(
                            "{} {}...",
                            rust_i18n::t!("gui.installation.progress.downloading_idf"),
                            version
                        )),
                    );
                }
                ProgressMessage::SubmoduleUpdate((name, value)) => {
                    progress.update(
                        value,
                        Some(&format!(
                            "{} {}... {}%",
                            rust_i18n::t!("gui.installation.progress.submodules"),
                            name,
                            value
                        )),
                    );
                }
                ProgressMessage::SubmoduleFinish(_name) => {
                    progress.update(100, None);
                }
            }
        }
    })
}

/// Downloads the ESP-IDF for a specific version without the detailed progress reporting
/// sadly the new gix library used in idf_im_lib for git operations does not provide progress
/// updates when used in an async context like Tauri GUI app.
async fn download_idf(
    app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    idf_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, _rx) = mpsc::channel();
    idf_im_lib::ensure_path(idf_path.to_str().unwrap())?;

    emit_installation_event(
        app_handle,
        InstallationProgress {
            stage: InstallationStage::Download,
            percentage: 0,
            message: rust_i18n::t!("gui.installation.download_starting", version = version)
                .to_string(),
            detail: Some(rust_i18n::t!("gui.installation.download_preparing").to_string()),
            version: Some(version.to_string()),
        },
    );

    let is_simple_installation = app_state::is_simple_installation(app_handle);
    let mirror_to_use = get_mirror_to_use(
        app_handle,
        MirrorType::IDF,
        settings,
        is_simple_installation,
    )
    .await;

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!(
            "gui.installation.cloning_from_mirror",
            version = version,
            mirror = mirror_to_use.as_str()
        )
        .to_string(),
    );

    // Clone values needed for the blocking task
    let idf_path_str = idf_path.to_str().unwrap().to_string();
    let repo_stub = settings.repo_stub.clone();
    let version_owned = version.to_string();
    let recurse_submodules = settings.recurse_submodules.unwrap_or_default();

    let result = match std::thread::spawn(move || {
        idf_im_lib::git_tools::get_esp_idf(
            &idf_path_str,
            repo_stub.as_deref(),
            &version_owned,
            Some(&mirror_to_use),
            recurse_submodules,
            tx,
        )
    })
    .join()
    {
        Ok(res) => res,
        Err(e) => Err(
            rust_i18n::t!("gui.installation.thread_panic", error = format!("{:?}", e)).to_string(),
        ),
    };

    match result {
        Ok(_) => {
            emit_installation_event(
                app_handle,
                InstallationProgress {
                    stage: InstallationStage::Extract,
                    percentage: 70,
                    message: rust_i18n::t!("gui.installation.ready_for_tools", version = version)
                        .to_string(),
                    detail: Some(
                        rust_i18n::t!(
                            "gui.installation.location",
                            path = idf_path.display().to_string()
                        )
                        .to_string(),
                    ),
                    version: Some(version.to_string()),
                },
            );

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                rust_i18n::t!(
                    "gui.installation.downloaded_successfully",
                    version = version,
                    path = idf_path.display().to_string()
                )
                .to_string(),
            );
            Ok(())
        }
        Err(e) => {
            emit_installation_event(
                app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.installation.download_failed", version = version)
                        .to_string(),
                    detail: Some(e.to_string()),
                    version: Some(version.to_string()),
                },
            );
            Err(e.into())
        }
    }
}

/// Installs a single ESP-IDF version
pub async fn install_single_version(
    app_handle: AppHandle,
    settings: &Settings,
    version: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Installing IDF version: {}", version);

    let paths = settings.get_version_paths(&version).map_err(|err| {
        error!("Failed to get version paths: {}", err);
        err.to_string()
    })?;

    if paths.using_existing_idf {
        info!("Using existing IDF directory: {}", paths.idf_path.display());
        send_message(
            &app_handle,
            rust_i18n::t!(
                "gui.installation.using_existing",
                path = paths.idf_path.display().to_string()
            )
            .to_string(),
            "info".to_string(),
        );

        debug!("Using IDF version: {}", paths.actual_version);
    } else {
        download_idf(&app_handle, settings, &version, &paths.idf_path).await?;
    }

    let (export_paths, export_vars) = setup_tools(
        &app_handle,
        settings,
        &paths.idf_path,
        &paths.actual_version,
        None,
    )
    .await?;

    let skip_component_installation = settings.skip_components_download == Some(true);

    idf_im_lib::single_version_post_install(
        paths.activation_script_path.to_string_lossy().as_ref(),
        paths.idf_path.to_string_lossy().as_ref(),
        &paths.actual_version,
        paths.tool_install_directory.to_string_lossy().as_ref(),
        export_paths,
        paths.python_venv_path.to_str(),
        Some(export_vars), // env_vars
        paths.python_path.to_string_lossy().as_ref(),
        false, // create_cmd_bat
        skip_component_installation,
        true, // is_gui
    );

    Ok(())
}

/// The IDE JSON (`eim_idf.json`) path configured in `settings`, if any.
fn configured_idf_config_path(settings: &Settings) -> Option<PathBuf> {
    settings
        .esp_idf_json_path
        .as_ref()
        .map(|p| PathBuf::from(p).join(IDF_CONFIG_FILE_NAME))
}

fn idf_config_path(settings: &Settings) -> PathBuf {
    configured_idf_config_path(settings).unwrap_or_else(get_default_config_path)
}

/// Updates the status of the pending `eim_idf.json` entry created for `version`, if any.
fn mark_pending_status(
    settings: &Settings,
    config_path: &Path,
    version: &str,
    status: InstallationStatus,
) {
    if let Some(id) = settings
        .pending_installation_ids
        .as_ref()
        .and_then(|ids| ids.get(version))
    {
        let _ = IdfConfig::update_status_in_file(config_path, id, status);
    }
}

#[cfg(target_os = "windows")]
fn validate_subprocess_install_path(
    app_handle: &AppHandle,
    settings_clone: &Settings,
) -> Result<(), String> {
    if !crate::gui::utils::is_path_empty_or_nonexistent(
        settings_clone.path.clone().unwrap().to_str().unwrap(),
        &settings_clone.clone().idf_versions.unwrap(),
    ) {
        log::error!(
            "Installation path not available: {:?}",
            settings_clone.path.clone().unwrap()
        );

        emit_installation_event(
            app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.installation.path_not_available").to_string(),
                detail: Some(
                    rust_i18n::t!(
                        "gui.installation.path_detail",
                        path = format!("{:?}", settings_clone.path.clone().unwrap())
                    )
                    .to_string(),
                ),
                version: None,
            },
        );

        return Err(rust_i18n::t!("gui.installation.path_not_available").to_string());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn save_subprocess_config(app_handle: &AppHandle, settings_clone: &Settings) -> Result<(), String> {
    if let Err(e) = settings_clone.save() {
        log::error!("Failed to save temporary config: {}", e);

        emit_installation_event(
            app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.installation.config_save_failed").to_string(),
                detail: Some(e.to_string()),
                version: None,
            },
        );

        return Err(rust_i18n::t!("gui.installation.config_save_failed").to_string());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<crate::gui::app_state::AppState>();

    // Set installation flag
    if let Err(e) = set_installation_status(&app_handle, true) {
        return Err(e);
    }

    // Get the settings and save to a temporary config file
    let settings = get_locked_settings(&app_handle)?;
    let config_path =
        cli_installer::subprocess_config_path(&std::env::temp_dir(), std::process::id());
    let settings_clone = cli_installer::subprocess_settings(&settings, &config_path);

    validate_subprocess_install_path(&app_handle, &settings_clone)?;
    save_subprocess_config(&app_handle, &settings_clone)?;

    log::info!("Saved temporary config to {}", config_path.display());

    // Get current executable path
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    // Emit initial progress
    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Checking,
            percentage: 0,
            message: rust_i18n::t!("gui.installation.starting_process").to_string(),
            detail: Some(rust_i18n::t!("gui.installation.launching_subprocess").to_string()),
            version: None,
        },
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        rust_i18n::t!("gui.installation.starting_separate_process").to_string(),
    );

    let child = cli_installer::spawn_cli_installer(&app_handle, current_exe, &config_path)?;

    // Set up monitor thread to read output and send to frontend
    let monitor_handle = app_handle.clone();
    let versions = settings_clone.idf_versions.clone().unwrap_or_default();

    emit_installation_plan(
        &app_handle,
        InstallationPlan {
            total_versions: versions.len(),
            versions: versions.clone(),
            current_version_index: None,
        },
    );

    std::thread::spawn(move || {
        cli_installer::monitor_cli_installer(monitor_handle, child, versions, config_path)
    });

    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
    info!("Starting installation");
    let _app_state = app_handle.state::<crate::gui::app_state::AppState>();
    // Set installation flag
    set_installation_status(&app_handle, true)?;

    let mut settings = get_locked_settings(&app_handle)?;

    let versions = batch::selected_versions(&app_handle, &settings)?;
    batch::announce_batch(&app_handle, &versions);

    // Create pending entries in eim_idf.json before installation starts
    if let Err(e) = settings.create_pending_esp_ide_json() {
        warn!("Failed to create pending installation entries: {}", e);
    }

    for index in 0..versions.len() {
        batch::install_batch_version(&app_handle, &settings, &versions, index).await?;
    }

    batch::save_ide_config(&app_handle, &settings);
    batch::finish_batch(&app_handle, &versions).await;

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

/// Starts a simple setup process that automates the installation
#[tauri::command]
pub async fn start_simple_setup(app_handle: tauri::AppHandle) -> Result<(), String> {
    let _app_state = app_handle.state::<crate::gui::app_state::AppState>();
    app_state::set_is_simple_installation(&app_handle, true)?;
    println!("Starting simple setup");
    let settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(e) => {
            emit_log_message(&app_handle, MessageLevel::Error, e.clone());
            return Err(e);
        }
    };

    emit_progress(
        &app_handle,
        InstallationStage::Checking,
        0,
        rust_i18n::t!("gui.simple_setup.starting").to_string(),
        None,
        None,
    );

    let os = std::env::consts::OS.to_lowercase();
    simple::ensure_prerequisites(&app_handle, &os)?;
    simple::ensure_python(&app_handle, &os)?;

    if settings.idf_versions.is_none() {
        simple::select_default_version(&app_handle).await?;
    }

    simple::add_ide_feature(&app_handle)?;

    // Start installation
    emit_progress(
        &app_handle,
        InstallationStage::Download,
        35,
        rust_i18n::t!("gui.simple_setup.starting_installation").to_string(),
        None,
        settings
            .idf_versions
            .as_ref()
            .and_then(|v| v.first().cloned()),
    );

    let res = start_installation(app_handle.clone()).await;
    app_state::set_is_simple_installation(&app_handle, false)?;
    res
}

/// Merges user-picked extra tools/features (from the "add more tools/features" pickers)
/// into whatever this version was already configured with, keyed the same way
/// `setup_tools` looks them up (by the installation's current name). The "always"
/// required tools/features are re-derived regardless of what's in these lists, so only
/// the newly requested ones need adding here.
fn merge_extra_selection(
    settings: &mut idf_im_lib::settings::Settings,
    version_key: &str,
    extra_tools: &Option<Vec<String>>,
    extra_features: &Option<Vec<String>>,
) {
    if let Some(extra) = extra_tools.as_ref().filter(|t| !t.is_empty()) {
        let mut per_version = settings.idf_tools_per_version.clone().unwrap_or_default();
        let mut tools_for_version = per_version
            .get(version_key)
            .cloned()
            .unwrap_or_else(|| settings.idf_tools.clone().unwrap_or_default());
        for tool in extra {
            if !tools_for_version.contains(tool) {
                tools_for_version.push(tool.clone());
            }
        }
        per_version.insert(version_key.to_string(), tools_for_version);
        settings.idf_tools_per_version = Some(per_version);
    }

    if let Some(extra) = extra_features.as_ref().filter(|f| !f.is_empty()) {
        let mut per_version = settings
            .idf_features_per_version
            .clone()
            .unwrap_or_default();
        let mut features_for_version = per_version
            .get(version_key)
            .cloned()
            .unwrap_or_else(|| settings.idf_features.clone().unwrap_or_default());
        for feature in extra {
            if !features_for_version.contains(feature) {
                features_for_version.push(feature.clone());
            }
        }
        per_version.insert(version_key.to_string(), features_for_version);
        settings.idf_features_per_version = Some(per_version);
    }
}

#[tauri::command]
pub async fn fix_installation(
    app_handle: AppHandle,
    id: String,
    extra_tools: Option<Vec<String>>,
    extra_features: Option<Vec<String>>,
) -> Result<(), String> {
    debug!(
        "Fixing installation with id {}, extra_tools: {:?}, extra_features: {:?}",
        id, extra_tools, extra_features
    );

    // Set installation flag to indicate installation is running
    set_installation_status(&app_handle, true)?;

    emit_progress(
        &app_handle,
        InstallationStage::Checking,
        0,
        rust_i18n::t!("gui.fix.checking_installation").to_string(),
        Some(rust_i18n::t!("gui.fix.looking_up", id = id.clone()).to_string()),
        None,
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        rust_i18n::t!("gui.fix.starting_repair", id = id.clone()).to_string(),
    );

    let fix_config_path = {
        let settings = app_state::get_locked_settings(&app_handle)
            .map_err(|e| format!("Failed to get settings: {}", e))?;
        configured_idf_config_path(&settings)
    };

    let installation = repair::find_installation(&app_handle, &id)?;
    let mut settings =
        repair::prepare_fix_settings(&app_handle, &installation, fix_config_path.as_ref()).await?;

    let extras = repair::ExtraSelection {
        tools: extra_tools,
        features: extra_features,
    };
    extras.apply(&mut settings, &installation.name);

    let config_path = fix_config_path
        .clone()
        .unwrap_or_else(get_default_config_path);

    // The actual repair process - this will generate detailed progress events
    repair::run_repair(&app_handle, &settings, &installation, &config_path).await?;
    info!("Successfully fixed installation {}", id);

    repair::save_repaired_config(
        &app_handle,
        &installation,
        &settings,
        fix_config_path.as_ref(),
        &config_path,
        &extras,
    )
    .await?;

    repair::emit_repair_completed(&app_handle, &installation);

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

/// Lightweight projection of an incomplete IdfInstallation for the frontend modal.
/// Only the fields the UI actually needs — avoids shipping the base64 installation_config
/// payload (potentially MBs of binary data) across the IPC boundary on every app start.
#[derive(serde::Serialize, Clone)]
pub struct IncompleteInstallationDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub status: idf_im_lib::idf_config::InstallationStatus,
}

#[tauri::command]
pub fn check_incomplete_installations(app_handle: AppHandle) -> Vec<IncompleteInstallationDto> {
    let path = match app_state::get_settings_non_blocking(&app_handle) {
        Ok(settings) => idf_config_path(&settings),
        Err(_) => return vec![],
    };

    match IdfConfig::from_file(&path) {
        Ok(config) => config
            .get_incomplete_installations()
            .into_iter()
            .map(|i| IncompleteInstallationDto {
                id: i.id.clone(),
                name: i.name.clone(),
                path: i.path.clone(),
                status: i.status.clone(),
            })
            .collect(),
        Err(_) => vec![],
    }
}

#[tauri::command]
pub async fn start_offline_installation(
    app_handle: AppHandle,
    archives: Vec<String>,
    install_path: String,
) -> Result<(), String> {
    // Set installation flag
    set_installation_status(&app_handle, true)?;

    // Validate archives
    if archives.is_empty() {
        emit_error(
            &app_handle,
            rust_i18n::t!("gui.offline.no_archives").to_string(),
            Some(rust_i18n::t!("gui.offline.select_archive").to_string()),
            None,
        );
        set_installation_status(&app_handle, false)?;
        return Err(rust_i18n::t!("gui.offline.no_archives").to_string());
    }

    emit_progress(
        &app_handle,
        InstallationStage::Checking,
        0,
        rust_i18n::t!("gui.offline.starting").to_string(),
        Some(rust_i18n::t!("gui.offline.processing_archives", count = archives.len()).to_string()),
        None,
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Info,
        rust_i18n::t!("gui.offline.starting_log", count = archives.len()).to_string(),
    );

    let total_archives = archives.len();

    for (archive_index, archive) in archives.iter().enumerate() {
        offline::ArchiveInstall {
            app_handle: &app_handle,
            archive,
            index: archive_index,
            total: total_archives,
        }
        .run(&install_path)
        .await?;
    }

    emit_progress(
        &app_handle,
        InstallationStage::Complete,
        100,
        rust_i18n::t!("gui.offline.completed").to_string(),
        Some(rust_i18n::t!("gui.offline.processed_archives", count = total_archives).to_string()),
        None,
    );

    emit_log_message(
        &app_handle,
        MessageLevel::Success,
        rust_i18n::t!("gui.offline.all_completed").to_string(),
    );

    // Clear installation flag
    set_installation_status(&app_handle, false)?;

    Ok(())
}

async fn platform_archives() -> Result<Vec<OfflineArchive>, String> {
    // Reuse the app's runtime platform detection (consistent with online tool
    // selection). None => no offline build for this platform.
    let platform = match idf_im_lib::offline_installer::current_offline_platform() {
        Some(p) => p,
        None => return Ok(vec![]),
    };
    let entries =
        idf_im_lib::offline_installer::fetch_offline_archive_manifest(OFFLINE_MANIFEST_URL).await?;

    let mut archives: Vec<OfflineArchive> = entries
        .into_iter()
        .filter(|e| e.status == "success" && e.platform == platform)
        .map(|e| OfflineArchive {
            url: format!("{}{}", OFFLINE_ARCHIVE_BASE_URL, e.filename),
            version: e.version,
            platform: e.platform,
            filename: e.filename,
            size: e.size,
        })
        .collect();

    archives.sort_by(
        |a, b| match (is_stable_version(&a.version), is_stable_version(&b.version)) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => compare_versions(&b.version, &a.version),
        },
    );
    Ok(archives)
}

/// Lists the offline packages available for the running platform.
/// Empty => no offline build for this OS/arch (e.g. windows-aarch64); the GUI
/// should steer the user to expert install.
#[tauri::command]
pub async fn get_offline_archives(_app_handle: AppHandle) -> Result<Vec<OfflineArchive>, String> {
    platform_archives().await
}

/// Lists available drive roots on Windows (e.g. `["C:", "D:"]`).
/// Returns an empty list on non-Windows platforms (drive selection is hidden there).
#[tauri::command]
pub fn get_available_drives() -> Vec<String> {
    if std::env::consts::OS != "windows" {
        return vec![];
    }
    let mut drives = Vec::new();
    for letter in b'C'..=b'Z' {
        // A: and B: are floppy disks
        let root = format!("{}:\\", letter as char);
        if std::path::Path::new(&root).exists() {
            drives.push(format!("{}:", letter as char));
        }
    }
    drives
}

/// Snapshot of the install-location settings we temporarily rewrite for a
/// drive override, so they can be restored afterwards. NOTE: `esp_idf_json_path`
/// (the registry) and `activation_script_path_override` are intentionally NOT
/// here — they stay on the default drive so version management keeps working.
#[derive(Clone)]
struct OriginalInstallPaths {
    path: Option<PathBuf>,
    tool_install_folder_name: Option<String>,
    tool_download_folder_name: Option<String>,
}

/// Rewrites only the drive letter of `path` + the two tool-folder names, so the
/// IDF dir, version dir, tool download dir and tool install dir all land on the
/// chosen drive. Returns the previous values for later restore.
fn apply_drive_override(
    app_handle: &AppHandle,
    drive: &str,
) -> Result<OriginalInstallPaths, String> {
    let current = get_settings_non_blocking(app_handle)?;
    let original = OriginalInstallPaths {
        path: current.path.clone(),
        tool_install_folder_name: current.tool_install_folder_name.clone(),
        tool_download_folder_name: current.tool_download_folder_name.clone(),
    };

    let drive = drive.to_string();
    update_settings(app_handle, move |settings| {
        if let Some(p) = settings.path.as_ref().and_then(|p| p.to_str()) {
            settings.path = Some(PathBuf::from(swap_windows_drive(p, &drive)));
        }
        if let Some(v) = settings.tool_install_folder_name.clone() {
            settings.tool_install_folder_name = Some(swap_windows_drive(&v, &drive));
        }
        if let Some(v) = settings.tool_download_folder_name.clone() {
            settings.tool_download_folder_name = Some(swap_windows_drive(&v, &drive));
        }
    })?;

    Ok(original)
}

fn restore_install_paths(app_handle: &AppHandle, orig: OriginalInstallPaths) -> Result<(), String> {
    update_settings(app_handle, move |settings| {
        settings.path = orig.path.clone();
        settings.tool_install_folder_name = orig.tool_install_folder_name.clone();
        settings.tool_download_folder_name = orig.tool_download_folder_name.clone();
    })
}

/// Bridges the library's `DownloadProgress` (std mpsc) onto the
/// `installation-progress` channel as the Download stage (0-100%).
async fn download_archive_with_progress(
    app_handle: &AppHandle,
    archive: &OfflineArchive,
    dest_dir: &str,
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<idf_im_lib::DownloadProgress>();

    let handle = app_handle.clone();
    let version = archive.version.clone();
    let total_size = archive.size;

    let progress_thread = tokio::task::spawn_blocking(move || {
        let mut last_pct: i32 = -1;
        while let Ok(msg) = rx.recv() {
            match msg {
                idf_im_lib::DownloadProgress::Start(_) => {
                    emit_installation_event(
                        &handle,
                        InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: 0,
                            message: rust_i18n::t!("gui.simple_offline.downloading").to_string(),
                            detail: Some(
                                rust_i18n::t!(
                                    "gui.simple_offline.download_detail",
                                    size = format_bytes(total_size)
                                )
                                .to_string(),
                            ),
                            version: Some(version.clone()),
                        },
                    );
                }
                idf_im_lib::DownloadProgress::Progress(downloaded, total) => {
                    let total = if total > 0 { total } else { total_size.max(1) };
                    let pct = ((downloaded.min(total) * 100) / total) as i32;
                    if pct != last_pct {
                        last_pct = pct;
                        emit_installation_event(
                            &handle,
                            InstallationProgress {
                                stage: InstallationStage::Download,
                                percentage: pct.clamp(0, 100) as u32,
                                message: rust_i18n::t!("gui.simple_offline.downloading")
                                    .to_string(),
                                detail: Some(
                                    rust_i18n::t!(
                                        "gui.simple_offline.download_progress",
                                        downloaded = format_bytes(downloaded),
                                        total = format_bytes(total)
                                    )
                                    .to_string(),
                                ),
                                version: Some(version.clone()),
                            },
                        );
                    }
                }
                idf_im_lib::DownloadProgress::Indeterminate(downloaded) => {
                    emit_installation_event(
                        &handle,
                        InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: 0,
                            message: rust_i18n::t!("gui.simple_offline.downloading").to_string(),
                            detail: Some(format_bytes(downloaded)),
                            version: Some(version.clone()),
                        },
                    );
                }
                idf_im_lib::DownloadProgress::Complete => {
                    emit_installation_event(
                        &handle,
                        InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: 100,
                            message: rust_i18n::t!("gui.simple_offline.download_complete")
                                .to_string(),
                            detail: None,
                            version: Some(version.clone()),
                        },
                    );
                }
                idf_im_lib::DownloadProgress::Error(e) => {
                    emit_log_message(&handle, MessageLevel::Error, e);
                }
                _ => {}
            }
        }
    });

    let result = idf_im_lib::download_file_and_rename(
        &archive.url,
        dest_dir,
        Some(tx), // dropped inside the fn when it returns -> receiver thread ends
        Some(&archive.filename),
        3,
    )
    .await;

    let _ = progress_thread.await;

    result.map_err(|e| {
        let msg = rust_i18n::t!(
            "gui.simple_offline.download_failed_detail",
            error = e.to_string()
        )
        .to_string();
        emit_installation_event(
            app_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.simple_offline.download_failed").to_string(),
                detail: Some(msg.clone()),
                version: Some(archive.version.clone()),
            },
        );
        msg
    })
}

/// Simple installation via the offline package.
///
/// 1. Resolve the `.zst` for `version` on the current platform.
/// 2. Download it to the persistent cache (skipped if a complete copy with the
///    expected byte size is already present).
/// 3. Optionally (Windows only) relocate IDF + tools to another drive by
///    rewriting the drive letter of `path`/tool folders for the duration of the
///    install, restoring defaults afterwards (registry + activation scripts stay
///    on the default drive).
/// 4. Hand the archive to the existing, e2e-tested `start_offline_installation`.
///
/// Returns the on-disk archive path so the GUI can offer to keep or delete it.
#[tauri::command]
pub async fn start_simple_offline_setup(
    app_handle: AppHandle,
    version: String,
    drive: Option<String>,
) -> Result<String, String> {
    let platform = idf_im_lib::offline_installer::current_offline_platform()
        .unwrap_or_else(|| "unsupported".to_string());

    let archives: Vec<OfflineArchive> = platform_archives().await?;
    let archive = archives
        .into_iter()
        .find(|a| a.version == version)
        .ok_or_else(|| {
            rust_i18n::t!(
                "gui.simple_offline.no_archive_for_version",
                version = version.clone(),
                platform = platform.clone()
            )
            .to_string()
        })?;

    // Non-Windows: verify prerequisites + Python BEFORE downloading several GB.
    // (Windows installs these from the archive, so it must download first.)
    if std::env::consts::OS != "windows" {
        checks::precheck_posix_prerequisites(&app_handle)?;
    }

    let cache_dir = get_offline_archive_cache_dir()?;
    let archive_path = cache_dir.join(&archive.filename);
    let archive_path_str = archive_path.to_string_lossy().to_string();

    emit_installation_event(
        &app_handle,
        InstallationProgress {
            stage: InstallationStage::Download,
            percentage: 0,
            message: rust_i18n::t!(
                "gui.simple_offline.preparing_download",
                version = version.clone()
            )
            .to_string(),
            detail: Some(archive.filename.clone()),
            version: Some(version.clone()),
        },
    );

    // Reuse a previously downloaded archive when it is fully present.
    let already_present = std::fs::metadata(&archive_path)
        .map(|m| m.len() == archive.size)
        .unwrap_or(false);

    if already_present {
        emit_log_message(
            &app_handle,
            MessageLevel::Info,
            rust_i18n::t!(
                "gui.simple_offline.using_cached",
                path = archive_path_str.clone()
            )
            .to_string(),
        );
        emit_installation_event(
            &app_handle,
            InstallationProgress {
                stage: InstallationStage::Download,
                percentage: 100,
                message: rust_i18n::t!("gui.simple_offline.archive_ready").to_string(),
                detail: Some(archive_path_str.clone()),
                version: Some(version.clone()),
            },
        );
    } else {
        download_archive_with_progress(&app_handle, &archive, cache_dir.to_str().unwrap()).await?;
        if let Ok(m) = std::fs::metadata(&archive_path) {
            if m.len() != archive.size {
                return Err(format!(
                    "Downloaded archive size mismatch for {}: expected {}, got {}",
                    archive.filename,
                    archive.size,
                    m.len()
                ));
            }
        }
    }

    // Windows-only: relocate IDF + tools to a different drive for THIS install.
    let restore_after = if std::env::consts::OS == "windows" {
        match drive.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => {
                emit_log_message(
                    &app_handle,
                    MessageLevel::Info,
                    rust_i18n::t!("gui.simple_offline.using_drive", drive = d).to_string(),
                );
                Some(apply_drive_override(&app_handle, d)?)
            }
            None => None,
        }
    } else {
        None
    };

    // Hand off to the existing offline pipeline (single archive, default path
    // = current settings, which we may have just drive-swapped).
    let install_result = start_offline_installation(
        app_handle.clone(),
        vec![archive_path_str.clone()],
        String::new(),
    )
    .await;

    // Always restore the default install location after a drive override so the
    // next default install goes back to the standard drive.
    if let Some(orig) = restore_after {
        if let Err(e) = restore_install_paths(&app_handle, orig) {
            warn!(
                "Failed to restore default install paths after drive override: {}",
                e
            );
        }
    }

    install_result?;
    Ok(archive_path_str)
}

/// Deletes a downloaded offline archive (the post-install "delete" choice).
#[tauri::command]
pub fn delete_offline_archive(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| format!("Failed to delete archive: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use idf_im_lib::idf_config::IdfInstallation;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn installation(id: &str, status: InstallationStatus) -> IdfInstallation {
        IdfInstallation {
            activation_script: None,
            id: id.to_string(),
            idf_tools_path: "/e/tools".to_string(),
            name: "v5.3".to_string(),
            path: "/e/v5.3/esp-idf".to_string(),
            python: None,
            installation_config: None,
            status,
        }
    }

    fn write_config(path: &Path, installations: Vec<IdfInstallation>) {
        IdfConfig {
            git_path: "/usr/bin/git".to_string(),
            idf_installed: installations,
            idf_selected_id: String::new(),
            eim_path: None,
            version: None,
        }
        .to_file(path, true, false)
        .unwrap();
    }

    #[test]
    fn configured_idf_config_path_joins_file_name() {
        let settings = Settings {
            esp_idf_json_path: Some("/cfg".to_string()),
            ..Settings::default()
        };
        assert_eq!(
            configured_idf_config_path(&settings),
            Some(PathBuf::from("/cfg").join(IDF_CONFIG_FILE_NAME))
        );
        assert_eq!(
            idf_config_path(&settings),
            Path::new("/cfg").join(IDF_CONFIG_FILE_NAME)
        );
    }

    #[test]
    fn idf_config_path_falls_back_to_default() {
        let settings = Settings {
            esp_idf_json_path: None,
            ..Settings::default()
        };
        assert_eq!(configured_idf_config_path(&settings), None);
        assert_eq!(idf_config_path(&settings), get_default_config_path());
    }

    #[test]
    fn mark_pending_status_updates_entry_for_version() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(IDF_CONFIG_FILE_NAME);
        write_config(
            &path,
            vec![
                installation("abc", InstallationStatus::InProgress),
                installation("other", InstallationStatus::InProgress),
            ],
        );
        let settings = Settings {
            pending_installation_ids: Some(HashMap::from([(
                "v5.3".to_string(),
                "abc".to_string(),
            )])),
            ..Settings::default()
        };

        mark_pending_status(&settings, &path, "v5.3", InstallationStatus::Failed);

        let config = IdfConfig::from_file(&path).unwrap();
        let status = |id: &str| {
            config
                .idf_installed
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .status
                .clone()
        };
        assert_eq!(status("abc"), InstallationStatus::Failed);
        assert_eq!(status("other"), InstallationStatus::InProgress);
    }

    #[test]
    fn mark_pending_status_without_pending_entry_is_noop() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(IDF_CONFIG_FILE_NAME);
        let mut settings = Settings {
            pending_installation_ids: None,
            ..Settings::default()
        };
        mark_pending_status(&settings, &path, "v5.3", InstallationStatus::Broken);
        assert!(!path.exists());

        settings.pending_installation_ids =
            Some(HashMap::from([("v5.2".to_string(), "abc".to_string())]));
        mark_pending_status(&settings, &path, "v5.3", InstallationStatus::Broken);
        assert!(!path.exists());
    }
}
