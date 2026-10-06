use anyhow::Result;
use idf_im_lib::cli_progress::show_download_progress;
use idf_im_lib::idf_features::get_requirements_json_url;
use idf_im_lib::idf_features::FeatureInfo;
use idf_im_lib::idf_features::RequirementsMetadata;
use idf_im_lib::idf_tools::get_tools_export_vars_from_list;
use idf_im_lib::idf_tools::{Tool, ToolsFile};
use idf_im_lib::offline_installer::copy_components_from_offline_archive;
use idf_im_lib::offline_installer::copy_idf_from_offline_archive;
use idf_im_lib::offline_installer::install_prerequisites_offline;
use idf_im_lib::offline_installer::merge_requirements_files;
use idf_im_lib::offline_installer::use_offline_archive;
use idf_im_lib::settings::{Settings, VersionPaths};
use idf_im_lib::tool_selection::fetch_tools_file;
use idf_im_lib::tool_selection::get_tool_names;
use idf_im_lib::tool_selection::get_tools_json_url;
use idf_im_lib::tool_selection::ToolSelectionInfo;
use idf_im_lib::utils::copy_dir_contents;
use idf_im_lib::{ensure_path, DownloadProgress};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{debug, error, info, warn};
use rust_i18n::t;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::{
    fmt::Write,
    fs::{self},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

// maybe move the default values to the config too?
const DEFAULT_TOOLS_DOWNLOAD_FOLDER: &str = "dist";
const DEFAULT_TOOLS_INSTALL_FOLDER: &str = "tools";
const DEFAULT_TOOLS_JSON_LOCATION: &str = "tools/tools.json";

use crate::cli::helpers::{generic_confirm, generic_input, generic_select_index};

use crate::cli::prompts::*;

async fn select_targets_and_versions(mut config: Settings) -> Result<Settings, String> {
    if (config.wizard_all_questions.unwrap_or_default()
        || config.target.is_none()
        || config.is_default("target"))
        && config.non_interactive == Some(false)
    {
        config.target = Some(select_target().await?);
    }
    let target = config.target.clone().unwrap_or_default();
    debug!(
        "{}",
        t!("wizard.debug.target.selected", target = target.join(", "))
    );

    // here the non-interactive flag is passed to the inner function
    if config.wizard_all_questions.unwrap_or_default()
        || config.idf_versions.is_none()
        || config.is_default("idf_versions")
    {
        config.idf_versions =
            Some(select_idf_version(&target[0], config.non_interactive.unwrap_or_default()).await?);
        // TODO: handle multiple targets
    }
    let idf_versions = config.idf_versions.clone().unwrap_or_default();
    debug!(
        "{}",
        t!(
            "wizard.debug.idf_version.selected",
            version = idf_versions.join(", ")
        )
    );

    Ok(config)
}

pub struct DownloadConfig {
    pub idf_path: String,
    pub repo_stub: Option<String>,
    pub idf_version: String,
    pub idf_mirror: Option<String>,
    pub recurse_submodules: Option<bool>,
    pub non_interactive: Option<bool>,
}

pub enum DownloadError {
    PathCreationFailed(String),
    DownloadFailed(String),
    UserCancelled,
}

fn handle_download_error(err: String) -> Result<(), DownloadError> {
    let err_string = err;
    if err_string.contains("exists") || err_string.contains("not empty") {
        match generic_confirm("wizard.idf_path_exists.prompt") {
            Ok(true) => Ok(()),
            Ok(false) => Err(DownloadError::UserCancelled),
            Err(e) => Err(DownloadError::DownloadFailed(e.to_string())),
        }
    } else {
        Err(DownloadError::DownloadFailed(err_string))
    }
}

pub fn download_idf(config: DownloadConfig) -> Result<(), DownloadError> {
    idf_im_lib::ensure_path(&config.idf_path)
        .map_err(|err| DownloadError::PathCreationFailed(err.to_string()))?;

    let (tx, rx) = mpsc::channel();

    // Spawn a thread to handle progress bar updates
    let handle = thread::spawn(move || {
        show_download_progress(
            rx,
            |name, value| {
                t!(
                    "wizard.debug.submodule.progress",
                    name = name,
                    progress = value
                )
                .to_string()
            },
            |name| info!("{}: {}", t!("wizard.idf.submodule_finish"), name),
        )
    });

    info!("{}", t!("wizard.idf.cloning"));

    match idf_im_lib::git_tools::get_esp_idf(
        &config.idf_path,
        config.repo_stub.as_deref(),
        &config.idf_version,
        config.idf_mirror.as_deref(),
        config.recurse_submodules.unwrap_or_default(),
        tx,
    ) {
        Ok(_) => {
            debug!("{}", t!("wizard.idf.success"));
            match handle.join() {
                Ok(_) => {
                    debug!("{}", t!("wizard.idf.progress_bar.join"));
                }
                Err(_err) => {
                    error!("{}", t!("wizard.idf.progress_bar.error"));
                }
            }
            Ok(())
        }
        Err(err) => {
            if config.non_interactive == Some(true) {
                // No user to prompt in non-interactive mode. Preserve the
                // idempotent "already exists" behaviour, but surface real
                // download/network failures instead of silently continuing
                // (which previously masked the cause with downstream errors).
                if (err.contains("exists") && !err.contains("does not exist"))
                    || err.contains("not empty")
                {
                    warn!(
                        "IDF path already present; continuing (non-interactive): {}",
                        err
                    );
                    Ok(())
                } else {
                    Err(DownloadError::DownloadFailed(err))
                }
            } else {
                handle_download_error(err)
            }
        }
    }
}

fn setup_directory(
    wizard_all_questions: Option<bool>,
    base_path: &Path,
    config_field: &mut Option<String>,
    prompt_key: &str,
    default_name: &str,
) -> Result<PathBuf, String> {
    let mut directory = base_path.to_path_buf();

    if let Some(name) = config_field.clone() {
        directory.push(name);
    } else if wizard_all_questions.unwrap_or(false) {
        let name = generic_input(prompt_key, &format!("{}.failure", prompt_key), default_name)?;
        directory.push(&name);
        *config_field = Some(name);
    } else {
        directory.push(default_name);
        *config_field = Some(default_name.to_string());
    }

    idf_im_lib::ensure_path(&directory.display().to_string()).map_err(|err| err.to_string())?;
    Ok(directory)
}

fn get_tools_json_path(config: &mut Settings, idf_path: &Path) -> PathBuf {
    let mut tools_json_file = idf_path.to_path_buf();

    if let Some(file) = &config.tools_json_file {
        tools_json_file.push(file);
    } else if config.wizard_all_questions.unwrap_or(false) {
        let name = generic_input(
            "wizard.tools_json.prompt",
            "wizard.tools_json.prompt.failure",
            DEFAULT_TOOLS_JSON_LOCATION,
        )
        .unwrap();
        tools_json_file.push(&name);
        config.tools_json_file = Some(name);
    } else {
        tools_json_file.push(DEFAULT_TOOLS_JSON_LOCATION);
        config.tools_json_file = Some(DEFAULT_TOOLS_JSON_LOCATION.to_string());
    }

    tools_json_file
}

async fn download_and_extract_tools(
    config: &Settings,
    tools: &ToolsFile,
    download_dir: &Path,
    install_dir: &Path,
) -> anyhow::Result<HashMap<String, (String, idf_im_lib::idf_tools::Download)>> {
    info!(
        "{}: {:?}",
        t!("wizard.tools_download.progress"),
        download_dir.display()
    );
    let progress_bar = ProgressBar::new(0);
    progress_bar.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})").unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));

    let progress_callback = move |progress: DownloadProgress| match progress {
        DownloadProgress::Progress(current, total) => {
            progress_bar.set_length(total);
            progress_bar.set_position(current);
        }
        DownloadProgress::Indeterminate(current) => {
            progress_bar.set_length(current * 2); // Set a length to allow the bar to move
            progress_bar.set_position(current);
        }
        DownloadProgress::Complete => {
            progress_bar.finish();
        }
        DownloadProgress::Error(err) => {
            progress_bar.abandon_with_message(format!("Error: {}", err));
        }
        DownloadProgress::Start(_) => {
            progress_bar.set_position(0);
        }
        DownloadProgress::Downloaded(url) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                info!(
                    "{}",
                    t!("wizard.tool.download.success", filename = filename)
                );
            }
        }
        DownloadProgress::Verified(url) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                info!("{}", t!("wizard.tool.verified", filename = filename));
            }
        }
        DownloadProgress::Extracted(url, dest) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                info!(
                    "{}",
                    t!(
                        "wizard.tool.extract.success",
                        filename = filename,
                        dest = dest
                    )
                );
            }
        }
    };

    idf_im_lib::idf_tools::setup_tools(
        tools,
        config.target.clone().unwrap(),
        download_dir,
        install_dir,
        config.mirror.as_deref(),
        progress_callback,
    )
    .await
}

