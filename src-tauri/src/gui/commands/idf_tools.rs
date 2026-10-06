use super::settings::update_settings_and_notify;
use crate::gui::{
    app_state::get_settings_non_blocking,
    ui::{
        emit_installation_event, emit_log_message, send_message, InstallationProgress,
        InstallationStage, MessageLevel,
    },
    utils::{get_mirror_to_use, MirrorType},
};
use anyhow::{anyhow, Context, Result};

use idf_im_lib::{
    add_path_to_path, ensure_path,
    idf_features::{get_requirements_json_url, FeatureInfo, RequirementsMetadata},
    idf_tools::{self, Tool, ToolsFile},
    settings::Settings,
    tool_selection::{fetch_tools_file_async, get_tools_for_selection, VersionToolsInfo},
    DownloadProgress,
};
use log::{debug, error, info, warn};
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFeatureInfo {
    pub version: String,
    pub features: Vec<FeatureInfo>,
}

/// Represents the tool setup configuration
#[derive(Debug)]
struct ToolSetup {
    download_dir: String,
    install_dir: String,
    tools_json_path: String,
}

impl ToolSetup {
    /// Creates a new tool setup based on settings and version path
    fn new(settings: &Settings, version_path: &Path) -> Result<Self, String> {
        let p = version_path;
        let tools_json_path = p
            .join("esp-idf")
            .join(settings.tools_json_file.clone().unwrap_or_default());
        let download_dir = p.join(
            settings
                .tool_download_folder_name
                .clone()
                .unwrap_or_default(),
        );
        let install_dir = p.join(
            settings
                .tool_install_folder_name
                .clone()
                .unwrap_or_default(),
        );
        Ok(Self {
            download_dir: download_dir.to_str().unwrap().to_string(),
            install_dir: install_dir.to_str().unwrap().to_string(),
            tools_json_path: tools_json_path.to_str().unwrap().to_string(),
        })
    }

    /// Creates necessary directories for tool installation
    fn create_directories(&self, app_handle: &AppHandle) -> Result<(), String> {
        // Create download directory
        ensure_path(&self.download_dir).map_err(|e| {
            send_message(
                app_handle,
                t!("gui.setup_tools.dir_create_failed", error = e.to_string()).to_string(),
                "error".to_string(),
            );
            e.to_string()
        })?;

        // Create installation directory
        ensure_path(&self.install_dir).map_err(|e| {
            send_message(
                app_handle,
                t!(
                    "gui.setup_tools.install_dir_create_failed",
                    error = e.to_string()
                )
                .to_string(),
                "error".to_string(),
            );
            e.to_string()
        })?;

        // Add installation directory to PATH
        add_path_to_path(&self.install_dir);

        Ok(())
    }

    /// Validates that the tools.json file exists
    fn validate_tools_json(&self) -> Result<(), String> {
        if std::fs::metadata(&self.tools_json_path).is_err() {
            return Err(t!(
                "gui.setup_tools.tools_json_not_found",
                path = self.tools_json_path.clone()
            )
            .to_string());
        }
        Ok(())
    }
}

const TOOLS_BASE_PERCENTAGE: u32 = 65;
const TOOLS_PERCENTAGE_RANGE: u32 = 25;
// Leaves room for the completion event
const TOOLS_MAX_PERCENTAGE: u32 = 89;

fn emit_stage(
    app_handle: &AppHandle,
    stage: InstallationStage,
    percentage: u32,
    message: String,
    detail: Option<String>,
    idf_version: &str,
) {
    emit_installation_event(
        app_handle,
        InstallationProgress {
            stage,
            percentage,
            message,
            detail,
            version: Some(idf_version.to_string()),
        },
    );
}

////////////////////// IMPORTANT MODIFY CLANG TOOL TO ALWAYS BE INSTALLED //////////////////////
// This is needed because the IDEs expect clang to be always installed
// ninja is included as without it the users are not able to actually build the projects
///////////////////////////////////////////////////////////////////////////////////////////////
fn force_required_tools(tools: &mut [Tool]) {
    for t in tools.iter_mut() {
        if t.name.contains("clang") || t.name.contains("ninja") {
            t.install = "always".to_string();
            debug!("{}: {}", t!("wizard.tools_json.modify_clang"), t.name);
        }
    }
}

