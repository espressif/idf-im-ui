use std::path::Path;
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
use idf_im_lib::idf_config::IdfConfig;
use idf_im_lib::settings::Settings;
use idf_im_lib::utils::is_valid_idf_directory;
use idf_im_lib::utils::find_by_name_and_extension;
use idf_im_lib::utils::parse_esp_idf_json;
use idf_im_lib::utils::EspIdfConfig;
use idf_im_lib::version_manager::remove_single_idf_version;
use idf_im_lib::version_manager::select_idf_version;
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
    // let cli = cli_args::Cli::parse();
    #[cfg(feature = "gui")]
    let command = cli
        .clone()
        .command
        .unwrap_or(Commands::Gui(InstallArgs::default()));
    #[cfg(not(feature = "gui"))]
    if cli.clone().command.is_none() {
        Cli::command()
            .print_help()
            .expect("No command specified, use --help to see available commands");
        return Ok(());
    }
    #[cfg(not(feature = "gui"))]
    let command = cli.clone().command.unwrap();
    match command {
        #[cfg(feature = "gui")]
        Commands::Gui(_) => {
            #[cfg(not(feature = "gui"))]
            unimplemented!("GUI not present in this type of build");
            println!("Running GUI...");
        }
        _ => {
            setup_logging(&cli, false).context("Failed to setup logging")?;
        }
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
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("Wizard result: {:?}", r);
                            info!("Successfully installed IDF");
                            info!("Now you can start using IDF tools");
                            Ok(())
                        }
                        Err(err) => Err(anyhow::anyhow!(err))
                    }
                }
                Err(err) => Err(anyhow::anyhow!(err))
            }
        }
        Commands::List => {
            info!("Listing installed versions...");
            match idf_im_lib::version_manager::get_esp_ide_config() {
                Ok(config) => {
                    if config.idf_installed.is_empty() {
                        warn!(
                            "No versions found. Use eim install to install a new ESP-IDF version."
                        );
                        Ok(())
                    } else {
                        println!("Installed versions:");
                        for version in config.idf_installed {
                            if version.id == config.idf_selected_id {
                                println!("- {} (selected) [{}]", version.name, version.path);
                            } else {
                                println!("- {} [{}]", version.name, version.path);
                            }
                        }
                        Ok(())
                    }
                }
                Err(err) => {
                    info!("No versions found. Use eim install to install a new ESP-IDF version.");
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
                            warn!("No versions installed");
                            Ok(())
                        } else {
                            println!("Available versions:");
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            match generic_select("Which version do you want to select?", &options) {
                                Ok(selected) => match select_idf_version(&selected) {
                                    Ok(_) => {
                                        println!("Selected version: {}", selected);
                                        Ok(())
                                    }
                                    Err(err) => Err(anyhow::anyhow!(err)),
                                },
                                Err(err) => Err(anyhow::anyhow!(err)),
                            }
                        }
                    }
                    Err(err) => {
                        error!(
                            "No versions found. Use eim install to install a new ESP-IDF version."
                        );
                        debug!("Error: {}", err);
                        Err(anyhow::anyhow!(err))
                    }
                }
            } else {
                match select_idf_version(&version.clone().unwrap()) {
                    Ok(_) => {
                        info!("Selected version: {}", version.clone().unwrap());
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
                            warn!("No versions installed");
                            Ok(())
                        } else {
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            let version = match helpers::generic_select(
                                "Which version do you want to rename?",
                                &options,
                            ) {
                                Ok(selected) => selected,
                                Err(err) => {
                                    error!("Error: {}", err);
                                    return Err(anyhow::anyhow!(err));
                                }
                            };

                            let new_name = match generic_input(
                                "Enter new name:",
                                "you need to enter a new name",
                                "",
                            ) {
                                Ok(name) => {
                                    if name.is_empty() {
                                        warn!("No name provided, using default!");
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
                                    println!("Version renamed.");
                                    Ok(())
                                }
                                Err(err) => Err(anyhow::anyhow!(err)),
                            }
                        }
                    }
                    Err(err) => {
                        debug!("Error: {}", err);
                        error!(
                            "No versions found. Use eim install to install a new ESP-IDF version."
                        );
                        Err(anyhow::anyhow!(err))
                    }
                }
            } else if new_name.is_none() {
                let new_name =
                    match generic_input("Enter new name:", "you need to enter a new name", "") {
                        Ok(name) => {
                            if name.is_empty() {
                                warn!("No name provided, using default!");
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
                        println!("Version renamed.");
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
                        println!("Version renamed.");
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
        }
        Commands::Discover {path  } => {
            info!("Discovering available versions... (This can take couple of minutes)");
            let path = path.unwrap_or_else(|| {
                let default_path = match std::env::consts::OS {
                    "windows" => {
                        "C:\\".to_string()
                    }
                    _ => {
                        "/".to_string()
                    }
                };


                debug!("No path provided, using default: {}", default_path);
                default_path
            });
            // first parse existing esp_idf.json (using parse_esp_idf_json) || previous VSCode installations
            info!("Searching for esp_idf.json files...");
            let search_patch = Path::new(&path);
            let esp_idf_json_path = find_by_name_and_extension(
              search_patch,
              "esp_idf",
              "json",
            );
            if esp_idf_json_path.is_empty() {
                info!("No esp_idf.json found");
            } else {
                info!("Found {} esp_idf.json files:", esp_idf_json_path.len());
            }
            for path in esp_idf_json_path {
                info!("- {} ", &path);
              match std::env::consts::OS {
                "windows" => {
                    // On Windows, we need to fix every installation from VSCode
                    info!("Parsing esp_idf.json at: {}", path);
                    let idf_json_path = Path::new(&path);
                    let json_str = std::fs::read_to_string(idf_json_path).unwrap();
                    let config: EspIdfConfig = match serde_json::from_str(&json_str) {
                        Ok(config) => config,
                        Err(e) => {
                          error!("Failed to parse config file: {}", e);
                          continue;
                        }
                    };
                    for (_key, value) in config.idf_installed {
                        let idf_path = value.path;
                        fix_command(Some(idf_path)).await?;
                    }
                }
                _ => {
                    match parse_esp_idf_json(&path) {
                          Ok(_) => {
                              info!("Parsed config: {:?}", path);
                          }
                          Err(err) => {
                              info!("Failed to parse esp_idf.json: {}", err);
                          }
                      }
                    }
                  }
            }
            // second try to find tool_set_config.json (using parse_tool_set_config) || previous Eclipse installations
            info!("Searching for tool_set_config.json files...");
            let tool_set_config_path = find_by_name_and_extension(
                search_patch,
                "tool_set_config",
                "json",
            );
            if tool_set_config_path.is_empty() {
                info!("No tool_set_config.json found");
            } else {
                info!("Found {} tool_set_config.json files:", tool_set_config_path.len());
            }
            for path in tool_set_config_path {
                info!("- {} ", &path);
                match idf_im_lib::utils::parse_tool_set_config(&path) {
                    Ok(_) => {
                        info!("Parsed config: {:?}", path);
                    }
                    Err(err) => {
                        info!("Failed to parse tool_set_config.json: {}", err);
                    }
                }
            }
            // third try to find IDF directories (using find_esp_idf_folders) || previous instalation from cli
            info!("Searching for any other IDF directories...");
            let idf_dirs = idf_im_lib::version_manager::find_esp_idf_folders(&path);
            if idf_dirs.is_empty() {
                info!("No IDF directories found");
            } else {
                info!("Found {} IDF directories:", idf_dirs.len());
            }
            let config = match idf_im_lib::version_manager::get_esp_ide_config() {
              Ok(config) => {
                if config.idf_installed.is_empty() {
                  debug!(
                      "No versions found. Every discovered version can be imported."
                  );
                }
                config
              }
              Err(_err) => {
                debug!("No ide config found. New will be created.");
                IdfConfig::default()
              }
            };
            let mut paths_to_add = vec![];
            for dir in idf_dirs {
              info!("- {} ", &dir);
              if config.clone().is_path_in_config(dir.clone()) {
                info!("Already present!");
              } else {
                info!("Can be added...");
                paths_to_add.push(dir);
              }
            }
            if paths_to_add.is_empty() {
                info!("No new IDF directories found to add.");
                return Ok(());
            } else {
                info!("Found {} new IDF directories available to add:", paths_to_add.len());
                info!("You can add them using `eim install` command with the `--path` option.");
            }
            Ok(())
        }
        Commands::Import { path } => match path {
            Some(config_file) => {
                info!("Importing using config file: {:?}", config_file);
                match idf_im_lib::utils::parse_tool_set_config(&config_file) {
                    Ok(_) => {
                        info!("Config file parsed. eim_idf.json updated.");
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
            None => {
                info!("No config file specified, nothing to import.");
                Ok(())
            }
        },
        Commands::Remove { version } => {
            // todo: add spinner
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.is_empty() {
                            info!("No versions installed");
                            Ok(())
                        } else {
                            println!("Available versions:");
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            match generic_select("Which version do you want to remove?", &options) {
                                Ok(selected) => match remove_single_idf_version(&selected, false) {
                                    Ok(_) => {
                                        info!("Removed version: {}", selected);
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
                        println!("Removed version: {}", version.clone().unwrap());
                        Ok(())
                    }
                    Err(err) => Err(anyhow::anyhow!(err)),
                }
            }
        }
        Commands::Purge => {
            // Todo: offer to run discovery first
            println!("Purging all known IDF installations...");
            match idf_im_lib::version_manager::list_installed_versions() {
                Ok(versions) => {
                    if versions.is_empty() {
                        println!("No versions installed");
                        Ok(())
                    } else {
                        let mut failed = false;
                        for version in versions {
                            info!("Removing version: {}", version.name);
                            match remove_single_idf_version(&version.name, false) {
                                Ok(_) => {
                                    info!("Removed version: {}", version.name);
                                }
                                Err(err) => {
                                  error!("Failed to remove version {}: {}", version.name, err);
                                  failed = true;
                                }
                            }
                        }
                        if failed {
                            return Err(anyhow::anyhow!("Some versions failed to remove. Check logs for details."));
                        } else {
                            info!("All versions removed successfully.");
                        }
                        Ok(())
                    }
                }
                Err(err) => Err(anyhow::anyhow!(err)),
            }
        }
        Commands::Wizard(install_args) => {
            info!("Running IDF Installer Wizard...");
            let settings = Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            );
            match settings {
                Ok(mut settings) => {
                    settings.non_interactive = Some(false);
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("Wizard result: {:?}", r);
                            info!("Successfully installed IDF");
                            info!("Now you can start using IDF tools");
                            Ok(())
                        }
                        Err(err) => Err(anyhow::anyhow!(err)),
                    }
                }
                Err(err) => Err(anyhow::anyhow!(err)),
            }
        }
        Commands::Fix { path } => {
          fix_command(path).await
        }
        #[cfg(feature = "gui")]
        Commands::Gui(_install_args) => {
            #[cfg(not(feature = "gui"))]
            unimplemented!("GUI not present in this type of build");
            gui::run();
            Ok(())
        }
    }
}

async fn fix_command(path:Option<String>) -> anyhow::Result<()> {
  let path_to_fix = if path.is_some() {
    // If a path is provided, fix the IDF installation at that path
    let path = path.unwrap();
      if is_valid_idf_directory(&path) {
      PathBuf::from(path)
      } else {
      error!("Invalid IDF directory: {}", path);
      return Err(anyhow::anyhow!("Invalid IDF directory: {}", path));
      }
  } else {
    match idf_im_lib::version_manager::list_installed_versions() {
      Ok(versions) => {
        if versions.is_empty() {
            warn!("No versions installed");
            return Ok(());
        } else {
          let options = versions.iter().map(|v| v.path.clone()).collect();
          let version_path = match helpers::generic_select(
              "Which version do you want to fix?",
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
          return Err(anyhow::anyhow!("No versions found. Use eim install to install a new ESP-IDF version."));
        }
    }
  };
  info!("Fixing IDF installation at path: {}", path_to_fix.display());
  // The fix logic is just instalation with use of existing repository
  let mut version_name = None;
  match idf_im_lib::version_manager::list_installed_versions() {
    Ok(versions) => {
      for v in versions {
        if v.path == path_to_fix.to_str().unwrap() {
          info!("Found existing IDF version: {}", v.name);
          // Remove the existing activation script and eim_idf.json entry
          match remove_single_idf_version(&v.name, true) {
            Ok(_) => {
              info!("Removed existing IDF version from eim_idf.json: {}", v.name);
              version_name = Some(v.name.clone());
            }
            Err(err) => {
              error!("Failed to remove existing IDF version {}: {}", v.name, err);
            }
          }
        }
      }
    }
    Err(_) => {
      info!("Failed to list installed versions. Using default naming.");
    }
  }

  let mut settings = Settings::default();
  settings.path = Some(path_to_fix.clone());
  settings.non_interactive = Some(true);
  settings.version_name = version_name;
  settings.install_all_prerequisites = Some(true);
  settings.config_file_save_path = None; // Do not save config file in fix mode
  let result = wizard::run_wizzard_run(settings).await;
  match result {
    Ok(r) => {
      info!("Fix result: {:?}", r);
      info!("Successfully fixed IDF installation at {}", path_to_fix.display());
    }
    Err(err) => {
      error!("Failed to fix IDF installation: {}", err);
      return Err(anyhow::anyhow!(err));
    }
  }
  info!("Now you can start using IDF tools");
  Ok(())
}