/// Checks for incomplete (non-Finished) installations and interactively offers fix or delete.
/// Only called in interactive mode (non_interactive == false).
pub async fn check_and_handle_incomplete_installations(
    _settings: &Settings,
    config_path: Option<&std::path::PathBuf>,
) {
    use idf_im_lib::idf_config::{IdfConfig, InstallationStatus};
    use idf_im_lib::version_manager::{get_default_config_path, remove_single_idf_version};

    let resolved = config_path.cloned().unwrap_or_else(get_default_config_path);

    let config = match IdfConfig::from_file(&resolved) {
        Ok(c) => c,
        Err(_) => return,
    };

    let incomplete: Vec<_> = config
        .get_incomplete_installations()
        .into_iter()
        .cloned()
        .collect();
    if incomplete.is_empty() {
        return;
    }

    println!(
        "{}",
        t!("wizard.incomplete.found", count = incomplete.len())
    );

    for installation in &incomplete {
        let status_label = match installation.status {
            InstallationStatus::InProgress => t!("list.status.in_progress").to_string(),
            InstallationStatus::Failed => t!("list.status.failed").to_string(),
            InstallationStatus::BeingRepaired => t!("list.status.being_repaired").to_string(),
            InstallationStatus::Broken => t!("list.status.broken").to_string(),
            InstallationStatus::Finished => continue,
        };

        println!(
            "{}",
            t!(
                "wizard.incomplete.entry",
                name = installation.name.clone(),
                status = status_label,
                path = installation.path.clone()
            )
        );

        let options = vec![
            t!("wizard.incomplete.action.fix").to_string(),
            t!("wizard.incomplete.action.delete").to_string(),
            t!("wizard.incomplete.action.skip").to_string(),
        ];

        let choice_idx = match generic_select_index("wizard.incomplete.action_prompt", &options) {
            Ok(i) => i,
            Err(_) => continue,
        };

        match choice_idx {
            0 => {
                info!(
                    "User chose to fix incomplete installation: {}",
                    installation.name
                );
                let fix_settings =
                    match idf_im_lib::version_manager::prepare_settings_for_fix_idf_installation(
                        std::path::PathBuf::from(&installation.path),
                        Some(&resolved),
                    )
                    .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to prepare fix: {}", e);
                            continue;
                        }
                    };

                match run_wizard_run(fix_settings).await {
                    Ok(_) => info!("Successfully fixed installation: {}", installation.name),
                    Err(e) => error!("Failed to fix installation {}: {}", installation.name, e),
                }
            }
            1 => match remove_single_idf_version(&installation.name, true, Some(&resolved)) {
                Ok(_) => info!("Deleted incomplete installation: {}", installation.name),
                Err(e) => error!("Failed to delete installation {}: {}", installation.name, e),
            },
            _ => {} // skip: do nothing
        }
    }
}