/// Keeps "always" tools plus the user's selection for `idf_version`. Without any
/// per-version selection only "always" tools are kept; a selection map that has
/// no entry for `idf_version` leaves the list untouched.
fn filter_selected_tools(
    tools: &mut Vec<Tool>,
    tools_per_version: Option<&HashMap<String, Vec<String>>>,
    idf_version: &str,
) {
    let Some(per_version) = tools_per_version else {
        tools.retain(|tool| tool.install == "always");
        return;
    };
    if let Some(selected_tool_names) = per_version.get(idf_version) {
        tools.retain(|tool| tool.install == "always" || selected_tool_names.contains(&tool.name));
        info!(
            "Filtered to {} tools based on user selection for {}",
            tools.len(),
            idf_version
        );
    }
}

/// Derives a readable tool name from a download URL by dropping the
/// version/platform parts of the archive file name.
fn tool_name_from_url(url: &str) -> Option<String> {
    let filename = Path::new(url).file_name().and_then(|f| f.to_str())?;
    Some(
        filename
            .split('-')
            .take(3)
            .collect::<Vec<_>>()
            .join("-")
            .replace(".tar", "")
            .replace(".zip", ""),
    )
}

fn short_tool_name(tool_name: &str) -> String {
    tool_name
        .split('/')
        .next_back()
        .unwrap_or(tool_name)
        .replace("-", " ")
}

fn completed_tools_share(completed: u32, total_tools: f32) -> f32 {
    (completed as f32 / total_tools) * TOOLS_PERCENTAGE_RANGE as f32
}

fn tools_stage_percentage(completed: u32, total_tools: f32, offset: u32) -> u32 {
    (TOOLS_BASE_PERCENTAGE + completed_tools_share(completed, total_tools) as u32 + offset)
        .min(TOOLS_MAX_PERCENTAGE)
}

fn tool_download_percentage(completed: u32, total_tools: f32, tool_progress: u64) -> u32 {
    let current_tool_contribution =
        (tool_progress as f32 / 100.0) * (TOOLS_PERCENTAGE_RANGE as f32 / total_tools);
    (TOOLS_BASE_PERCENTAGE
        + completed_tools_share(completed, total_tools) as u32
        + current_tool_contribution as u32)
        .min(TOOLS_MAX_PERCENTAGE)
}

/// Per-tool milestone texts; `message_key` takes `tool_name`, `detail_key`
/// takes `current`/`total`, `log_key` takes `tool_name`.
struct ToolStep {
    percentage_offset: u32,
    message_key: &'static str,
    detail_key: &'static str,
    log_level: MessageLevel,
    log_key: &'static str,
}

const STARTED_STEP: ToolStep = ToolStep {
    percentage_offset: 0,
    message_key: "gui.setup_tools.preparing",
    detail_key: "gui.setup_tools.starting_tool",
    log_level: MessageLevel::Info,
    log_key: "gui.setup_tools.starting_download",
};

const DOWNLOADED_STEP: ToolStep = ToolStep {
    percentage_offset: 1,
    message_key: "gui.setup_tools.verifying",
    detail_key: "gui.setup_tools.downloaded_tool",
    log_level: MessageLevel::Info,
    log_key: "gui.setup_tools.downloaded",
};

const VERIFIED_STEP: ToolStep = ToolStep {
    percentage_offset: 2,
    message_key: "gui.setup_tools.extracting",
    detail_key: "gui.setup_tools.verified_tool",
    log_level: MessageLevel::Success,
    log_key: "gui.setup_tools.verified",
};

/// Translates library download events into GUI progress events for one
/// IDF version's tool installation.
#[derive(Clone)]
struct ToolProgressReporter {
    app_handle: AppHandle,
    idf_version: String,
    total_tools: f32,
    completed_tools: Arc<Mutex<u32>>,
    current_tool_name: Arc<Mutex<String>>,
}

impl ToolProgressReporter {
    fn new(app_handle: &AppHandle, idf_version: &str, total_tools: usize) -> Self {
        Self {
            app_handle: app_handle.clone(),
            idf_version: idf_version.to_string(),
            total_tools: total_tools as f32,
            completed_tools: Arc::new(Mutex::new(0u32)),
            current_tool_name: Arc::new(Mutex::new(String::new())),
        }
    }

