use anyhow::anyhow;
use anyhow::Result;
use dialoguer::FolderSelect;
use idf_im_lib::idf_tools::ToolsFile;
use idf_im_lib::settings::Settings;
use idf_im_lib::{ensure_path, DownloadProgress, ProgressMessage};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{debug, error, info, warn};
use rust_i18n::t;
use std::sync::mpsc;
use std::thread;
use std::{
    env,
    fmt::Write,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

// maybe move the default values to the config too?
const DEFAULT_TOOLS_DOWNLOAD_FOLDER: &str = "dist";
const DEFAULT_TOOLS_INSTALL_FOLDER: &str = "tools";
const DEFAULT_TOOLS_JSON_LOCATION: &str = "tools/tools.json";
const DEFAULT_IDF_TOOLS_PY_LOCATION: &str = "./tools/idf_tools.py";

use crate::cli::helpers::{
    create_progress_bar, create_theme, generic_confirm, generic_input, update_progress_bar_number,
};

use crate::cli::prompts::*;

async fn download_tools(
    tools_file: ToolsFile,
    selected_chip: Vec<String>,
    destination_path: &str,
    mirror: Option<&str>,
) -> Result<Vec<String>> {
    let tool_name_list: Vec<String> = tools_file
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    info!(
        "{}: {:?}",
        t!("wizard.tools_download.progress"),
        tool_name_list
    );
    let list = idf_im_lib::idf_tools::filter_tools_by_target(tools_file.tools, &selected_chip);

    let platform = match idf_im_lib::idf_tools::get_platform_identification(None) {
        Ok(platform) => platform,
        Err(err) => {
            if std::env::consts::OS == "windows" {
                // All this is for cases when on windows microsoft store creates "pseudolinks" for python
                let scp = idf_im_lib::system_dependencies::get_scoop_path();
                let usable_python = match scp {
                    Some(path) => {
                        let mut python_path = PathBuf::from(path);
                        python_path.push("python3.exe");
                        python_path.to_str().unwrap().to_string()
                    }
                    None => "python3.exe".to_string(),
                };
                match idf_im_lib::idf_tools::get_platform_identification(Some(&usable_python)) {
                    Ok(platform) => platform,
                    Err(err) => {
                        error!("Unable to identify platform: {}", err);
                        panic!("{}.  {:?}", t!("wizard.tools_platform_error"), err);
                    }
                }
            } else {
                panic!("{}.  {:?}", t!("wizard.tools_platform_error"), err);
            }
        }
    };
    debug!("Python platform: {}", platform);
    let download_links = idf_im_lib::idf_tools::change_links_donwanload_mirror(
        idf_im_lib::idf_tools::get_download_link_by_platform(list, &platform),
        // Some("https://dl.espressif.com/github_assets"), // this switches mirror, should be parametrized
        mirror,
    )
    .into_iter()
    .collect::<Vec<_>>();
    let mut downloaded_tools: Vec<String> = vec![];
    for (tool_name, download_link) in download_links.iter() {
        info!("{}: {}", t!("wizard.tool_download.progress"), tool_name);

        let (progress_tx, progress_rx) = mpsc::channel();

        let progress_bar = ProgressBar::new(download_link.size);
        progress_bar.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})").unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));

        debug!("Download link: {}", download_link.url);
        debug!("destination: {}", destination_path);

        let file_path = Path::new(&download_link.url);
        let filename: &str = file_path.file_name().unwrap().to_str().unwrap();

        let full_file_path = Path::new(&destination_path).join(Path::new(filename));
        match idf_im_lib::verify_file_checksum(
            &download_link.sha256,
            full_file_path.to_str().unwrap(),
        ) {
            Ok(true) => {
                downloaded_tools.push(filename.to_string()); // add it to the list for extraction even if it's already downloaded
                info!("{}", t!("wizard.tool_file.present"));
                progress_bar.finish();
                continue;
            }
            _ => {
                debug!("{}", t!("wizard.tool_file.missing"));
            }
        }
        let tn = tool_name.clone();
        let pb = progress_bar.clone();
        let progress_handle = {
            thread::spawn(move || {
                while let Ok(progress_msg) = progress_rx.recv() {
                    match progress_msg {
                        DownloadProgress::Progress(current, _total) => {
                            pb.set_position(current);
                        }
                        DownloadProgress::Complete => {
                            pb.finish();
                            break;
                        }
                        DownloadProgress::Error(err) => {
                            log::error!("Error downloading {}: {}", tn, err);
                            break;
                        }
                    }
                }
            })
        };

        match idf_im_lib::download_file(&download_link.url, destination_path, Some(progress_tx)).await {
            Ok(_) => {
                downloaded_tools.push(filename.to_string());
                progress_bar.finish();
                info!("{} {}", t!("wizard.tool.downloaded"), tool_name);
            }
            Err(err) => {
                error!("{}: {}", t!("wizard.tool.download_failed"), tool_name);
                error!("Error: {:?}", err);
                return Err(anyhow!("{}: {}", t!("wizard.tool.download_failed"), err));
            }
        }
        progress_handle.join().map_err(|e| anyhow::anyhow!("Progress reporting thread panicked: {:?}", e))?;
        match idf_im_lib::verify_file_checksum(
            &download_link.sha256,
            full_file_path.to_str().unwrap(),
        ) {
            Ok(true) => {
                continue;
            }
            _ => {
                error!("{}", t!("wizard.tool.corupted"));
                match fs::remove_file(&full_file_path) {
                    Ok(_) => {
                        error!(
                            "{}: {}",
                            t!("wizard.tool.removed"),
                            full_file_path.to_str().unwrap()
                        );
                    }
                    Err(err) => {
                        error!(
                            "{}: {}: {}",
                            t!("wizard.tool.remove_failed"),
                            full_file_path.to_str().unwrap(),
                            err
                        );
                    }
                };
            }
        }
    }
    Ok(downloaded_tools)
}