pub async fn run_wizard_run(mut config: Settings) -> Result<(), String> {
    debug!(
        "{}",
        t!(
            "wizard.debug.config_entering",
            config = format!("{:?}", config)
        )
    );

    let offline_mode = config.use_local_archive.is_some();
    let offline_archive_dir = if offline_mode {
        Some(TempDir::new().expect(&t!("wizard.error.create_temp_dir")))
    } else {
        None
    };
    let tool_install_directory = tool_install_folder(&config);

    if let Some(archive_dir) = offline_archive_dir.as_ref() {
        config = prepare_offline_archive(config, archive_dir, &tool_install_directory).await?;
    }

    check_prerequisites_and_python(&config, offline_mode, &tool_install_directory).await?;

    if let Some(archive_dir) = offline_archive_dir.as_ref() {
        copy_offline_idf_and_components(archive_dir, &config, &tool_install_directory)?;
    }

    // select target & idf version
    config = select_targets_and_versions(config).await?;

    // mirrors select (skip in offline mode - mirrors aren't used)
    if !offline_mode {
        config = select_mirrors(config).await?;
    }

    config = select_installation_path(config)?;

    // initialize the per-version map if not already set
    if config.idf_features_per_version.is_none() {
        config.idf_features_per_version = Some(HashMap::new());
    }
    for idf_version in config.idf_versions.clone().unwrap() {
        install_idf_version(&mut config, &idf_version, offline_archive_dir.as_ref()).await?;
    }
    save_config_if_desired(&config)?;
    save_ide_config(&config)?;
    print_finish_steps(&config)
}

fn tool_install_folder(config: &Settings) -> PathBuf {
    PathBuf::from(
        config
            .tool_install_folder_name
            .clone()
            .expect("Tools install folder not defined"),
    )
}

fn version_paths(config: &Settings, idf_version: &str) -> Result<VersionPaths, String> {
    config.get_version_paths(idf_version).map_err(|err| {
        error!("Failed to get version paths: {}", err);
        err.to_string()
    })
}

async fn prepare_offline_archive(
    config: Settings,
    archive_dir: &TempDir,
    tool_install_directory: &Path,
) -> Result<Settings, String> {
    let config = match use_offline_archive(config, archive_dir) {
        Ok(updated_config) => updated_config,
        Err(err) => {
            error!("Failed to use offline archive: {}", err);
            return Err(err);
        }
    };
    if std::env::consts::OS == "windows" {
        install_prerequisites_offline(archive_dir, tool_install_directory.to_path_buf())
            .await
            .map_err(|err| {
                t!(
                    "wizard.error.prerequisites_offline_install",
                    error = err.to_string()
                )
                .to_string()
            })?;
        info!("{}", t!("wizard.prerequisites.offline_install.success"));
    }
    Ok(config)
}

