use anyhow::anyhow;
use anyhow::Result;
use dialoguer::FolderSelect;
use idf_im_lib::idf_features::get_requirements_json_url;
use idf_im_lib::idf_features::RequirementsMetadata;
use idf_im_lib::idf_tools::Tool;
use idf_im_lib::idf_tools::ToolsFile;
use idf_im_lib::offline_installer::copy_idf_from_offline_archive;
use idf_im_lib::offline_installer::install_prerequisites_offline;
use idf_im_lib::offline_installer::use_offline_archive;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::copy_dir_contents;
use idf_im_lib::utils::extract_zst_archive;
use idf_im_lib::{ensure_path, DownloadProgress};
use idf_im_lib::git_tools::ProgressMessage;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{debug, error, info, warn};
use rust_i18n::t;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::{
    env,
    fmt::Write,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

// maybe move the default values to the config too?
const DEFAULT_TOOLS_DOWNLOAD_FOLDER: &str = "dist";
const DEFAULT_TOOLS_INSTALL_FOLDER: &str = "tools";
const DEFAULT_TOOLS_JSON_LOCATION: &str = "tools/tools.json";
const DEFAULT_IDF_TOOLS_PY_LOCATION: &str = "./tools/idf_tools.py";

use crate::cli::helpers::{
    create_progress_bar, create_theme, generic_confirm, generic_input, update_progress_bar_number,
};

use crate::cli::prompts::*;

fn add_to_shell_rc(content: &str) -> Result<(), String> {
    let shell = env::var("SHELL").unwrap_or_else(|_| String::from(""));
    let home = dirs::home_dir().unwrap();

    let rc_file = match shell.as_str() {
        "/bin/bash" => home.join(".bashrc"),
        "/bin/zsh" => home.join(".zshrc"),
        "/bin/fish" => home.join(".config/fish/config.fish"),
        _ => return Err(t!("wizard.shell.unsupported").to_string()),
    };

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(rc_file)
        .unwrap();

    match std::io::Write::write_all(&mut file, content.as_bytes()) {
        Ok(_) => info!("{}", t!("wizard.shellrc.update.success")),
        Err(err) => {
            error!("{}", t!("wizard.shellrc.update.error"));
            error!("Error: {:?}", err);
        }
    };

    Ok(())
}

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
                    let message = t!(
                        "wizard.debug.submodule.progress",
                        name = name,
                        progress = value
                    );
                    progress_bar.set_message(message);
                    progress_bar.set_position(value);
                }
                Ok(ProgressMessage::SubmoduleFinish(name)) => {
                    let message = format!("{}: {}", name, 100);
                    progress_bar.set_message(message);
                    progress_bar.finish();
                    info!("{}: {}", t!("wizard.idf.submodule_finish"), name);
                    progress_bar = create_progress_bar();
                }
                Err(_) => {
                    break;
                }
            }
        }
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
                Err(err) => {
                    error!("{}", t!("wizard.idf.progress_bar.error"));
                }
            }
            Ok(())
        }
        Err(err) => {
            if config.non_interactive == Some(true) {
                Ok(())
            } else {
                handle_download_error(err)
            }
        }
    }
}