    fn completed(&self) -> u32 {
        *self.completed_tools.lock().unwrap()
    }

    fn tool_name(&self) -> String {
        self.current_tool_name.lock().unwrap().clone()
    }

    fn total(&self) -> u32 {
        self.total_tools as u32
    }

    fn emit(&self, stage: InstallationStage, percentage: u32, message: String, detail: String) {
        emit_stage(
            &self.app_handle,
            stage,
            percentage,
            message,
            Some(detail),
            &self.idf_version,
        );
    }

    fn log(&self, level: MessageLevel, message: String) {
        emit_log_message(&self.app_handle, level, message);
    }

    fn handle(&self, progress: DownloadProgress) {
        match progress {
            DownloadProgress::Progress(current, total) => self.on_progress(current, total),
            DownloadProgress::Indeterminate(current) => self.on_indeterminate(current),
            DownloadProgress::Start(url) => self.on_start(&url),
            DownloadProgress::Downloaded(_url) => self.on_tool_step(&DOWNLOADED_STEP),
            DownloadProgress::Verified(_url) => self.on_tool_step(&VERIFIED_STEP),
            DownloadProgress::Extracted(_url, _dest) => self.on_extracted(),
            DownloadProgress::Complete => self.on_complete(),
            DownloadProgress::Error(err) => self.on_error(&err),
        }
    }

    fn on_progress(&self, current: u64, total: u64) {
        let Some(tool_progress) = (current * 100).checked_div(total) else {
            return;
        };
        let completed = self.completed();
        let tool_name = self.tool_name();
        self.emit(
            InstallationStage::Tools,
            tool_download_percentage(completed, self.total_tools, tool_progress),
            t!(
                "gui.setup_tools.downloading",
                tool_name = short_tool_name(&tool_name)
            )
            .to_string(),
            t!(
                "gui.setup_tools.tool_progress",
                current = completed + 1,
                total = self.total(),
                percentage = tool_progress
            )
            .to_string(),
        );
    }

    fn on_indeterminate(&self, current: u64) {
        let completed = self.completed();
        let tool_name = self.tool_name();
        self.emit(
            InstallationStage::Tools,
            completed_tools_share(completed, self.total_tools) as u32,
            t!(
                "gui.setup_tools.downloading",
                tool_name = short_tool_name(&tool_name)
            )
            .to_string(),
            t!(
                "gui.setup_tools.tool_progress_indeterminate",
                bytes = current,
            )
            .to_string(),
        );
    }

    fn on_start(&self, url: &str) {
        let tool_name = tool_name_from_url(url)
            .unwrap_or_else(|| t!("gui.setup_tools.unknown_tool").to_string());
        *self.current_tool_name.lock().unwrap() = tool_name;
        self.on_tool_step(&STARTED_STEP);
    }

    fn on_tool_step(&self, step: &ToolStep) {
        let completed = self.completed();
        let tool_name = self.tool_name();
        self.emit(
            InstallationStage::Tools,
            tools_stage_percentage(completed, self.total_tools, step.percentage_offset),
            t!(step.message_key, tool_name = tool_name.replace("-", " ")).to_string(),
            t!(
                step.detail_key,
                current = completed + 1,
                total = self.total()
            )
            .to_string(),
        );
        self.log(
            step.log_level.clone(),
            t!(step.log_key, tool_name = tool_name).to_string(),
        );
    }

    fn on_extracted(&self) {
        let completed_count = {
            let mut completed = self.completed_tools.lock().unwrap();
            *completed += 1;
            *completed
        };
        let tool_name = self.tool_name();
        self.emit(
            InstallationStage::Tools,
            tools_stage_percentage(completed_count, self.total_tools, 0),
            t!(
                "gui.setup_tools.installed",
                tool_name = tool_name.replace("-", " ")
            )
            .to_string(),
            t!(
                "gui.setup_tools.completed_tools",
                current = completed_count,
                total = self.total()
            )
            .to_string(),
        );
        self.log(
            MessageLevel::Success,
            t!(
                "gui.setup_tools.installed_tool",
                tool_name = tool_name,
                current = completed_count,
                total = self.total()
            )
            .to_string(),
        );
    }

