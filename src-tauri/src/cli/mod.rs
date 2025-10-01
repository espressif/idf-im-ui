use std::path::PathBuf;

use anyhow::Context;
use cli_args::Cli;
use cli_args::Commands;
use clap::CommandFactory;
use cli_args::InstallArgs;
use config::ConfigError;
use helpers::generic_input;
use helpers::generic_select;
use idf_im_lib::get_log_directory;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::is_valid_idf_directory;
use idf_im_lib::version_manager::prepare_settings_for_fix_idf_installation;
use idf_im_lib::version_manager::remove_single_idf_version;
use idf_im_lib::version_manager::select_idf_version;
use idf_im_lib::telemetry::track_event;
use log::debug;
use log::error;
use log::info;
use log::warn;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use serde_json::json;
use rust_i18n::t;

use crate::cli::helpers::track_cli_event;
#[cfg(feature = "gui")]
use crate::gui;

pub mod cli_args;
pub mod helpers;
pub mod prompts;
pub mod wizard;

fn setup_logging(cli: &cli_args::Cli, non_interactive: bool) -> anyhow::Result<()> {
    let log_file_name = cli.log_file.clone().map_or_else(
        || {
            get_log_directory()
                .map(|dir| dir.join("eim.log"))
                .unwrap_or_else(|| {
                    eprintln!("Failed to get log directory, using default eim.log");
                    PathBuf::from("eim.log")
                })
        },
        PathBuf::from,
    );

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build(log_file_name)
        .map_err(|e| ConfigError::Message(format!("Failed to build file appender: {}", e)))?;

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build();

    let console_log_level = match (cli.verbose, non_interactive) {
        (0, false) => LevelFilter::Info,
        (0, true) => LevelFilter::Debug, // At least Debug level for non-interactive mode
        (1, _) => LevelFilter::Debug,
        (_, _) => LevelFilter::Trace,
    };

    let file_log_level = LevelFilter::Trace;

    let config = log4rs::Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(log4rs::filter::threshold::ThresholdFilter::new(
                    file_log_level,
                )))
                .build("file", Box::new(logfile)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(log4rs::filter::threshold::ThresholdFilter::new(
                    console_log_level,
                )))
                .build("stdout", Box::new(stdout)),
        )
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Trace),
        )
        .map_err(|e| ConfigError::Message(format!("Failed to build log4rs config: {}", e)))?;

    log4rs::init_config(config)
        .map_err(|e| ConfigError::Message(format!("Failed to initialize logger: {}", e)))?;

    // Log the configuration to verify settings
    debug!(
        "Logging initialized with console level: {:?}, file level: {:?}",
        console_log_level, file_log_level
    );
    debug!("Non-interactive mode: {}", non_interactive);
    debug!("Verbosity level: {}", cli.verbose);

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
        }
        _ => {
            setup_logging(&cli, false).context("Failed to setup logging")?;
        }
    }
    if !do_not_track {
        track_cli_event("CLI started", Some(json!({
          "command": format!("{:?}", command)
        }))).await;
    }
    match command {
        Commands::Install(install_args) => {
            let settings = Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            );
            match settings {
                Ok(mut settings) => {
                  if install_args.install_all_prerequisites.is_none() { // if cli argument is not set
                    settings.install_all_prerequisites = Some(true); // The non-interactive install will always install all prerequisites
                  }
                  let time = std::time::SystemTime::now();
                  if !do_not_track {
                      track_cli_event("CLI installation started", Some(json!({
                        "versions": format!("{:?}", settings.idf_versions),
                      }))).await;
                  }
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("{}", t!("install.wizard_result"));
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
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
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
            gui::run();
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