async fn check_prerequisites_and_python(
    config: &Settings,
    offline_mode: bool,
    tool_install_directory: &Path,
) -> Result<(), String> {
    let non_interactive = config.non_interactive.unwrap_or_default();
    let install_all_prerequisites = config.install_all_prerequisites.unwrap_or_default();
    if config.skip_prerequisites_check.unwrap_or(false) {
        info!("{}", t!("wizard.prerequisites.skip_check"));
    } else {
        check_and_install_prerequisites(
            non_interactive,
            install_all_prerequisites,
            tool_install_directory.to_path_buf(),
        )
        .await?;
    }

    check_and_install_python(
        non_interactive,
        install_all_prerequisites,
        config.python_version_override.clone(),
        offline_mode,
        tool_install_directory.to_path_buf(),
    )
    .await
}

fn copy_offline_idf_and_components(
    archive_dir: &TempDir,
    config: &Settings,
    tool_install_directory: &Path,
) -> Result<(), String> {
    copy_idf_from_offline_archive(archive_dir, config)?;
    let components_dir = tool_install_folder(config);
    match copy_components_from_offline_archive(archive_dir, &components_dir, tool_install_directory)
    {
        Ok(_) => {
            info!("{}", t!("wizard.components_copy.success"));
        }
        Err(err) => {
            warn!("{}: {}", t!("wizard.components_copy.failure"), err);
            // The component will be downloaded during first build and not all of them are actually needed but most importantly this is for backward compatibility with old IDF not supporting the new component manager, so we don't want to block the installation if the copy fails
        }
    }
    Ok(())
}

async fn install_idf_version(
    config: &mut Settings,
    idf_version: &str,
    offline_archive_dir: Option<&TempDir>,
) -> Result<(), String> {
    let offline_mode = offline_archive_dir.is_some();
    let paths = version_paths(config, idf_version)?;
    config.idf_path = Some(paths.idf_path.clone());
    idf_im_lib::add_path_to_path(paths.idf_path.to_str().unwrap());

    let requirements_files = if offline_mode {
        match offline_requirements(&paths.idf_path) {
            Ok(metadata) => metadata,
            Err(e) => {
                error!("Failed to merge requirements: {}", e);
                return Ok(());
            }
        }
    } else {
        fetch_requirements(config, idf_version)
    };

    let features_names =
        select_version_features(config, idf_version, &requirements_files, offline_mode)?;
    let selected_tools = select_version_tools(config, idf_version, offline_mode)?;
    ensure_idf_present(config, idf_version, &paths, offline_mode)?;

    let tool_download_directory = setup_directory(
        config.wizard_all_questions,
        &paths.version_installation_path,
        &mut config.tool_download_folder_name,
        "wizard.tools.download.prompt",
        DEFAULT_TOOLS_DOWNLOAD_FOLDER,
    )?;

    if let Some(archive_dir) = offline_archive_dir {
        copy_dir_contents(&archive_dir.path().join("dist"), &tool_download_directory)
            .map_err(|err| t!("wizard.error.copy_dist_directory", error = err.to_string()))?;
    }

    let tool_install_directory = setup_directory(
        config.wizard_all_questions,
        &paths.version_installation_path,
        &mut config.tool_install_folder_name,
        "wizard.tools.install.prompt",
        DEFAULT_TOOLS_INSTALL_FOLDER,
    )?;

    idf_im_lib::add_path_to_path(tool_install_directory.to_str().unwrap());

    let (tools, tools_json_file) =
        load_tools_to_install(config, &paths.idf_path, idf_version, &selected_tools)?;
    let installed_tools_list = install_tools(
        config,
        &tools,
        &tools_json_file,
        &tool_download_directory,
        &tool_install_directory,
    )
    .await?;

    setup_python_env(
        config,
        &paths,
        &tool_install_directory,
        &features_names,
        offline_archive_dir,
    )
    .await?;

    run_post_install(
        config,
        &paths,
        &tool_install_directory,
        tools,
        installed_tools_list,
        offline_mode,
    )
}

fn merged_requirements_metadata(requirements_dir: &Path) -> RequirementsMetadata {
    let requirements_file = requirements_dir
        .join("requirements.merged.txt")
        .to_string_lossy()
        .to_string();
    let feature_info = FeatureInfo {
        name: "merged".to_string(),
        description: Some(
            "All merged ESP-IDF features available in the offline installer".to_string(),
        ),
        optional: false,
        requirement_path: requirements_file,
    };
    RequirementsMetadata {
        version: 1,
        features: vec![feature_info],
    }
}