    fn on_complete(&self) {
        self.emit(
            InstallationStage::Tools,
            TOOLS_MAX_PERCENTAGE,
            t!("gui.setup_tools.all_downloaded").to_string(),
            t!(
                "gui.setup_tools.completed_installation",
                count = self.total()
            )
            .to_string(),
        );
    }

    fn on_error(&self, err: &str) {
        let tool_name = self.tool_name();
        self.emit(
            InstallationStage::Error,
            0,
            t!("gui.setup_tools.tool_failed", tool_name = tool_name).to_string(),
            err.to_string(),
        );
        self.log(
            MessageLevel::Error,
            t!("gui.setup_tools.tool_error", error = err.to_string()).to_string(),
        );
    }
}

fn prepare_tool_setup(
    app_handle: &AppHandle,
    settings: &Settings,
    version_path: &Path,
) -> Result<ToolSetup> {
    let tool_setup = ToolSetup::new(settings, &PathBuf::from(version_path))
        .map_err(|e| anyhow!("Failed to initialize tool setup: {}", e))?;

    tool_setup
        .create_directories(app_handle)
        .map_err(|e| anyhow!("Failed to create tool directories: {}", e))?;

    tool_setup
        .validate_tools_json()
        .map_err(|e| anyhow!("Failed to validate tools.json: {}", e))?;

    Ok(tool_setup)
}

fn load_tools_file(app_handle: &AppHandle, tools_json_path: &str) -> Result<ToolsFile> {
    idf_tools::read_and_parse_tools_file(tools_json_path).map_err(|e| {
        emit_log_message(
            app_handle,
            MessageLevel::Error,
            t!(
                "gui.setup_tools.tools_json_parse_failed",
                error = e.to_string()
            )
            .to_string(),
        );
        anyhow!(t!(
            "gui.setup_tools.tools_json_parse_failed",
            error = e.to_string()
        )
        .to_string())
    })
}

fn qemu_prerequisites_failure(
    app_handle: &AppHandle,
    idf_version: &str,
    message: String,
    detail: String,
) -> anyhow::Error {
    emit_stage(
        app_handle,
        InstallationStage::Tools,
        TOOLS_BASE_PERCENTAGE,
        message,
        Some(detail),
        idf_version,
    );
    anyhow!(t!("gui.setup_tools.qemu_prerequisites_check.failed").to_string())
}

fn ensure_qemu_prerequisites(
    app_handle: &AppHandle,
    tools: &ToolsFile,
    idf_version: &str,
) -> Result<()> {
    if !tools.tools.iter().any(|x| x.name.contains("qemu")) {
        return Ok(());
    }
    match idf_im_lib::system_dependencies::check_qemu_prerequisites() {
        Err(err) => {
            error!("{}: {}", t!("wizard.qemu.prerequisites.check_error"), err);
            Err(qemu_prerequisites_failure(
                app_handle,
                idf_version,
                t!("gui.setup_tools.qemu_prerequisites_check.failed").to_string(),
                t!("gui.setup_tools.qemu_prerequisites_check.xyz").to_string(),
            ))
        }
        Ok(qemu_prereqs) if !qemu_prereqs.is_empty() => {
            error!(
                "{}: {:?}",
                t!("wizard.qemu.prerequisites.missing"),
                qemu_prereqs
            );
            Err(qemu_prerequisites_failure(
                app_handle,
                idf_version,
                t!("gui.setup_tools.qemu_prerequisites_check.missing").to_string(),
                t!(
                    "gui.setup_tools.qemu_prerequisites_check.missing_details",
                    list = qemu_prereqs.join(", ")
                )
                .to_string(),
            ))
        }
        Ok(_) => Ok(()),
    }
}