fn extract_tools(tools: Vec<String>, source_path: &str, destination_path: &str) -> anyhow::Result<()> {
  let mut failed = false;
    for tool in tools.iter() {
        let mut archive_path = PathBuf::from(source_path);
        archive_path.push(tool);
        let out = idf_im_lib::decompress_archive(archive_path.to_str().unwrap(), destination_path);
        match out {
            Ok(_) => {
                info!("{}: {}", t!("wizard.tool.extracted"), tool);
            }
            Err(err) => {
                error!("{:?}", err);
                error!("{}: {}", t!("wizard.tool.extract_failed"), tool);
                failed = true;
            }
        }
    }
    if failed {
        Err(anyhow::anyhow!("Some tools failed to extract"))
    } else {
        Ok(())
    }
}

fn add_to_shell_rc(content: &str) -> Result<(), String> {
    let shell = env::var("SHELL").unwrap_or_else(|_| String::from(""));
    let home = dirs::home_dir().unwrap();

    let rc_file = match shell.as_str() {
        "/bin/bash" => home.join(".bashrc"),
        "/bin/zsh" => home.join(".zshrc"),
        "/bin/fish" => home.join(".config/fish/config.fish"),
        _ => return Err("Unsupported shell".to_string()),
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
    debug!("Selected target: {:?}", target);

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
    debug!("Selected idf version: {:?}", idf_versions);

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

fn handle_download_error(err: git2::Error) -> Result<(), DownloadError> {
    match err.code() {
        git2::ErrorCode::Exists => match generic_confirm("wizard.idf_path_exists.prompt") {
            Ok(true) => Ok(()),
            Ok(false) => Err(DownloadError::UserCancelled),
            Err(e) => Err(DownloadError::DownloadFailed(e.to_string())),
        },
        _ => Err(DownloadError::DownloadFailed(err.to_string())),
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
                    let message = format!("{}: {}", name, value);
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

    info!("Cloning ESP-IDF");

    match idf_im_lib::get_esp_idf(
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
) -> anyhow::Result<()> {
    let downloaded_tools_list = match download_tools(
        tools.clone(),
        config.target.clone().unwrap(),
        download_dir.to_str().unwrap(),
        config.mirror.as_deref(),
    )
    .await {
        Ok(list) => list,
        Err(err) => {
            error!("Failed to download tools: {}", err);
            return Err(anyhow!(err));
        }
    };

    extract_tools(
        downloaded_tools_list,
        download_dir.to_str().unwrap(),
        install_dir.to_str().unwrap(),
    )
}

fn get_and_validate_idf_tools_path(
    config: &mut Settings,
    idf_path: &PathBuf,
) -> Result<PathBuf, String> {
    let mut idf_tools_path = idf_path.clone();

    if let Some(file) = config.idf_tools_path.clone() {
        idf_tools_path.push(&file);
    } else if config.wizard_all_questions.unwrap_or(false) {
        let name = generic_input(
            "wizard.idf_tools.prompt",
            "wizard.idf_tools.prompt.failure",
            DEFAULT_IDF_TOOLS_PY_LOCATION,
        )?;

        idf_tools_path.push(&name);
        config.idf_tools_path = Some(name);
    } else {
        idf_tools_path.push(DEFAULT_IDF_TOOLS_PY_LOCATION); // TODO: defaults are in lib now
        config.idf_tools_path = Some(DEFAULT_IDF_TOOLS_PY_LOCATION.to_string());
    }

    if fs::metadata(&idf_tools_path).is_err() {
        warn!("{}", t!("wizard.idf_tools.not_found"));
        let idf_tools_py_select = FolderSelect::with_theme(&create_theme())
            .with_prompt(t!("wizard.idf_tools.select.prompt"))
            .folder(idf_path.to_str().unwrap())
            .file(true)
            .interact()
            .map_err(|e| format!("Failed to select: {}", e))?;

        if fs::metadata(&idf_tools_py_select).is_ok() {
            idf_tools_path = PathBuf::from(&idf_tools_py_select);
            config.idf_tools_path = Some(idf_tools_py_select);
        } else {
            return Err(t!("wizard.idf_tools.unreachable").to_string());
        }
    }

    Ok(idf_tools_path)
}

pub async fn run_wizzard_run(mut config: Settings) -> Result<(), String> {
    debug!("Config entering wizard: {:?}", config);

    // Check prerequisites
    check_and_install_prerequisites(
        config.non_interactive.unwrap_or_default(),
        config.install_all_prerequisites.unwrap_or_default(),
    )?;

    // Python sanity check
    check_and_install_python(
        config.non_interactive.unwrap_or_default(),
        config.install_all_prerequisites.unwrap_or_default(),
    )?;

    // select target & idf version
    config = select_targets_and_versions(config).await?;

    // mirrors select
    config = select_mirrors(config)?;

    config = select_installation_path(config)?;

    // Multiple version starts here

    for idf_version in config.idf_versions.clone().unwrap() {
        let mut version_instalation_path = config.path.clone().unwrap();
        version_instalation_path = idf_im_lib::expand_tilde(version_instalation_path.as_path());
        version_instalation_path.push(&idf_version);
        let mut idf_path = version_instalation_path.clone();
        idf_path.push("esp-idf");
        config.idf_path = Some(idf_path.clone()); // todo: list all of the paths
        idf_im_lib::add_path_to_path(idf_path.to_str().unwrap());

        // download idf
        let download_config = DownloadConfig {
            idf_path: idf_path.to_str().unwrap().to_string(),
            repo_stub: config.repo_stub.clone(),
            idf_version: idf_version.to_string(),
            idf_mirror: config.idf_mirror.clone(),
            recurse_submodules: config.recurse_submodules,
            non_interactive: config.non_interactive,
        };

        match download_idf(download_config) {
            Ok(_) => {
                debug!("{}", t!("wizard.idf.sucess"));
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
        // setup tool directories

        let tool_download_directory = setup_directory(
            config.wizard_all_questions,
            &version_instalation_path,
            &mut config.tool_download_folder_name,
            "wizard.tools.download.prompt",
            DEFAULT_TOOLS_DOWNLOAD_FOLDER,
        )?;

        // Setup install directory
        let tool_install_directory = setup_directory(
            config.wizard_all_questions,
            &version_instalation_path,
            &mut config.tool_install_folder_name,
            "wizard.tools.install.prompt",
            DEFAULT_TOOLS_INSTALL_FOLDER,
        )?;

        idf_im_lib::add_path_to_path(tool_install_directory.to_str().unwrap());

        // tools_json_file

        let tools_json_file = get_tools_json_path(&mut config, &idf_path);
        let validated_file = validate_tools_json_file(&tools_json_file, &mut config);

        debug!("Tools json file: {}", tools_json_file.display());

        let tools = idf_im_lib::idf_tools::read_and_parse_tools_file(&validated_file)
            .map_err(|err| format!("{}: {}", t!("wizard.tools_json.unparsable"), err))?;

        match download_and_extract_tools(
            &config,
            &tools,
            &tool_download_directory,
            &tool_install_directory,
        )
        .await {
            Ok(_) => {
                info!("{}: {}", t!("wizard.tools.downloaded"), tools_json_file.display());
            }
            Err(err) => {
                error!("Failed to download and extract tools: {}", err);
                return Err(err.to_string());
            }
        }
        match idf_im_lib::python_utils::install_python_env(
          &idf_version,
          &tool_install_directory,
          true, //TODO: actually read from config
          &idf_path,
          &config.idf_features.clone().unwrap_or_default(),
        ).await {
            Ok(_) => {
                info!("Python environment installed");
            }
            Err(err) => {
                error!("Failed to install Python environment: {}", err);
                return Err(err.to_string());
            }
        };
        let idf_python_env_path = tool_install_directory.join("python").join(&idf_version).join("venv"); //todo: move to config
        ensure_path(idf_python_env_path.to_str().unwrap())
            .map_err(|err| format!("Failed to create Python environment directory: {}", err))?;

        let export_paths = idf_im_lib::idf_tools::get_tools_export_paths(
            tools,
            config.target.clone().unwrap().clone(),
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
            version_instalation_path.to_str().unwrap(),
            idf_path.to_str().unwrap(),
            &idf_version,
            tool_install_directory.to_str().unwrap(),
            export_paths,
            idf_python_env_path.to_str(),
        )
    }
    save_config_if_desired(&config)?;
    let ide_conf_path_tmp = PathBuf::from(&config.esp_idf_json_path.clone().unwrap_or_default());
    debug!("IDE configuration path: {}", ide_conf_path_tmp.display());
    match ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
        Ok(_) => (),
        Err(err) => {
            error!("Failed to create IDE configuration directory: {}", err);
            return Err(err.to_string());
        }
    }
    // TODO: stop using useless param
    let ide_conf_path = ide_conf_path_tmp.join("esp_ide.json");
    match config.save_esp_ide_json(ide_conf_path.to_str().unwrap()) {
        Ok(_) => debug!("IDE configuration saved to: {}", ide_conf_path.display()),
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
            println!(
                "{:?}",
                config.path.clone().unwrap().to_str().unwrap().to_string()
            );
            println!("{}", t!("wizard.posix.finish_steps.line_3"));
            println!("============================================");
            println!("{}:", t!("wizard.posix.finish_steps.line_4"));
            for idf_version in config.idf_versions.clone().unwrap() {
                println!(
                    "       {} \"{}/activate_idf_{}.sh\"",
                    t!("wizard.posix.finish_steps.line_5"),
                    config.path.clone().unwrap().to_str().unwrap(),
                    idf_version,
                );
            }
            println!("============================================");
        }
    }
    Ok(())
}