fn offline_requirements(idf_path: &Path) -> std::io::Result<RequirementsMetadata> {
    let requirements_dir = idf_path.join("tools").join("requirements");
    merge_requirements_files(&requirements_dir)?;
    Ok(merged_requirements_metadata(&requirements_dir))
}

fn fetch_requirements(config: &Settings, idf_version: &str) -> RequirementsMetadata {
    let req_url = get_requirements_json_url(
        config.repo_stub.as_deref(),
        idf_version,
        config.idf_mirror.as_deref(),
    );
    debug!("repo url: {} ", req_url);
    match RequirementsMetadata::from_url(&req_url) {
        Ok(files) => files,
        Err(err) => {
            warn!(
                "{}: {}. {}",
                t!("wizard.requirements.read_failure"),
                err,
                t!("wizard.features.selection_unavailable")
            );
            // Missing option for feature selection should not block installation
            // This is exceptionally important for headless run against private repos
            RequirementsMetadata {
                version: 1,
                features: vec![],
            }
        }
    }
}

/// Picks the features for `idf_version` (from CLI/config, a prompt, or none in
/// offline mode), records them in the per-version map and returns their names.
fn select_version_features(
    config: &mut Settings,
    idf_version: &str,
    requirements_files: &RequirementsMetadata,
    offline_mode: bool,
) -> Result<Vec<String>, String> {
    let features = if let Some(existing) = config.get_features_for_version_if_set(idf_version) {
        features_matching_names(requirements_files, &existing)
    } else if !offline_mode {
        select_features(
            requirements_files,
            config.non_interactive.unwrap_or_default(),
            true,
        )?
    } else {
        Vec::new() // Empty - will use defaults later
    };
    let features_names = features
        .iter()
        .map(|f| f.name.clone())
        .collect::<Vec<String>>();

    debug!(
        "{}: {}",
        t!("wizard.features.selected"),
        features_names.join(", ")
    );
    if let Some(ref mut per_version) = config.idf_features_per_version {
        per_version.insert(idf_version.to_string(), features_names.clone());
    }
    Ok(features_names)
}

fn fetch_remote_tools_file(config: &Settings, idf_version: &str) -> Option<ToolsFile> {
    let tools_url = get_tools_json_url(
        config.repo_stub.as_deref(),
        idf_version,
        config.idf_mirror.as_deref(),
    );

    match fetch_tools_file(&tools_url) {
        Ok(file) => Some(file),
        Err(err) => {
            warn!(
                "{}: {}. {}",
                t!("wizard.tools.read_failure"),
                err,
                t!("wizard.tools.selection_unavailable")
            );
            None
        }
    }
}

/// Picks the tools for `idf_version` from the remote tools.json (when it can be
/// fetched), checks QEMU prerequisites and records the choice per version.
fn select_version_tools(
    config: &mut Settings,
    idf_version: &str,
    offline_mode: bool,
) -> Result<Vec<ToolSelectionInfo>, String> {
    let remote_tools_file = if offline_mode {
        None
    } else {
        fetch_remote_tools_file(config, idf_version)
    };

    let selected_tools = if let Some(mut tools_file) = remote_tools_file {
        force_always_install(&mut tools_file.tools);
        let existing = config.get_tools_for_version_if_set(idf_version);
        select_tools(
            &tools_file,
            config.non_interactive.unwrap_or_default(),
            true,
            config.target.as_deref(),
            existing.as_deref(),
        )?
    } else {
        Vec::new() // Empty - will use defaults later
    };

    debug!(
        "{}: {}",
        t!("wizard.tools.selected"),
        get_tool_names(&selected_tools).join(", ")
    );
    if contains_qemu(selected_tools.iter().map(|t| t.name.as_str())) {
        ensure_qemu_prerequisites()?;
    }
    record_selected_tools(config, idf_version, &selected_tools);
    Ok(selected_tools)
}

// IDEs expect clang to always be installed, and without ninja users can't
// actually build their projects.
fn force_always_install(tools: &mut [Tool]) {
    for t in tools.iter_mut() {
        if t.name.contains("clang") || t.name.contains("ninja") {
            t.install = "always".to_string();
            debug!("{}: {}", t!("wizard.tools_json.modify_clang"), t.name);
        }
    }
}

fn contains_qemu<'a>(mut tool_names: impl Iterator<Item = &'a str>) -> bool {
    tool_names.any(|name| name.contains("qemu"))
}

fn ensure_qemu_prerequisites() -> Result<(), String> {
    match idf_im_lib::system_dependencies::check_qemu_prerequisites() {
        Ok(prereqs) if !prereqs.is_empty() => {
            error!("{}: {:?}", t!("wizard.qemu.prerequisites.missing"), prereqs);
            Err(t!("wizard.qemu.prerequisites.unmet").to_string())
        }
        Err(err) => {
            error!("{}: {}", t!("wizard.qemu.prerequisites.check_error"), err);
            Err(t!("wizard.qemu.prerequisites.unmet").to_string())
        }
        Ok(_) => Ok(()),
    }
}

