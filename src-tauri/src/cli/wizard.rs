use anyhow::anyhow;
use anyhow::Result;
use dialoguer::FolderSelect;
use idf_im_lib::command_executor;
use idf_im_lib::command_executor::execute_command;
use idf_im_lib::idf_tools::ToolsFile;
use idf_im_lib::settings::Settings;
use idf_im_lib::system_dependencies::add_to_path;
use idf_im_lib::system_dependencies::get_scoop_path;
use idf_im_lib::utils::copy_dir_contents;
use idf_im_lib::utils::extract_zst_archive;
use idf_im_lib::utils::is_valid_idf_directory;
use idf_im_lib::{ensure_path, DownloadProgress, ProgressMessage};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::{debug, error, info, warn};
use rust_i18n::t;
use serde::de;
use tempfile::TempDir;
use tera::Context;
use tera::Tera;
use std::collections::HashMap;
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
                info!("{} sucessfully downloaded", filename.to_string());
            }
        }
        DownloadProgress::Verified(url) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                info!("{} checksum verified", filename.to_string());
            }
        }
        DownloadProgress::Extracted(url, dest) => {
            if let Some(filename) = Path::new(&url).file_name().and_then(|f| f.to_str()) {
                info!("Successfully extracted {} to {}", filename.to_string(), dest);
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
    debug!("Config entering wizard: {:?}", config);

    // TODO: only needed for offline mode
    let offline_archive_dir = TempDir::new().expect("Failed to create temporary directory");
    let mut offline_mode = false;
    if config.use_local_archive.is_some() {
        debug!("Using local archive: {:?}", config.use_local_archive);
        if !config.use_local_archive.as_ref().unwrap().exists() {
            return Err(format!(
                "Local archive path does not exist: {}",
                config.use_local_archive.as_ref().unwrap().display()
            ));
        }
        offline_mode = true;
    }

    if offline_mode { // only setting up temp directory if offline mode is enabled

      debug!("Temporary directory created: {}", offline_archive_dir.path().display());
      match extract_zst_archive(&config.use_local_archive.as_ref().unwrap(), &offline_archive_dir.path()) {
        Ok(_) => {
            info!("Successfully extracted archive to: {:?}", offline_archive_dir);
        }
        Err(err) => {
            return Err(format!("Failed to extract archive: {}", err));
        }
      }
      // load the config from the extracted archive
      let config_path = offline_archive_dir.path().join("config.toml");
      if config_path.exists() {
        // debug!("Loading config from extracted archive: {}", config_path.display());
        let mut tmp_setting = Settings::default();
        match Settings::load(&mut tmp_setting, &config_path.to_str().unwrap()) {
          Ok(()) => {
            debug!("Config loaded from archive: {:?}", config_path.display());
            debug!("Config: {:?}", tmp_setting);
            debug!("Using only version for now.");
            config.idf_versions = tmp_setting.idf_versions;
        }
          Err(err) => {
            return Err(format!("Failed to load config from archive: {}", err));
          }
        }
      } else {
        warn!("Config file not found in archive: {}. Continuing with default config.", config_path.display());
      }
      // install prerequisites offline
      match std::env::consts::OS {
          "windows" => {
            // Setup Scoop
            let scoop_path = offline_archive_dir.path().join("scoop");
            let scoop_install_path = dirs::home_dir()
                        .unwrap()
                        .join("scoop");
            let scoop_command = scoop_install_path.join("shims").join("scoop.ps1");
            add_to_path(&scoop_install_path.to_str().unwrap());
            add_to_path(&scoop_install_path.join("shims").to_str().unwrap());
            match execute_command(
                "powershell",
                &[
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &scoop_path.join("install_scoop_offline.ps1").to_str().unwrap(),
                    "-OfflineDir",
                    &scoop_path.to_str().unwrap(),
                ],
            ) {
                Ok(out) => {
                  if out.status.success() {
                    info!("Scoop installed successfully.");

                    info!("Scoop install path: {}", scoop_install_path.display());
                    idf_im_lib::add_path_to_path(&scoop_install_path.to_str().unwrap());
                  } else {
                    warn!("Failed to install Scoop: {:?} | {:?}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                  }
                }
                Err(err) => {
                    return Err(format!("Failed to install Scoop: {}", err));
                }
            }
            add_to_path(&scoop_install_path.to_str().unwrap());
            add_to_path(&scoop_install_path.join("shims").to_str().unwrap());

            // TODO: disable auto-updates in scoop config.json
            setup_scoop_packages(&scoop_path, &scoop_command)?;

          }
          "linux" | "macos" => {
              info!("On POSIX system, we do not provide prerequisites. Please ensure you have the necessary tools installed.");
          }
          _ => {
              return Err(format!(
                  "Unsupported OS: {}",
                  std::env::consts::OS
              ));
          }
      }

      // copy IDFs
      for archive_version in config.clone().idf_versions.unwrap() {
        match std::env::consts::OS {

          "windows" => {
            // As on windows the IDF contains too long paths, we need to copy the content from the offline archive to the IDF path
            // using windows powershell command
            let mut main_command = "powershell";

            let test_for_pwsh = command_executor::execute_command("pwsh", &["--version"]);
            match test_for_pwsh {
                // this needs to be used in powershell 7
                Ok(_) => {
                    debug!("Found powershell core");
                    main_command = "pwsh";
                }
                Err(_) => {
                    debug!("Powershell core not found, using powershell");
                }
            }

            let output_cp = command_executor::execute_command(
                main_command,
                &vec![
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "cp",
                    "-r",
                    &offline_archive_dir.path().join(&archive_version).to_str().unwrap(),
                    &config.clone().path.unwrap().join(&archive_version).to_str().unwrap(),
                ],
            );
            match output_cp {
                Ok(out) => {
                    if out.status.success() {
                        info!("Successfully copied content from offline archive to IDF path");
                    } else {
                        return Err(format!("Failed to copy content from offline archive: {:?} | {:?}", out.stdout, out.stderr));
                    }
                }
                Err(err) => {
                    return Err(format!("Failed to copy content from offline archive: {}", err));
                }
            }

          },
          _ => {
            debug!("Copying IDF version: {}", archive_version);
            match copy_dir_contents(&offline_archive_dir.path().join(&archive_version), &config.clone().path.unwrap().join(&archive_version)) {
              Ok(_) => {
                  info!("Successfully copied content from offline archive to IDF path");
                  // config.path = Some(config.clone().path.unwrap().join("v5.5").join("esp-idf"));
              }
              Err(err) => {
                  return Err(format!("Failed to copy content from offline archive: {}", err));
              }
            }
          }
        }


      }

    }

    if config.skip_prerequisites_check.unwrap_or(false) {
        info!("Skipping prerequisites check as per user request.");
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
    )?;

    // select target & idf version
    config = select_targets_and_versions(config).await?;

    // mirrors select
    config = select_mirrors(config)?;

    config = select_installation_path(config)?;

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
            &offline_archive_dir.path().join("dist"),
            &tool_download_directory,
          )
          .map_err(|err| format!("Failed to copy dist directory from offline archive: {}", err))?;
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

        debug!("Tools json file: {}", tools_json_file.display());

        let tools = idf_im_lib::idf_tools::read_and_parse_tools_file(&validated_file)
            .map_err(|err| format!("{}: {}", t!("wizard.tools_json.unparsable"), err))?;

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
            &paths.actual_version,
            &tool_install_directory,
            true, //TODO: actually read from config
            &paths.idf_path,
            &config.idf_features.clone().unwrap_or_default(),
            if offline_mode {
                Some(offline_archive_dir.path())
            } else {
                None
            },
        )
        .await
        {
            Ok(_) => {
                info!("Python environment installed");
            }
            Err(err) => {
                error!("Failed to install Python environment: {}", err);
                return Err(err.to_string());
            }
        };

        ensure_path(paths.python_venv_path.to_str().unwrap())
            .map_err(|err| format!("Failed to create Python environment directory: {}", err))?;

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
    debug!("IDE configuration path: {}", ide_conf_path_tmp.display());
    match ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
        Ok(_) => (),
        Err(err) => {
            error!("Failed to create IDE configuration directory: {}", err);
            return Err(err.to_string());
        }
    }
    match config.save_esp_ide_json() {
        Ok(_) => debug!("IDE configuration saved."),
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

// Structure to define package information with compile-time template content
struct ScoopPackage {
    name: &'static str,
    template_content: &'static str,
    manifest_filename: &'static str,
    test_command: &'static str,
}

// Helper function to create and write a manifest
fn create_manifest(
    package: &ScoopPackage,
    context: &Context,
    scoop_path: &Path,
    tera: &mut Tera,
) -> Result<PathBuf, String> {
    // Add template to Tera
    let template_name = format!("{}_manifest", package.name);
    if let Err(e) = tera.add_raw_template(&template_name, package.template_content) {
        error!("Failed to add {} template: {}", package.name, e);
        return Err(format!("Failed to add {} template", package.name));
    }

    // Render the template
    let mut rendered_manifest = match tera.render(&template_name, context) {
        Err(e) => {
            error!("Failed to render {} template: {}", package.name, e);
            return Err(format!("Failed to render {} template", package.name));
        }
        Ok(text) => text,
    };

    // Normalize line endings
    rendered_manifest = rendered_manifest.replace("\r\n", "\n").replace("\n", "\r\n");

    // Write manifest to file
    let manifest_path = scoop_path.join(package.manifest_filename);
    if let Err(e) = fs::write(&manifest_path, rendered_manifest) {
        error!("Failed to write {} manifest: {}", package.name, e);
        return Err(format!("Failed to write {} manifest", package.name));
    }

    info!("{} manifest written to: {}", package.name, manifest_path.display());
    Ok(manifest_path)
}

// Helper function to install a single package with retry logic
fn install_package_with_scoop(
    main_command: &str,
    scoop_command: &Path,
    manifest_path: &Path,
    package: &ScoopPackage,
    path_with_scoop: &str,
    max_retries: u32,
) -> Result<(), String> {
    for attempt in 1..=max_retries {
        info!("Installing {} (attempt {}/{})", package.name, attempt, max_retries);

        // Build the complete PowerShell command as a single string to avoid shell spawning
        // let full_command = &format!("Start-Process -FilePath '{}' -ArgumentList 'install', '--no-update-scoop', '{}' -Wait -NoNewWindow", scoop_command.to_str().unwrap(), manifest_path.to_str().unwrap());
        let full_command = format!(
            "scoop install --no-update-scoop '{}'",
            // scoop_command.to_str().unwrap(),
            manifest_path.to_str().unwrap()
        );

        // Use the executor directly to ensure proper process handling
        let executor = command_executor::get_executor();
        let output = executor.execute_with_env(
            main_command,
            &vec![
                "-NoProfile",
                "-NonInteractive",
                "-NoLogo",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &full_command,
            ],
            vec![("PATH", &add_to_path(path_with_scoop).unwrap())],
        );

        match output {
            Ok(out) => {
                if out.status.success() {
                    info!("{} installation completed.", package.name);

                    // Test if the package is working correctly
                    if test_package_installation(package.test_command, main_command, path_with_scoop) {
                        info!("{} installed and tested successfully.", package.name);
                        return Ok(());
                    } else {
                        warn!("{} installed but test command '{}' failed on attempt {}",
                              package.name, package.test_command, attempt);

                        if attempt == max_retries {
                            return Err(format!(
                                "Failed to install {}: package installed but test command '{}' failed after {} attempts",
                                package.name, package.test_command, max_retries
                            ));
                        }
                        // Continue to next retry attempt
                    }
                } else {
                    warn!("Installation failed for {} on attempt {}: {:?} | {:?}",
                          package.name, attempt,
                          String::from_utf8_lossy(&out.stdout),
                          String::from_utf8_lossy(&out.stderr));

                    if attempt == max_retries {
                        return Err(format!(
                            "Failed to install {} after {} attempts: {:?} | {:?}",
                            package.name, max_retries,
                            String::from_utf8_lossy(&out.stdout),
                            String::from_utf8_lossy(&out.stderr)
                        ));
                    }
                    // Continue to next retry attempt
                }
            }
            Err(err) => {
                warn!("Command execution failed for {} on attempt {}: {}", package.name, attempt, err);

                if attempt == max_retries {
                    return Err(format!("Failed to install {} after {} attempts: {}", package.name, max_retries, err));
                }
                // Continue to next retry attempt
            }
        }

        // Add a small delay between retries
        if attempt < max_retries {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    unreachable!()
}

// Helper function to test if a package is working
fn test_package_installation(test_command: &str, main_command: &str, path_with_scoop: &str) -> bool {
    if test_command == "echo 0" {
        return true; // Skip test for packages that don't have a meaningful test
    }

    // Add a small delay to allow file system operations to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Try the test command multiple times with short delays
    for attempt in 1..=3 {
        let executor = command_executor::get_executor();
        let test_result = executor.execute_with_env(
            main_command,
            &vec![
                "-NoProfile",
                "-NonInteractive",
                "-NoLogo",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                test_command
            ],
            vec![("PATH", path_with_scoop)],
        );

        match test_result {
            Ok(output) if output.status.success() => return true,
            Ok(_) => {
                debug!("Test command '{}' failed on attempt {}", test_command, attempt);
            }
            Err(e) => {
                debug!("Test command '{}' error on attempt {}: {}", test_command, attempt, e);
            }
        }

        // Short delay between test attempts
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    false
}

// Main function that creates manifests and installs packages
fn setup_scoop_packages(scoop_path: &Path, scoop_command: &Path) -> Result<(), String> {
    // Create Tera context
    let mut context = Context::new();
    context.insert("offline_archive_scoop_dir", &scoop_path.to_str().unwrap().replace("\\", "/"));

    // Define packages to install with compile-time template content
    let packages = [
        ScoopPackage {
            name: "7zip",
            template_content: include_str!("../../scoop_manifest_templates/7zip.json"),
            manifest_filename: "7zip.json",
            test_command: "echo 0"
        },
        ScoopPackage {
            name: "git",
            template_content: include_str!("../../scoop_manifest_templates/git.json"),
            manifest_filename: "git.json",
            test_command: "git --version",
        },
        ScoopPackage {
            name: "dark",
            template_content: include_str!("../../scoop_manifest_templates/dark.json"),
            manifest_filename: "dark.json",
            test_command: "echo 0"
        },
        ScoopPackage {
            name: "python",
            template_content: include_str!("../../scoop_manifest_templates/python310.json"),
            manifest_filename: "python.json",
            test_command: "python3 --version",
        },
    ];

    // Create all manifests
    let mut tera = Tera::default();
    let mut manifest_paths = Vec::new();

    for package in &packages {
        let manifest_path = create_manifest(package, &context, scoop_path, &mut tera)?;
        manifest_paths.push((manifest_path, package.name));
    }

    // Get Scoop path and determine PowerShell command
    let path_with_scoop = get_scoop_path()
        .ok_or_else(|| "Could not get scoop path".to_string())?;

    let main_command = match command_executor::execute_command("pwsh", &["--version"]) {
        Ok(_) => {
            debug!("Found powershell core");
            "pwsh"
        }
        Err(_) => {
            debug!("Powershell core not found, using powershell");
            "powershell"
        }
    };

    // Install all packages with retry logic
    const MAX_RETRIES: u32 = 3;
    for (manifest_path, package_name) in manifest_paths {
        // Find the package struct to get the test command
        let package = packages.iter()
            .find(|p| p.name == package_name)
            .ok_or_else(|| format!("Package {} not found in package definitions", package_name))?;

        install_package_with_scoop(
            main_command,
            scoop_command,
            &manifest_path,
            package,
            &path_with_scoop,
            MAX_RETRIES,
        )?;
    }

    Ok(())
}
