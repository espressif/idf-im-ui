use std::path::PathBuf;

use anyhow::Context;
use cli_args::Cli;
use cli_args::Commands;
use clap::CommandFactory;
use clap_complete::generate;
use cli_args::InstallArgs;
use fern::Dispatch;
use helpers::generic_input;
use helpers::generic_select;
use idf_im_lib::get_log_directory;
use idf_im_lib::logging::formatter;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::is_valid_idf_directory;
use idf_im_lib::version_manager::get_selected_version;
use idf_im_lib::version_manager::prepare_settings_for_fix_idf_installation;
use idf_im_lib::version_manager::remove_single_idf_version;
use idf_im_lib::version_manager::run_command_in_context;
use idf_im_lib::version_manager::select_idf_version;
use idf_im_lib::logging;
use log::debug;
use log::error;
use log::info;
use log::warn;
use log::LevelFilter;
use semver::Op;
use serde_json::json;
use rust_i18n::t;

use crate::cli::helpers::track_cli_event;
#[cfg(feature = "gui")]
use crate::gui;

pub mod cli_args;
pub mod helpers;
pub mod prompts;
pub mod wizard;

/// Setup logging for the CLI application.
///
/// # Arguments
/// * `verbose` - Verbosity level (0=Info, 1=Debug, 2+=Trace)
/// * `non_interactive` - Whether running in non-interactive mode
/// * `custom_log_path` - Optional custom path for the log file
///
/// # Log Level Behavior
/// | verbose | non_interactive | Console Level | File Level |
/// |---------|-----------------|---------------|------------|
/// | 0       | false           | Info          | Trace      |
/// | 0       | true            | Debug         | Trace      |
/// | 1       | *               | Debug         | Trace      |
/// | 2+      | *               | Trace         | Trace      |
pub fn setup_cli(
    verbose: u8,
    non_interactive: bool,
    custom_log_path: Option<PathBuf>,
) -> Result<(), fern::InitError> {
    // Console level based on verbosity and mode
    let console_level = match (verbose, non_interactive) {
        (0, false) => LevelFilter::Info,
        (0, true) => LevelFilter::Debug,   // Non-interactive needs Debug minimum
        (1, _) => LevelFilter::Debug,
        (_, _) => LevelFilter::Trace,
    };

    // File level is always Trace for maximum detail
    let file_level = LevelFilter::Trace;

    // Determine log file path
    let log_file_path = custom_log_path.unwrap_or_else(|| {
        get_log_directory()
            .map(|dir| dir.join("eim.log"))
            .unwrap_or_else(|| PathBuf::from("eim.log"))
    });

    // Build dispatch with file chain first (Trace level)
    // Then add console chain with configurable level
    // Module filters are applied globally
    Dispatch::new()
        .format(formatter)
        // Apply file at Trace level
        .chain(
            Dispatch::new()
                .level(file_level)
                .chain(fern::log_file(&log_file_path)?)
        )
        // Apply console at configurable level
        .chain(
            Dispatch::new()
                .level(console_level)
                .chain(std::io::stdout())
        )
        .apply()?;

    log::trace!("CLI logging initialized. Console: {:?}, File: {:?}", console_level, file_level);
    Ok(())
}

pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
  let do_not_track = cli.do_not_track;
    // Initial tracking of CLI start
    #[cfg(feature = "gui")]
    let command = cli
        .clone()
        .command
        .unwrap_or(Commands::Gui(InstallArgs::default()));
    #[cfg(not(feature = "gui"))]
    if cli.clone().command.is_none() {
        Cli::command()
            .print_help()
            .expect(&t!("cli.no_command"));
        return Ok(());
    }
    #[cfg(not(feature = "gui"))]
    let command = cli.clone().command.unwrap();
    match command {
        #[cfg(feature = "gui")]
        Commands::Gui(_) => {
            #[cfg(not(feature = "gui"))]
            unimplemented!("GUI not present in this type of build");
            println!("{}", t!("gui.running"));
            // Skip CLI logging setup - tauri-plugin-log handles GUI logging
        }
        _ => {
            setup_cli(cli.verbose, false, cli.log_file.map(PathBuf::from))
                .context("Failed to setup logging")?;
        }
    }
    if !do_not_track {
        track_cli_event("CLI started", Some(json!({
          "command": format!("{:?}", command)
        }))).await;
    }
    match command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = env!("CARGO_PKG_NAME");
            generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
            return Ok(());
        }
        Commands::Install(install_args) => {
            let settings = Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            );
            info!("Returned settings: {:?}", settings);
            match settings {
                Ok(mut settings) => {
                  debug!("Settings before adjustments: {:?}", settings);
                  if install_args.install_all_prerequisites.is_none() { // if cli argument is not set
                    settings.install_all_prerequisites = Some(true); // The non-interactive install will always install all prerequisites
                  }
                  debug!("Settings after adjustments: {:?}", settings);
                  let time = std::time::SystemTime::now();
                  if !do_not_track {
                      track_cli_event("CLI installation started", Some(json!({
                        "versions": format!("{:?}", settings.idf_versions),
                      }))).await;
                  }
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("{}", t!("install.wizard_result", r = "Ok".to_string()));
                            info!("{}", t!("install.success"));
                            info!("{}", t!("install.ready"));
                            if !do_not_track {
                              track_cli_event("CLI installation finished", Some(json!({
                                "duration": format!("{:?}", time.elapsed().unwrap_or_default()),
                              }))).await;
                            }
                            Ok(())
                        }
                        Err(err) => {
                          if !do_not_track {
                            track_cli_event("CLI installation failed", Some(json!({
                              "duration": format!("{:?}", time.elapsed().unwrap_or_default()),
                              "error": format!("{:?}", err),
                            }))).await;
                          }
                            Err(anyhow::anyhow!(err))
                        }
                    }
                }
                Err(err) => Err(anyhow::anyhow!(err))
            }
        }
        Commands::List => {
            info!("{}", t!("list.title"));
            match idf_im_lib::version_manager::get_esp_ide_config() {
                Ok(config) => {
                    if config.idf_installed.is_empty() {
                        warn!("{}", t!("list.no_versions"));
                        Ok(())
                    } else {
                        println!("{}", t!("list.installed_title"));
                        for version in config.idf_installed {
                            if version.id == config.idf_selected_id {
                                println!("{}", t!("list.version_selected", name = version.name, path = version.path));
                            } else {
                                println!("{}", t!("list.version", name = version.name, path = version.path));
                            }
                        }
                        Ok(())
                    }
                }
                Err(err) => {
                    info!("{}", t!("list.no_versions"));
                    debug!("Error: {}", err);
                    Ok(())
                }
            }
        }
        Commands::Select { version } => {
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.is_empty() {
                            warn!("{}", t!("select.no_versions"));
                            Ok(())
                        } else {
                            println!("{}", t!("select.available_title"));
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            match generic_select(&t!("select.prompt"), &options) {
                                Ok(selected) => match select_idf_version(&selected) {
                                    Ok(_) => {
                                        println!("{}", t!("select.success", version = selected));
                                        if let Some(selected) = get_selected_version() {
                                          println!("{}", t!("wizard.separator.line"));
                                          println!("{}", t!("cli.select.activation_instructions"));
                                          println!("source {}",selected.activation_script );
                                          println!("{}", t!("wizard.separator.line"));
                                        } else {
                                          warn!("{}", t!("select.unable_to_get_selected"));
                                        }
                                        Ok(())
                                    }
                                    Err(err) => Err(anyhow::anyhow!(err)),
                                },
                                Err(err) => Err(anyhow::anyhow!(err)),
                            }
                        }
                    }
                    Err(err) => {
                        error!("{}", t!("list.no_versions"));
                        debug!("Error: {}", err);
                        Err(anyhow::anyhow!(err))
                    }
                }
            } else {
                match select_idf_version(&version.clone().unwrap()) {
                    Ok(_) => {
                        info!("{}", t!("select.success", version = version.clone().unwrap()));
                        if let Some(selected) = get_selected_version() {
                          info!("{}", t!("wizard.separator.line"));
                          info!("{}", t!("cli.select.activation_instructions"));
                          info!("source {}",selected.activation_script );
                          info!("{}", t!("wizard.separator.line"));
                        } else {
                          warn!("{}", t!("select.unable_to_get_selected"));
                        }
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
        }
        Commands::Run { command, idf } => {
            let idf_identifier = if let Some(idf_str) = idf {
                idf_str
            } else if let Some(selected) = get_selected_version() {
                info!("{}", t!("run.using_selected", idf = selected.name));
                selected.id
            } else {
                return Err(anyhow::anyhow!(t!("run.no_idf_specified_no_selected")));
            };

            match run_command_in_context(&idf_identifier, &command) {
                Ok(output) => {
                    if output.status.success() {
                        println!("{}", String::from_utf8_lossy(&output.stdout));
                    } else {
                        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                    }
                    Ok(())
                }
                Err(err) => Err(err),
            }
        }
        Commands::Rename { version, new_name } => {
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.is_empty() {
                            warn!("{}", t!("rename.no_versions"));
                            Ok(())
                        } else {
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            let version = match helpers::generic_select(
                                &t!("rename.prompt"),
                                &options,
                            ) {
                                Ok(selected) => selected,
                                Err(err) => {
                                    error!("Error: {}", err);
                                    return Err(anyhow::anyhow!(err));
                                }
                            };

                            let new_name = match generic_input(
                                &t!("rename.new_name_prompt"),
                                &t!("rename.new_name_required"),
                                "",
                            ) {
                                Ok(name) => {
                                    if name.is_empty() {
                                        warn!("{}", t!("rename.using_default"));
                                        version.clone()
                                    } else {
                                        name
                                    }
                                }
                                Err(err) => {
                                    error!("Error: {}", err);
                                    version.clone()
                                }
                            };
                            match idf_im_lib::version_manager::rename_idf_version(
                                &version, new_name,
                            ) {
                                Ok(_) => {
                                    println!("{}", t!("rename.success"));
                                    Ok(())
                                }
                                Err(err) => Err(anyhow::anyhow!(err)),
                            }
                        }
                    }
                    Err(err) => {
                        debug!("Error: {}", err);
                        error!("{}", t!("list.no_versions"));
                        Err(anyhow::anyhow!(err))
                    }
                }
            } else if new_name.is_none() {
                let new_name =
                    match generic_input(&t!("rename.new_name_prompt"), &t!("rename.new_name_required"), "") {
                        Ok(name) => {
                            if name.is_empty() {
                                warn!("{}", t!("rename.using_default"));
                                version.clone().unwrap()
                            } else {
                                name
                            }
                        }
                        Err(err) => {
                            error!("Error: {}", err);
                            version.clone().unwrap()
                        }
                    };
                match idf_im_lib::version_manager::rename_idf_version(
                    &version.clone().unwrap(),
                    new_name,
                ) {
                    Ok(_) => {
                        println!("{}", t!("rename.success"));
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            } else {
                match idf_im_lib::version_manager::rename_idf_version(
                    &version.clone().unwrap(),
                    new_name.clone().unwrap(),
                ) {
                    Ok(_) => {
                        println!("{}", t!("rename.success"));
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
        }
        Commands::Discover => {
            // TODO:Implement version discovery
            unimplemented!("Version discovery not implemented yet");
            println!("{}", t!("discover.title"));
            let idf_dirs = idf_im_lib::version_manager::find_esp_idf_folders("/");
            for dir in idf_dirs {
                println!("{}", t!("discover.found", dir = dir));
            }
            Ok(())
        }
        Commands::Import { path } => match path {
            Some(config_file) => {
                info!("{}", t!("import.using_config", config = format!("{:?}", config_file)));
                match idf_im_lib::utils::parse_tool_set_config(&config_file) {
                    Ok(_) => {
                        info!("{}", t!("import.success"));
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
            None => {
                info!("{}", t!("import.no_config"));
                Ok(())
            }
        },
        Commands::Remove { version } => {
            // todo: add spinner
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.is_empty() {
                            info!("{}", t!("remove.no_versions"));
                            Ok(())
                        } else {
                            println!("{}", t!("remove.available_title"));
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            match generic_select(&t!("remove.prompt"), &options) {
                                Ok(selected) => match remove_single_idf_version(&selected, false) {
                                    Ok(_) => {
                                        info!("{}", t!("remove.success", version = selected));
                                        Ok(())
                                    }
                                    Err(err) => Err(anyhow::anyhow!(err)),
                                },
                                Err(err) => Err(anyhow::anyhow!(err)),
                            }
                        }
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            } else {
                match remove_single_idf_version(&version.clone().unwrap(), false) {
                    Ok(_) => {
                        println!("{}", t!("remove.success", version = version.clone().unwrap()));
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
        }
        Commands::Purge => {
            // Todo: offer to run discovery first
            println!("{}", t!("purge.title"));
            match idf_im_lib::version_manager::list_installed_versions() {
                Ok(versions) => {
                    if versions.is_empty() {
                        println!("{}", t!("purge.no_versions"));
                        Ok(())
                    } else {
                        let mut failed = false;
                        for version in versions {
                            info!("{}", t!("purge.removing", version = version.name));
                            match remove_single_idf_version(&version.name, false) {
                                Ok(_) => {
                                    info!("{}", t!("purge.removed", version = version.name));
                                }
                                Err(err) => {
                                  error!("{}", t!("purge.failed", version = version.name, error = err));
                                  failed = true;
                                }
                            }
                        }
                        if failed {
                            return Err(anyhow::anyhow!(t!("purge.some_failed")));
                        } else {
                            info!("{}", t!("purge.all_success"));
                        }
                        Ok(())
                    }
                }
                Err(err) => Err(anyhow::anyhow!(err)),
            }
        }
        Commands::Wizard(install_args) => {
            info!("{}", t!("wizard.title"));
            let settings = Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            );
            match settings {
                Ok(mut settings) => {
                    settings.non_interactive = Some(false);
                    let time = std::time::SystemTime::now();
                    if !do_not_track {
                      track_cli_event("CLI wizard started", Some(json!({}))).await;
                    }
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("{}", t!("install.wizard_result"));
                            info!("{}", t!("install.success"));
                            info!("{}", t!("install.ready"));
                            if !do_not_track {
                              track_cli_event("CLI wizard finished", Some(json!({
                                "duration": format!("{:?}", time.elapsed().unwrap_or_default()),
                              }))).await;
                            }
                            Ok(())
                        }
                        Err(err) => {
                          if !do_not_track {
                            track_cli_event("CLI wizard failed", Some(json!({
                              "duration": format!("{:?}", time.elapsed().unwrap_or_default()),
                              "error": format!("{:?}", err),
                            }))).await;
                          }
                          Err(anyhow::anyhow!(err))
                        }
                    }
                }
                Err(err) => Err(anyhow::anyhow!(err)),
            }
        }
        Commands::Fix { path } => {
          let path_to_fix = if path.is_some() {
              // If a path is provided, fix the IDF installation at that path
              let path = path.unwrap();
               if is_valid_idf_directory(&path) {
                PathBuf::from(path)
               } else {
                error!("{}", t!("fix.invalid_directory", path = path));
                return Err(anyhow::anyhow!(t!("fix.invalid_directory", path = path)));
               }
            } else {
              match idf_im_lib::version_manager::list_installed_versions() {
                Ok(versions) => {
                  if versions.is_empty() {
                      warn!("{}", t!("fix.no_versions"));
                      return Ok(());
                  } else {
                    let options = versions.iter().map(|v| v.path.clone()).collect();
                    let version_path = match helpers::generic_select(
                        &t!("fix.prompt"),
                        &options,
                    ) {
                        Ok(selected) => selected,
                        Err(err) => {
                            error!("Error: {}", err);
                            return Err(anyhow::anyhow!(err));
                        }
                    };
                    PathBuf::from(version_path)
                  }
                }
                Err(err) => {
                  debug!("Error: {}", err);
                  return Err(anyhow::anyhow!(t!("fix.no_versions_found")));
                }
            }
          };
          info!("{}", t!("fix.fixing", path = path_to_fix.display()));
          // The fix logic is just instalation with use of existing repository
          let settings = prepare_settings_for_fix_idf_installation(path_to_fix.clone()).await?;
          let result = wizard::run_wizzard_run(settings).await;
          match result {
            Ok(r) => {
              info!("{}", t!("fix.result"));
              info!("{}", t!("fix.success", path = path_to_fix.display()));
            }
            Err(err) => {
              error!("{}", t!("fix.failed", error = err));
              return Err(anyhow::anyhow!(err));
            }
          }
          info!("{}", t!("fix.ready"));
          Ok(())
        }
        #[cfg(feature = "gui")]
        Commands::Gui(_install_args) => {
            #[cfg(not(feature = "gui"))]
            unimplemented!("GUI not present in this type of build");
            let log_level = match cli.verbose {
                0 => LevelFilter::Info,
                1 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            };
            gui::run(Some(log_level));
            Ok(())
        }
        Commands::InstallDrivers => {
          match std::env::consts::OS {
            "windows" => {

              info!("{}", t!("drivers.installing"));
              if let Err(err) = idf_im_lib::install_drivers().await {
                error!("{}", t!("drivers.failed", error = err));
                return Err(anyhow::anyhow!(err));
              }
              info!("{}", t!("drivers.success"));
              Ok(())
            }
            _ => {
              return Err(anyhow::anyhow!(t!("drivers.windows_only")));
            }
          }
        }
    }
}