fn record_selected_tools(
    config: &mut Settings,
    idf_version: &str,
    selected_tools: &[ToolSelectionInfo],
) {
    if selected_tools.is_empty() {
        return;
    }
    config
        .idf_tools_per_version
        .get_or_insert_with(HashMap::new)
        .insert(idf_version.to_string(), get_tool_names(selected_tools));
}

fn ensure_idf_present(
    config: &Settings,
    idf_version: &str,
    paths: &VersionPaths,
    offline_mode: bool,
) -> Result<(), String> {
    if paths.using_existing_idf {
        return Ok(());
    }
    if offline_mode {
        error!("THIS SHOULD NOT HAPPEN: offline mode should be using existing IDF copied from offline archive, but it seems like the IDF is not present at the expected location. This likely means that the copy from offline archive failed. Please check previous logs for any errors related to copying IDF from offline archive.");
        return Ok(());
    }
    let download_config = DownloadConfig {
        idf_path: paths.idf_path.to_str().unwrap().to_string(),
        repo_stub: config.repo_stub.clone(),
        idf_version: idf_version.to_string(),
        idf_mirror: config.idf_mirror.clone(),
        recurse_submodules: config.recurse_submodules,
        non_interactive: config.non_interactive,
    };

    download_idf(download_config).map_err(report_download_error)?;
    debug!("{}", t!("wizard.idf.success"));
    Ok(())
}

fn report_download_error(err: DownloadError) -> String {
    match err {
        DownloadError::PathCreationFailed(err) => {
            error!("{} {:?}", t!("wizard.idf.path_creation_failure"), err);
            err
        }
        DownloadError::DownloadFailed(err) => {
            error!("{} {:?}", t!("wizard.idf.failure"), err);
            err
        }
        DownloadError::UserCancelled => {
            error!("{}", t!("wizard.idf.user_cancelled"));
            "User cancelled the operation".to_string()
        }
    }
}

fn load_tools_to_install(
    config: &mut Settings,
    idf_path: &Path,
    idf_version: &str,
    selected_tools: &[ToolSelectionInfo],
) -> Result<(ToolsFile, PathBuf), String> {
    let tools_json_file = get_tools_json_path(config, idf_path);
    let validated_file = tools_json_file.to_string_lossy().to_string();

    debug!(
        "{}",
        t!(
            "wizard.debug.tools_json_file",
            path = tools_json_file.display()
        )
    );

    let mut tools = idf_im_lib::idf_tools::read_and_parse_tools_file(&validated_file)
        .map_err(|err| format!("{}: {}", t!("wizard.tools_json.unparsable"), err))?;
    force_always_install(&mut tools.tools);
    filter_tools_to_install(
        &mut tools.tools,
        config.idf_tools_per_version.as_ref(),
        idf_version,
        selected_tools,
    );

    if contains_qemu(tools.tools.iter().map(|t| t.name.as_str())) {
        ensure_qemu_prerequisites()?;
    }
    debug!(
        "{}: {}",
        t!("wizard.tools.to_install"),
        tools
            .tools
            .iter()
            .map(|t| t.name.clone())
            .collect::<Vec<String>>()
            .join(", ")
    );
    Ok((tools, tools_json_file))
}

/// Keeps "always" tools plus the tools chosen for `idf_version`. Without a
/// per-version map the current `selected_tools` are used; a map without an
/// entry for `idf_version` leaves the list untouched.
fn filter_tools_to_install(
    tools: &mut Vec<Tool>,
    tools_per_version: Option<&HashMap<String, Vec<String>>>,
    idf_version: &str,
    selected_tools: &[ToolSelectionInfo],
) {
    let Some(per_version) = tools_per_version else {
        tools.retain(|tool| {
            tool.install == "always" || selected_tools.iter().any(|t| t.name == tool.name)
        });
        return;
    };
    let Some(selected_tool_names) = per_version.get(idf_version) else {
        return;
    };
    info!(
        "{}: {}",
        t!("wizard.tools.filtering.selected_tools"),
        selected_tool_names.join(", ")
    );
    tools.retain(|tool| tool.install == "always" || selected_tool_names.contains(&tool.name));
    info!(
        "Filtered to {} tools based on user selection for {}",
        tools.len(),
        idf_version
    );
}

async fn install_tools(
    config: &Settings,
    tools: &ToolsFile,
    tools_json_file: &Path,
    tool_download_directory: &Path,
    tool_install_directory: &Path,
) -> Result<HashMap<String, (String, idf_im_lib::idf_tools::Download)>, String> {
    let installed_tools_list = match download_and_extract_tools(
        config,
        tools,
        tool_download_directory,
        tool_install_directory,
    )
    .await
    {
        Ok(list) => {
            info!(
                "{}: {}",
                t!("wizard.tools.downloaded"),
                tools_json_file.display()
            );
            list
        }
        Err(err) => {
            error!("Failed to download and extract tools: {}", err);
            return Err(err.to_string());
        }
    };
    if config.cleanup.unwrap_or(false) {
        remove_download_directory(tool_download_directory);
    }
    Ok(installed_tools_list)
}