async fn download_and_install_tools(
    app_handle: &AppHandle,
    settings: &Settings,
    tools: &ToolsFile,
    tool_setup: &ToolSetup,
    idf_version: &str,
    is_simple_installation: bool,
) -> Result<HashMap<String, (String, idf_tools::Download)>> {
    let reporter = ToolProgressReporter::new(app_handle, idf_version, tools.tools.len());
    let progress_callback = move |progress: DownloadProgress| reporter.handle(progress);

    let tools_mirror_to_use = get_mirror_to_use(
        app_handle,
        MirrorType::IDFTools,
        settings,
        is_simple_installation,
    )
    .await;
    idf_tools::setup_tools(
        tools,
        settings.target.clone().unwrap_or_default(),
        &PathBuf::from(&tool_setup.download_dir),
        &PathBuf::from(&tool_setup.install_dir),
        Some(&tools_mirror_to_use),
        progress_callback,
    )
    .await
    .map_err(|e| {
        emit_stage(
            app_handle,
            InstallationStage::Error,
            0,
            t!("gui.setup_tools.setup_failed").to_string(),
            Some(e.to_string()),
            idf_version,
        );
        anyhow!("Failed to setup tools: {}", e)
    })
}

fn cleanup_tools_download_dir(app_handle: &AppHandle, download_dir: &str) {
    match std::fs::remove_dir_all(download_dir) {
        Ok(_) => {
            info!("Cleaned up tools download directory: {}", download_dir);
            emit_log_message(
                app_handle,
                MessageLevel::Success,
                t!("gui.setup_tools.cleanup.success").to_string(),
            );
        }
        Err(err) => {
            error!(
                "Failed to clean up tools download directory {}: {}",
                download_dir, err
            );
            emit_log_message(
                app_handle,
                MessageLevel::Error,
                t!("gui.setup_tools.cleanup.failure").to_string(),
            );
        }
    }
}

async fn install_python_environment(
    app_handle: &AppHandle,
    settings: &Settings,
    idf_version: &str,
    offline_archive_dir: Option<&Path>,
    is_simple_installation: bool,
) -> Result<()> {
    emit_stage(
        app_handle,
        InstallationStage::Python,
        90,
        t!("gui.setup_tools.python_setup_starting").to_string(),
        Some(t!("gui.setup_tools.python_installing").to_string()),
        idf_version,
    );

    let paths = settings
        .get_version_paths(idf_version)
        .map_err(|_err| anyhow!("Failed to setup environment paths for idf versions"))?;

    let features_for_version = settings.get_features_for_version(idf_version);

    info!(
        "Installing Python environment for {} with features: {:?}",
        idf_version, features_for_version
    );
    let pypi_mirror_to_use = get_mirror_to_use(
        app_handle,
        MirrorType::PyPI,
        settings,
        is_simple_installation,
    )
    .await;

    if let Err(err) = idf_im_lib::python_utils::install_python_env(
        &paths,
        &paths.actual_version,
        &paths.tool_install_directory,
        &features_for_version,
        offline_archive_dir,
        &Some(pypi_mirror_to_use),
    )
    .await
    {
        error!("Failed to install Python environment: {}", err);
        emit_stage(
            app_handle,
            InstallationStage::Error,
            0,
            t!("gui.setup_tools.python_setup_failed").to_string(),
            Some(err.to_string()),
            idf_version,
        );
        return Err(anyhow!("Failed to install Python environment: {}", err));
    }

    info!("Python environment installed");
    emit_stage(
        app_handle,
        InstallationStage::Python,
        93,
        t!("gui.setup_tools.python_configured").to_string(),
        Some(t!("gui.setup_tools.python_deps_installed").to_string()),
        idf_version,
    );
    emit_log_message(
        app_handle,
        MessageLevel::Success,
        t!("gui.setup_tools.python_installed").to_string(),
    );
    Ok(())
}

fn tool_exports(
    tools: ToolsFile,
    installed_tools_list: HashMap<String, (String, idf_tools::Download)>,
    tools_install_folder: &Path,
) -> (Vec<String>, Vec<(String, String)>) {
    let export_paths = idf_tools::get_tools_export_paths_from_list(
        tools.clone(),
        installed_tools_list.clone(),
        tools_install_folder.to_str().unwrap(),
    )
    .into_iter()
    .map(|p| {
        if std::env::consts::OS == "windows" {
            idf_im_lib::replace_unescaped_spaces_win(&p)
        } else {
            p
        }
    })
    .collect();

    let export_vars = idf_tools::get_tools_export_vars_from_list(
        tools,
        installed_tools_list,
        tools_install_folder.to_str().unwrap(),
    );
    (export_paths, export_vars)
}