fn setup_directory(
    wizard_all_questions: Option<bool>,
    base_path: &PathBuf,
    config_field: &mut Option<String>,
    prompt_key: &str,
    default_name: &str,
) -> Result<PathBuf, String> {
    let mut directory = base_path.clone();

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

fn validate_tools_json_file(tools_json_file: &Path, config: &mut Settings) -> String {
    if fs::metadata(tools_json_file).is_err() {
        warn!("{}", t!("wizard.tools_json.not_found"));
        let selected_file = FolderSelect::with_theme(&create_theme())
            .with_prompt(t!("wizard.tools_json.select.prompt"))
            .folder(tools_json_file.to_str().unwrap())
            .file(true)
            .interact()
            .unwrap();
        if fs::metadata(&selected_file).is_ok() {
            config.tools_json_file = Some(selected_file.to_string());
            selected_file
        } else {
            // TODO: implement the retry logic -> in interactive mode the user should not be able to proceed until the files is found
            panic!("{}", t!("wizard.tools_json.unreachable"));
        }
    } else {
        tools_json_file.to_str().unwrap().to_string()
    }
}

async fn download_and_extract_tools(
    config: &Settings,
    tools: &ToolsFile,
    download_dir: &PathBuf,
    install_dir: &PathBuf,
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
                info!("{}", t!("wizard.tool.download.success", filename = filename));
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
                    t!("wizard.tool.extract.success", filename = filename, dest = dest)
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

pub async fn run_wizzard_run(mut config: Settings) -> Result<(), String> {
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

    if offline_mode {
        // only setting up temp directory if offline mode is enabled
        let archive_dir = offline_archive_dir.as_ref().unwrap();
        config = match use_offline_archive(config, archive_dir) {
            Ok(updated_config) => updated_config,
            Err(err) => {
                error!("Failed to use offline archive: {}", err);
                return Err(err);
            }
        };
        // install prerequisites offline
        if std::env::consts::OS == "windows" {
            match install_prerequisites_offline(&archive_dir) {
                Ok(_) => {
                    info!("{}", t!("wizard.prerequisites.offline_install.success"));
                }
                Err(err) => {
                    return Err(t!(
                        "wizard.error.prerequisites_offline_install",
                        error = err.to_string()
                    )
                    .to_string());
                }
            }
        }
    }

    if config.skip_prerequisites_check.unwrap_or(false) {
        info!("{}", t!("wizard.prerequisites.skip_check"));
    } else {
        // Check prerequisites
        check_and_install_prerequisites(
            config.non_interactive.unwrap_or_default(),
            config.install_all_prerequisites.unwrap_or_default(),
        )?;
    }

    // Python sanity check
    check_and_install_python(
        config.non_interactive.unwrap_or_default(),
        config.install_all_prerequisites.unwrap_or_default(),
        config.python_version_override.clone(),
    )?;

    if offline_mode {
        let archive_dir = offline_archive_dir.as_ref().unwrap();
        // copy IDFs
        copy_idf_from_offline_archive(archive_dir, &config)?;
    }

    // select target & idf version
    config = select_targets_and_versions(config).await?;

    // mirrors select
    config = select_mirrors(config).await?;

    config = select_installation_path(config)?;

    // initialize the per-version map if not already set
    if config.idf_features_per_version.is_none() {
        config.idf_features_per_version = Some(HashMap::new());
    }
    // Multiple version starts here
    let mut using_existing_idf = false;
    for idf_version in config.idf_versions.clone().unwrap() {
        let paths = config.get_version_paths(&idf_version).map_err(|err| {
            error!("Failed to get version paths: {}", err);
            err.to_string()
        })?;
        using_existing_idf = paths.using_existing_idf;

        config.idf_path = Some(paths.idf_path.clone());
        idf_im_lib::add_path_to_path(paths.idf_path.to_str().unwrap());


        let features = if !offline_mode {
          let req_url = get_requirements_json_url(config.repo_stub.clone().as_deref(), &idf_version.to_string(), config.idf_mirror.clone().as_deref());

          let requirements_files = match RequirementsMetadata::from_url(&req_url) {
              Ok(files) => files,
              Err(err) => {
                  warn!("{}: {}. {}", t!("wizard.requirements.read_failure"), err, t!("wizard.features.selection_unavailable"));
                  return Err(err.to_string());
              }
          };

          // Check if we already have features for this version (from CLI arg or config file)
          let f = if let Some(existing) = config.get_features_for_version_if_set(&idf_version) {
              // Convert feature names back to FeatureInfo
              requirements_files.features
                  .iter()
                  .filter(|f| existing.contains(&f.name))
                  .cloned()
                  .collect()
          } else {
              // Interactive selection for this version
              select_features(
                  &requirements_files,
                  config.non_interactive.unwrap_or_default(),
                  true,
              )?
          };
          // Save to per-version map
          if let Some(ref mut per_version) = config.idf_features_per_version {
            per_version.insert(
              idf_version.clone(),
              f.iter().map(|f| f.name.clone()).collect(),
            );
          }
          f.iter().map(|f| f.name.clone()).collect::<Vec<String>>()
        } else {
          config.get_features_for_version_if_set(&idf_version).unwrap_or(vec![])
        };

        debug!(
            "{}: {}",
            t!("wizard.features.selected"),
            features
                .join(", ")
        );


        if !using_existing_idf {
            // download idf
            let download_config = DownloadConfig {
                idf_path: paths.idf_path.to_str().unwrap().to_string(),
                repo_stub: config.repo_stub.clone(),
                idf_version: idf_version.to_string(),
                idf_mirror: config.idf_mirror.clone(),
                recurse_submodules: config.recurse_submodules,
                non_interactive: config.non_interactive,
            };

            match download_idf(download_config) {
                Ok(_) => {
                    debug!("{}", t!("wizard.idf.success"));
                }
                Err(DownloadError::PathCreationFailed(err)) => {
                    error!("{} {:?}", t!("wizard.idf.path_creation_failure"), err);
                    return Err(err);
                }
                Err(DownloadError::DownloadFailed(err)) => {
                    error!("{} {:?}", t!("wizard.idf.failure"), err);
                    return Err(err);
                }
                Err(DownloadError::UserCancelled) => {
                    error!("{}", t!("wizard.idf.user_cancelled"));
                    return Err("User cancelled the operation".to_string());
                }
            }
        }
        // setup tool directories

        let tool_download_directory = setup_directory(
            config.wizard_all_questions,
            &paths.version_installation_path,
            &mut config.tool_download_folder_name,
            "wizard.tools.download.prompt",
            DEFAULT_TOOLS_DOWNLOAD_FOLDER,
        )?;

        if offline_mode {
            // copy content dist from offline_archive_dir
            copy_dir_contents(
                &offline_archive_dir.as_ref().unwrap().path().join("dist"),
                &tool_download_directory,
            )
            .map_err(|err| t!("wizard.error.copy_dist_directory", error = err.to_string()))?;
        }

        // Setup install directory
        let tool_install_directory = setup_directory(
            config.wizard_all_questions,
            &paths.version_installation_path,
            &mut config.tool_install_folder_name,
            "wizard.tools.install.prompt",
            DEFAULT_TOOLS_INSTALL_FOLDER,
        )?;

        idf_im_lib::add_path_to_path(tool_install_directory.to_str().unwrap());

        // tools_json_file

        let tools_json_file = get_tools_json_path(&mut config, &paths.idf_path);
        let validated_file = validate_tools_json_file(&tools_json_file, &mut config);

        debug!(
            "{}",
            t!(
                "wizard.debug.tools_json_file",
                path = tools_json_file.display()
            )
        );

        let tools = idf_im_lib::idf_tools::read_and_parse_tools_file(&validated_file)
            .map_err(|err| format!("{}: {}", t!("wizard.tools_json.unparsable"), err))?;

        if tools.tools.iter().find(|&x| x.name.contains("qemu")).is_some() {
            let qemu_prereqs = idf_im_lib::system_dependencies::check_qemu_prerequisites();
            match qemu_prereqs {
            Ok(prereqs) if !prereqs.is_empty() => {
                error!(
                    "{}: {:?}",
                    t!("wizard.qemu.prerequisites.missing"),
                    prereqs
                );
                return Err(t!("wizard.qemu.prerequisites.unmet").to_string());
            }
            Err(err) => {
                error!(
                    "{}: {}",
                    t!("wizard.qemu.prerequisites.check_error"),
                    err
                );
                return Err(t!("wizard.qemu.prerequisites.unmet").to_string());
            }
            Ok(_) => { /* All good, continue. */ }
        }
        }

        let installed_tools_list = match download_and_extract_tools(
            &config,
            &tools,
            &tool_download_directory,
            &tool_install_directory,
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

        match idf_im_lib::python_utils::install_python_env(
            &paths,
            &paths.actual_version,
            &tool_install_directory,
            true, //TODO: actually read from config
            &features,
            if offline_mode {
                Some(offline_archive_dir.as_ref().unwrap().path())
            } else {
                None
            },
            &config.pypi_mirror,
        )
        .await
        {
            Ok(_) => {
                info!("{}", t!("wizard.python.env_installed"));
            }
            Err(err) => {
                error!("Failed to install Python environment: {}", err);
                return Err(err.to_string());
            }
        };

        ensure_path(paths.python_venv_path.to_str().unwrap())
            .map_err(|err| t!("wizard.error.create_python_env", error = err.to_string()))?;

        let export_paths = idf_im_lib::idf_tools::get_tools_export_paths_from_list(
            tools,
            installed_tools_list,
            tool_install_directory.to_str().unwrap(),
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
        idf_im_lib::single_version_post_install(
            &paths.activation_script_path.to_str().unwrap(),
            paths.idf_path.to_str().unwrap(),
            &paths.actual_version,
            tool_install_directory.to_str().unwrap(),
            export_paths,
            paths.python_venv_path.to_str(),
            None, // env_vars
        )
    }
    save_config_if_desired(&config)?;
    let ide_conf_path_tmp = PathBuf::from(&config.esp_idf_json_path.clone().unwrap_or_default());
    debug!(
        "{}",
        t!(
            "wizard.debug.ide_config_path",
            path = ide_conf_path_tmp.display()
        )
    );
    match ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
        Ok(_) => (),
        Err(err) => {
            error!("Failed to create IDE configuration directory: {}", err);
            return Err(err.to_string());
        }
    }
    match config.save_esp_ide_json() {
        Ok(_) => debug!("{}", t!("wizard.debug.ide_config_saved")),
        Err(err) => {
            error!("Failed to save IDE configuration: {}", err);
            return Err(err.to_string());
        }
    };

    match std::env::consts::OS {
        "windows" => {
            println!("{}", t!("wizard.windows.finish_steps.line_1"));
            println!("{}", t!("wizard.windows.finish_steps.line_2"));
        }
        _ => {
            println!("{}", t!("wizard.posix.finish_steps.line_1"));
            println!("{}", t!("wizard.posix.finish_steps.line_2"));
            println!("{}", t!("wizard.posix.finish_steps.line_3"));
            println!("============================================");
            println!("{}:", t!("wizard.posix.finish_steps.line_4"));
            for idf_version in config.idf_versions.clone().unwrap() {
                let paths = config.get_version_paths(&idf_version).map_err(|err| {
                    error!("Failed to get version paths: {}", err);
                    err.to_string()
                })?;
                println!(
                    "       {} \"{}\"",
                    t!("wizard.posix.finish_steps.line_5"),
                    paths.activation_script.display()
                );
            }
            println!("============================================");
        }
    }
    Ok(())
}