fn remove_download_directory(tool_download_directory: &Path) {
    match fs::remove_dir_all(tool_download_directory) {
        Ok(_) => {
            info!(
                "{}: {}",
                t!("wizard.tools.cleanup.success"),
                tool_download_directory.display()
            );
        }
        Err(err) => {
            warn!(
                "{}: {}. {}",
                t!("wizard.tools.cleanup.failure"),
                tool_download_directory.display(),
                err
            );
        }
    }
}

async fn setup_python_env(
    config: &Settings,
    paths: &VersionPaths,
    tool_install_directory: &Path,
    features_names: &[String],
    offline_archive_dir: Option<&TempDir>,
) -> Result<(), String> {
    info!("{}", t!("wizard.python.reusing_env"));
    match idf_im_lib::python_utils::install_python_env(
        paths,
        &paths.actual_version,
        tool_install_directory,
        features_names,
        offline_archive_dir.map(|dir| dir.path()),
        &config.pypi_mirror,
    )
    .await
    {
        Ok(_) => {
            info!("{}", t!("wizard.python.env_installed"));
            Ok(())
        }
        Err(err) => {
            error!("Failed to install Python environment: {}", err);
            Err(err.to_string())
        }
    }
}

fn escape_export_paths_for_os(export_paths: Vec<String>, os: &str) -> Vec<String> {
    if os == "windows" {
        export_paths
            .into_iter()
            .map(|p| idf_im_lib::replace_unescaped_spaces_win(&p))
            .collect()
    } else {
        export_paths
    }
}

fn run_post_install(
    config: &Settings,
    paths: &VersionPaths,
    tool_install_directory: &Path,
    tools: ToolsFile,
    installed_tools_list: HashMap<String, (String, idf_im_lib::idf_tools::Download)>,
    offline_mode: bool,
) -> Result<(), String> {
    ensure_path(paths.python_venv_path.to_str().unwrap())
        .map_err(|err| t!("wizard.error.create_python_env", error = err.to_string()))?;

    let export_paths = escape_export_paths_for_os(
        idf_im_lib::idf_tools::get_tools_export_paths_from_list(
            tools.clone(),
            installed_tools_list.clone(),
            tool_install_directory.to_str().unwrap(),
        ),
        std::env::consts::OS,
    );
    let export_vars = get_tools_export_vars_from_list(
        tools,
        installed_tools_list,
        tool_install_directory.to_str().unwrap(),
    );
    let skip_component_installation = offline_mode || config.skip_components_download == Some(true);
    idf_im_lib::single_version_post_install(
        paths.activation_script_path.to_str().unwrap(),
        paths.idf_path.to_str().unwrap(),
        &paths.actual_version,
        tool_install_directory.to_str().unwrap(),
        export_paths,
        paths.python_venv_path.to_str(),
        Some(export_vars),
        &paths.python_path.to_string_lossy(),
        config.create_bat_activation_script.unwrap_or(false),
        skip_component_installation,
        false, // is_gui
    );
    Ok(())
}

fn save_ide_config(config: &Settings) -> Result<(), String> {
    let ide_conf_path_tmp = PathBuf::from(&config.esp_idf_json_path.clone().unwrap_or_default());
    debug!(
        "{}",
        t!(
            "wizard.debug.ide_config_path",
            path = ide_conf_path_tmp.display()
        )
    );
    if let Err(err) = ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
        error!("Failed to create IDE configuration directory: {}", err);
        return Err(err.to_string());
    }
    match config.save_esp_ide_json() {
        Ok(_) => {
            debug!("{}", t!("wizard.debug.ide_config_saved"));
            Ok(())
        }
        Err(err) => {
            error!("Failed to save IDE configuration: {}", err);
            Err(err.to_string())
        }
    }
}