/// Sets up ESP-IDF tools based on settings and IDF path
pub async fn setup_tools(
    app_handle: &AppHandle,
    settings: &Settings,
    idf_path: &Path,
    idf_version: &str,
    offline_archive_dir: Option<&Path>,
) -> Result<(Vec<String>, Vec<(String, String)>)> {
    info!("Setting up tools...");

    let is_simple_installation = crate::gui::app_state::is_simple_installation(app_handle);

    let version_path = idf_path
        .parent()
        .context("Failed to get parent directory of IDF path")?;

    let tool_setup = prepare_tool_setup(app_handle, settings, version_path)?;

    let mut tools = load_tools_file(app_handle, &tool_setup.tools_json_path)?;
    force_required_tools(&mut tools.tools);
    filter_selected_tools(
        &mut tools.tools,
        settings.idf_tools_per_version.as_ref(),
        idf_version,
    );

    ensure_qemu_prerequisites(app_handle, &tools, idf_version)?;

    emit_stage(
        app_handle,
        InstallationStage::Tools,
        TOOLS_BASE_PERCENTAGE,
        t!("gui.setup_tools.installation_starting").to_string(),
        Some(t!("gui.setup_tools.preparing_tools", count = tools.tools.len()).to_string()),
        idf_version,
    );

    let installed_tools_list = download_and_install_tools(
        app_handle,
        settings,
        &tools,
        &tool_setup,
        idf_version,
        is_simple_installation,
    )
    .await?;

    let tools_install_folder = PathBuf::from(&tool_setup.install_dir);
    info!(
        "Setting up tools... to directory: {}",
        tools_install_folder.display()
    );
    if settings.cleanup.unwrap_or(false) {
        cleanup_tools_download_dir(app_handle, &tool_setup.download_dir);
    }

    install_python_environment(
        app_handle,
        settings,
        idf_version,
        offline_archive_dir,
        is_simple_installation,
    )
    .await?;

    let (export_paths, export_vars) =
        tool_exports(tools, installed_tools_list, &tools_install_folder);

    emit_stage(
        app_handle,
        InstallationStage::Configure,
        95,
        t!("gui.setup_tools.configuring_env").to_string(),
        Some(t!("gui.setup_tools.configuring_dev_env").to_string()),
        idf_version,
    );

    emit_log_message(
        app_handle,
        MessageLevel::Success,
        t!("gui.setup_tools.setup_completed").to_string(),
    );

    Ok((export_paths, export_vars))
}

#[tauri::command]
pub async fn get_features_list_all_versions(
    app_handle: AppHandle,
) -> Result<Vec<VersionFeatureInfo>, String> {
    let settings = get_settings_non_blocking(&app_handle)?;

    let versions = match &settings.idf_versions {
        Some(versions) if !versions.is_empty() => versions.clone(),
        _ => {
            let msg = t!("wizard.requirements.no_idf_version_specified").to_string();
            warn!("{}", msg);
            return Err(msg);
        }
    };

    let mut result = Vec::new();

    for version in versions {
        let req_url = get_requirements_json_url(
            settings.repo_stub.clone().as_deref(),
            &version,
            settings.idf_mirror.clone().as_deref(),
        );

        info!(
            "Fetching requirements for version {} from: {}",
            version, req_url
        );

        let requirements_files = match RequirementsMetadata::from_url_async(&req_url).await {
            Ok(files) => files,
            Err(err) => {
                warn!(
                    "{}: {} for version {}. {}",
                    t!("wizard.requirements.read_failure"),
                    err,
                    version,
                    t!("wizard.features.selection_unavailable")
                );
                // Continue with other versions even if one fails
                continue;
            }
        };

        result.push(VersionFeatureInfo {
            version: version.clone(),
            features: requirements_files.features.clone(),
        });
    }

    if result.is_empty() {
        return Err(t!("wizard.requirements.no_features_available").to_string());
    }

    Ok(result)
}

enum PerVersionSelection {
    Features,
    Tools,
}

impl PerVersionSelection {
    fn label(&self) -> &'static str {
        match self {
            Self::Features => "features",
            Self::Tools => "tools",
        }
    }

    fn updated_message(&self) -> String {
        match self {
            Self::Features => t!("gui.settings.features_updated").to_string(),
            Self::Tools => t!("gui.settings.tools_updated").to_string(),
        }
    }

    fn field<'a>(
        &self,
        settings: &'a mut Settings,
    ) -> &'a mut Option<HashMap<String, Vec<String>>> {
        match self {
            Self::Features => &mut settings.idf_features_per_version,
            Self::Tools => &mut settings.idf_tools_per_version,
        }
    }
}

