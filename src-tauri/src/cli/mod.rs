use std::path::PathBuf;

use clap::CommandFactory;
use cli_args::Cli;
use cli_args::Commands;

use cli_args::InstallArgs;
use config::ConfigError;
use helpers::generic_input;
use helpers::generic_select;
use idf_im_lib::get_log_directory;
use idf_im_lib::settings::Settings;
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

fn setup_logging(cli: &cli_args::Cli, non_interactive: bool) -> Result<(), config::ConfigError> {
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

pub async fn run_cli(cli: Cli) {
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
        return;
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
            setup_logging(&cli, false).unwrap();
        }
    }
    match command {
        Commands::Install(install_args) => {
            let settings = Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            );
            match settings {
                Ok(settings) => {
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("Wizard result: {:?}", r);
                            info!("Successfully installed IDF");
                            info!("Now you can start using IDF tools");
                        }
                        Err(err) => error!("Error: {}", err),
                    }
                }
                Err(err) => error!("Error: {}", err),
            }
        }
        Commands::List => {
            info!("Listing installed versions...");
            match idf_im_lib::version_manager::get_esp_ide_config() {
                Ok(config) => {
                    if config.idf_installed.len() == 0 {
                        warn!(
                            "No versions found. Use eim install to install a new ESP-IDF version."
                        );
                    } else {
                        println!("Installed versions:");
                        for version in config.idf_installed {
                            if version.id == config.idf_selected_id {
                                println!("- {} (selected)", version.name);
                            } else {
                                println!("- {}", version.name);
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("No versions found. Use eim install to install a new ESP-IDF version.");
                    debug!("Error: {}", err)
                }
            }
        }
        Commands::Select { version } => {
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.len() == 0 {
                            warn!("No versions installed");
                        } else {
                            println!("Available versions:");
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            match generic_select("Which version do you want to select?", &options) {
                                Ok(selected) => match select_idf_version(&selected) {
                                    Ok(_) => {
                                        println!("Selected version: {}", selected);
                                    }
                                    Err(err) => error!("Error: {}", err),
                                },
                                Err(err) => error!("Error: {}", err),
                            }
                        }
                    }
                    Err(err) => {
                        error!(
                            "No versions found. Use eim install to install a new ESP-IDF version."
                        );
                        debug!("Error: {}", err)
                    }
                }
            } else {
                match select_idf_version(&version.clone().unwrap()) {
                    Ok(_) => {
                        info!("Selected version: {}", version.clone().unwrap());
                    }
                    Err(err) => error!("Error: {}", err),
                }
            }
        }
        Commands::Rename { version, new_name } => {
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.len() == 0 {
                            warn!("No versions installed");
                        } else {
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            let version = helpers::generic_select(
                                "Which version do you want to rename?",
                                &options,
                            )
                            .unwrap(); // todo move to function and add error handling
                            let new_name = match generic_input(
                                "Enter new name:",
                                "you need to enter a new name",
                                "",
                            ) {
                                Ok(name) => {
                                    if name.len() == 0 {
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
                                }
                                Err(err) => error!("Error: {}", err),
                            }
                        }
                    }
                    Err(err) => {
                        debug!("Error: {}", err);
                        error!(
                            "No versions found. Use eim install to install a new ESP-IDF version."
                        )
                    }
                }
            } else if new_name.is_none() {
                let new_name =
                    match generic_input("Enter new name:", "you need to enter a new name", "") {
                        Ok(name) => {
                            if name.len() == 0 {
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
                    }
                    Err(err) => error!("Error: {}", err),
                }
            } else {
                match idf_im_lib::version_manager::rename_idf_version(
                    &version.clone().unwrap(),
                    new_name.clone().unwrap(),
                ) {
                    Ok(_) => {
                        println!("Version renamed.");
                    }
                    Err(err) => error!("Error: {}", err),
                }
            }
        }
        Commands::Discover => {
            // TODO:Implement version discovery
            unimplemented!("Version discovery not implemented yet");
            println!("Discovering available versions... (This can take couple of minutes)");
            let idf_dirs = idf_im_lib::version_manager::find_esp_idf_folders("/");
            for dir in idf_dirs {
                println!("Found IDF directory: {}", dir);
            }
        }
        Commands::Import { path } => match path {
            Some(config_file) => {
                println!("Importing using config file: {:?}", config_file);
                match idf_im_lib::utils::parse_tool_set_config(&config_file) {
                    Ok(_) => {
                        println!("Config file parsed. eim_idf.json updated.");
                    }
                    Err(err) => error!("Error: {}", err),
                }
            }
            None => {
                println!("No config file specified, nothing to import.");
            }
        },
        Commands::Remove { version } => {
            // todo: add spinner
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions() {
                    Ok(versions) => {
                        if versions.len() == 0 {
                            println!("No versions installed");
                        } else {
                            println!("Available versions:");
                            let options = versions.iter().map(|v| v.name.clone()).collect();
                            match generic_select("Which version do you want to remove?", &options) {
                                Ok(selected) => match remove_single_idf_version(&selected) {
                                    Ok(_) => {
                                        println!("Removed version: {}", selected);
                                    }
                                    Err(err) => error!("Error: {}", err),
                                },
                                Err(err) => error!("Error: {}", err),
                            }
                        }
                    }
                    Err(err) => error!("Error: {}", err),
                }
            } else {
                match remove_single_idf_version(&version.clone().unwrap()) {
                    Ok(_) => {
                        println!("Removed version: {}", version.clone().unwrap());
                    }
                    Err(err) => error!("Error: {}", err),
                }
            }
        }
        Commands::Purge => {
            // Todo: offer to run discovery first
            println!("Purging all known IDF installations...");
            match idf_im_lib::version_manager::list_installed_versions() {
                Ok(versions) => {
                    if versions.len() == 0 {
                        println!("No versions installed");
                    } else {
                        for version in versions {
                            println!("Removing version: {}", version.name);
                            match remove_single_idf_version(&version.name) {
                                Ok(_) => {
                                    println!("Removed version: {}", version.name);
                                }
                                Err(err) => error!("Error: {}", err),
                            }
                        }
                    }
                }
                Err(err) => error!("Error: {}", err),
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
                        }
                        Err(err) => error!("Error: {}", err),
                    }
                }
                Err(err) => error!("Error: {}", err),
            }
        }
        #[cfg(feature = "gui")]
        Commands::Gui(install_args) => {
            #[cfg(not(feature = "gui"))]
            unimplemented!("GUI not present in this type of build");
            gui::run()
        }
    }
}