fn print_finish_steps(config: &Settings) -> Result<(), String> {
    if std::env::consts::OS == "windows" {
        println!("{}", t!("wizard.windows.finish_steps.line_1"));
        println!("{}", t!("wizard.windows.finish_steps.line_2"));
        return Ok(());
    }
    println!("{}", t!("wizard.posix.finish_steps.line_1"));
    println!("{}", t!("wizard.posix.finish_steps.line_2"));
    println!("{}", t!("wizard.posix.finish_steps.line_3"));
    println!("============================================");
    println!("{}:", t!("wizard.posix.finish_steps.line_4"));
    for idf_version in config.idf_versions.clone().unwrap() {
        let paths = version_paths(config, &idf_version)?;
        println!(
            "       {} \"{}\"",
            t!("wizard.posix.finish_steps.line_5"),
            paths.activation_script.display()
        );
    }
    println!("============================================");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, install: &str) -> Tool {
        serde_json::from_value(serde_json::json!({
            "description": "",
            "export_paths": [],
            "export_vars": {},
            "info_url": "",
            "install": install,
            "name": name,
            "version_cmd": [],
            "version_regex": "",
            "versions": [],
        }))
        .unwrap()
    }

    fn selection(name: &str) -> ToolSelectionInfo {
        ToolSelectionInfo {
            name: name.to_string(),
            description: None,
            install: "on_request".to_string(),
            editable: true,
            supported_targets: None,
        }
    }

    fn names(tools: &[Tool]) -> Vec<&str> {
        tools.iter().map(|t| t.name.as_str()).collect()
    }

    #[test]
    fn merged_requirements_metadata_points_to_merged_file() {
        let dir = Path::new("idf").join("tools").join("requirements");
        let metadata = merged_requirements_metadata(&dir);
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.features.len(), 1);
        let feature = &metadata.features[0];
        assert_eq!(feature.name, "merged");
        assert!(!feature.optional);
        assert_eq!(
            feature.requirement_path,
            dir.join("requirements.merged.txt").to_string_lossy()
        );
    }

    #[test]
    fn force_always_install_marks_clang_and_ninja_only() {
        let mut tools = vec![
            tool("esp-clang", "on_request"),
            tool("ninja", "never"),
            tool("qemu-xtensa", "on_request"),
        ];
        force_always_install(&mut tools);
        let installs: Vec<&str> = tools.iter().map(|t| t.install.as_str()).collect();
        assert_eq!(installs, vec!["always", "always", "on_request"]);
    }

    #[test]
    fn contains_qemu_matches_substring() {
        assert!(contains_qemu(["cmake", "qemu-riscv32"].into_iter()));
        assert!(!contains_qemu(["cmake", "ninja"].into_iter()));
        assert!(!contains_qemu(std::iter::empty()));
    }

    #[test]
    fn record_selected_tools_skips_empty_selection() {
        let mut config = Settings {
            idf_tools_per_version: None,
            ..Settings::default()
        };
        record_selected_tools(&mut config, "v5.3", &[]);
        assert!(config.idf_tools_per_version.is_none());
    }

    #[test]
    fn record_selected_tools_creates_map_and_inserts() {
        let mut config = Settings {
            idf_tools_per_version: None,
            ..Settings::default()
        };
        record_selected_tools(&mut config, "v5.3", &[selection("a"), selection("b")]);
        let map = config.idf_tools_per_version.unwrap();
        assert_eq!(
            map.get("v5.3").unwrap(),
            &vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn filter_tools_without_map_uses_current_selection() {
        let mut tools = vec![
            tool("cmake", "always"),
            tool("gdb", "on_request"),
            tool("qemu", "on_request"),
        ];
        filter_tools_to_install(&mut tools, None, "v5.3", &[selection("gdb")]);
        assert_eq!(names(&tools), vec!["cmake", "gdb"]);
    }

    #[test]
    fn filter_tools_with_map_entry_uses_recorded_names() {
        let mut tools = vec![
            tool("cmake", "always"),
            tool("gdb", "on_request"),
            tool("qemu", "on_request"),
        ];
        let map = HashMap::from([("v5.3".to_string(), vec!["qemu".to_string()])]);
        filter_tools_to_install(&mut tools, Some(&map), "v5.3", &[selection("gdb")]);
        assert_eq!(names(&tools), vec!["cmake", "qemu"]);
    }

    #[test]
    fn filter_tools_with_map_missing_version_keeps_all() {
        let mut tools = vec![tool("cmake", "always"), tool("gdb", "on_request")];
        let map = HashMap::from([("v5.1".to_string(), vec![])]);
        filter_tools_to_install(&mut tools, Some(&map), "v5.3", &[]);
        assert_eq!(names(&tools), vec!["cmake", "gdb"]);
    }

    #[test]
    fn escape_export_paths_only_on_windows() {
        let paths = vec!["C:\\Program Files\\tool".to_string()];
        assert_eq!(escape_export_paths_for_os(paths.clone(), "linux"), paths);
        assert_eq!(
            escape_export_paths_for_os(paths, "windows"),
            vec!["C:\\Program` Files\\tool".to_string()]
        );
    }

    #[test]
    fn report_download_error_returns_inner_message() {
        assert_eq!(
            report_download_error(DownloadError::DownloadFailed("boom".to_string())),
            "boom"
        );
        assert_eq!(
            report_download_error(DownloadError::PathCreationFailed("nope".to_string())),
            "nope"
        );
        assert_eq!(
            report_download_error(DownloadError::UserCancelled),
            "User cancelled the operation"
        );
    }
}