fn set_selection_per_version(
    app_handle: &AppHandle,
    kind: PerVersionSelection,
    selection: HashMap<String, Vec<String>>,
) -> Result<(), String> {
    info!(
        "Setting selected {} per version: {:?}",
        kind.label(),
        selection
    );
    update_settings_and_notify(app_handle, kind.updated_message(), |settings| {
        *kind.field(settings) = Some(selection)
    })
}

/// Sets the selected ESP-IDF features for each version
/// features_map: HashMap where key is version string, value is list of selected feature names
#[tauri::command]
pub fn set_selected_features_per_version(
    app_handle: AppHandle,
    features_map: HashMap<String, Vec<String>>,
) -> Result<(), String> {
    set_selection_per_version(&app_handle, PerVersionSelection::Features, features_map)
}

/// Gets the previously selected features per version (for restoring state)
#[tauri::command]
pub fn get_selected_features_per_version(
    app_handle: AppHandle,
) -> Result<HashMap<String, Vec<String>>, String> {
    get_settings_non_blocking(&app_handle)
        .map(|settings| settings.idf_features_per_version.unwrap_or_default())
}

/// Gets the list of available tools for all selected IDF versions
#[tauri::command]
pub async fn get_tools_list_all_versions(
    app_handle: AppHandle,
) -> Result<Vec<VersionToolsInfo>, String> {
    let settings = get_settings_non_blocking(&app_handle)?;

    let versions = match &settings.idf_versions {
        Some(versions) if !versions.is_empty() => versions.clone(),
        _ => {
            let msg = t!("wizard.tools.no_idf_version_specified").to_string();
            warn!("{}", msg);
            return Err(msg);
        }
    };

    let targets = settings.target.clone();

    let mut result = Vec::new();

    for version in versions {
        let tools_url = idf_im_lib::tool_selection::get_tools_json_url(
            settings.repo_stub.clone().as_deref(),
            &version,
            settings.idf_mirror.clone().as_deref(),
        );

        info!("Fetching tools for version {} from: {}", version, tools_url);

        let mut tools_file = match fetch_tools_file_async(&tools_url).await {
            Ok(file) => file,
            Err(err) => {
                warn!(
                    "{}: {} for version {}. {}",
                    t!("wizard.tools.read_failure"),
                    err,
                    version,
                    t!("wizard.tools.selection_unavailable")
                );
                // Continue with other versions even if one fails
                continue;
            }
        };

        force_required_tools(&mut tools_file.tools);

        let tools = match get_tools_for_selection(&tools_file, targets.as_deref()) {
            Ok(tools) => tools,
            Err(err) => {
                warn!("Failed to get tools for version {}: {}", version, err);
                continue;
            }
        };

        result.push(VersionToolsInfo {
            version: version.clone(),
            tools,
        });
    }

    if result.is_empty() {
        return Err(t!("wizard.tools.no_tools_available").to_string());
    }

    Ok(result)
}

/// Sets the selected tools for each version
/// tools_map: HashMap where key is version string, value is list of selected tool names
#[tauri::command]
pub fn set_selected_tools_per_version(
    app_handle: AppHandle,
    tools_map: HashMap<String, Vec<String>>,
) -> Result<(), String> {
    set_selection_per_version(&app_handle, PerVersionSelection::Tools, tools_map)
}

/// Gets the previously selected tools per version (for restoring state)
#[tauri::command]
pub fn get_selected_tools_per_version(
    app_handle: AppHandle,
) -> Result<HashMap<String, Vec<String>>, String> {
    get_settings_non_blocking(&app_handle)
        .map(|settings| settings.idf_tools_per_version.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, install: &str) -> Tool {
        Tool {
            description: String::new(),
            export_paths: Vec::new(),
            export_vars: HashMap::new(),
            info_url: String::new(),
            install: install.to_string(),
            license: None,
            name: name.to_string(),
            platform_overrides: None,
            supported_targets: None,
            strip_container_dirs: None,
            version_cmd: Vec::new(),
            version_regex: String::new(),
            version_regex_replace: None,
            versions: Vec::new(),
        }
    }

    fn names(tools: &[Tool]) -> Vec<&str> {
        tools.iter().map(|t| t.name.as_str()).collect()
    }

    #[test]
    fn per_version_selection_targets_matching_settings_field() {
        let mut settings = Settings::default();
        let selection = HashMap::from([("v5.4".to_string(), vec!["x".to_string()])]);
        *PerVersionSelection::Features.field(&mut settings) = Some(selection.clone());
        *PerVersionSelection::Tools.field(&mut settings) = None;
        assert_eq!(settings.idf_features_per_version, Some(selection));
        assert_eq!(settings.idf_tools_per_version, None);
        assert_eq!(PerVersionSelection::Features.label(), "features");
        assert_eq!(PerVersionSelection::Tools.label(), "tools");
    }

    #[test]
    fn force_required_tools_marks_clang_and_ninja_always() {
        let mut tools = vec![
            tool("esp-clang", "on_request"),
            tool("ninja", "never"),
            tool("qemu-xtensa", "on_request"),
        ];
        force_required_tools(&mut tools);
        let installs: Vec<&str> = tools.iter().map(|t| t.install.as_str()).collect();
        assert_eq!(installs, vec!["always", "always", "on_request"]);
    }

    #[test]
    fn filter_selected_tools_without_selection_keeps_only_always() {
        let mut tools = vec![tool("a", "always"), tool("b", "on_request")];
        filter_selected_tools(&mut tools, None, "v5.4");
        assert_eq!(names(&tools), vec!["a"]);
    }

    #[test]
    fn filter_selected_tools_keeps_always_and_selected() {
        let mut tools = vec![
            tool("a", "always"),
            tool("b", "on_request"),
            tool("c", "on_request"),
        ];
        let per_version = HashMap::from([("v5.4".to_string(), vec!["c".to_string()])]);
        filter_selected_tools(&mut tools, Some(&per_version), "v5.4");
        assert_eq!(names(&tools), vec!["a", "c"]);
    }

    #[test]
    fn filter_selected_tools_without_entry_for_version_keeps_all() {
        let mut tools = vec![tool("a", "always"), tool("b", "on_request")];
        let per_version = HashMap::from([("v5.3".to_string(), Vec::new())]);
        filter_selected_tools(&mut tools, Some(&per_version), "v5.4");
        assert_eq!(names(&tools), vec!["a", "b"]);
    }

    #[test]
    fn tool_name_from_url_strips_version_and_extension() {
        assert_eq!(
            tool_name_from_url("https://example.com/dl/xtensa-esp-elf-14.2.0-macos.tar.xz"),
            Some("xtensa-esp-elf".to_string())
        );
        assert_eq!(
            tool_name_from_url("https://example.com/ninja-mac.zip"),
            Some("ninja-mac".to_string())
        );
    }

    #[test]
    fn tool_name_from_url_without_file_name_is_none() {
        assert_eq!(tool_name_from_url(""), None);
        assert_eq!(tool_name_from_url(".."), None);
    }

    #[test]
    fn short_tool_name_takes_last_segment_and_spaces_dashes() {
        assert_eq!(short_tool_name("dir/esp-clang"), "esp clang");
        assert_eq!(short_tool_name("cmake"), "cmake");
    }

    #[test]
    fn tools_stage_percentage_scales_and_caps() {
        assert_eq!(tools_stage_percentage(0, 4.0, 0), 65);
        assert_eq!(tools_stage_percentage(2, 4.0, 1), 78);
        assert_eq!(tools_stage_percentage(4, 4.0, 2), 89);
    }

    #[test]
    fn tool_download_percentage_adds_current_tool_share() {
        assert_eq!(tool_download_percentage(0, 5.0, 0), 65);
        assert_eq!(tool_download_percentage(1, 5.0, 50), 72);
        assert_eq!(tool_download_percentage(5, 5.0, 100), 89);
    }

    #[test]
    fn completed_tools_share_is_fraction_of_range() {
        assert_eq!(completed_tools_share(1, 5.0) as u32, 5);
        assert_eq!(completed_tools_share(5, 5.0) as u32, 25);
    }
}
