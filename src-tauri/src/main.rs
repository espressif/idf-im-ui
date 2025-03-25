// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "gui")]
pub mod gui;

#[cfg(feature = "cli")]
use clap::Parser;
#[cfg(feature = "cli")]
use config::ConfigError;
#[cfg(feature = "cli")]
use idf_im_lib::get_log_directory;
#[cfg(feature = "cli")]
use idf_im_lib::settings::Settings;
#[cfg(feature = "cli")]
use log::{debug, error, info, LevelFilter};
#[cfg(feature = "cli")]
use std::path::PathBuf;
#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "cli")]
rust_i18n::i18n!("locales", fallback = "en");

#[cfg(feature = "cli")]
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

#[cfg(feature = "cli")]
fn setup_logging(cli: &cli::cli_args::Cli) -> Result<(), config::ConfigError> {
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

    let console_log_level = match (cli.verbose, cli.non_interactive.unwrap_or(false)) {
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
    debug!("Logging initialized with console level: {:?}, file level: {:?}", console_log_level, file_log_level);
    debug!("Non-interactive mode: {}", cli.non_interactive.unwrap_or(false));
    debug!("Verbosity level: {}", cli.verbose);

    Ok(())
}

#[cfg(feature = "cli")]
fn set_locale(locale: &Option<String>) {
    match locale {
        Some(l) => {
            rust_i18n::set_locale(l);
            info!("Set locale to: {}", l);
        }
        None => debug!("No locale specified, defaulting to en"),
    }
}

#[cfg(all(target_os = "windows", feature = "cli", not(debug_assertions)))]
fn has_console() -> bool {
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_OUTPUT_HANDLE;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        !(handle == INVALID_HANDLE_VALUE || handle.is_null())
    }
}

#[cfg(all(target_os = "windows", feature = "cli"))]
fn attach_console() -> bool {
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS) != 0
    }
}

#[tokio::main]
async fn main() {
    let has_args = std::env::args().len() > 1;

    // Remove the windows_subsystem attribute if we're in CLI mode with arguments
    #[cfg(all(target_os = "windows", feature = "cli", not(debug_assertions)))]
    {
        if has_args {
            let has_existing_console = has_console();
            
            if !has_existing_console {
                // Try to attach to parent console
                let attached = attach_console();
                
                // If attachment failed, allocate a new console
                if !attached {
                    unsafe { winapi::um::consoleapi::AllocConsole(); }
                }
                
            }
        }
    }

    // Now we can safely use println
    println!("Starting EIM");
    if has_args {
        println!("Arguments detected: {:?}", std::env::args().collect::<Vec<_>>());
    }

    // Process in CLI mode if arguments are provided
    #[cfg(feature = "cli")]
    if has_args {
        println!("Starting CLI mode");
        let cli = cli::cli_args::Cli::parse();

        // #[cfg(not(feature = "gui"))]
        setup_logging(&cli).unwrap();
        set_locale(&cli.locale);

        let settings = Settings::new(cli.config.clone(), cli.into_iter());
        match settings {
            Ok(settings) => {
                let result = cli::wizard::run_wizzard_run(settings).await;
                match result {
                    Ok(r) => {
                        info!("Wizard result: {:?}", r);
                        println!("Successfully installed IDF");
                        println!("Now you can start using IDF tools");
                    }
                    Err(err) => {
                        error!("Error: {}", err);
                        eprintln!("Error: {}", err);
                    }
                }
            }
            Err(err) => {
                error!("Error: {}", err);
                eprintln!("Error: {}", err);
            }
        }
        
        // Exit after CLI processing to avoid starting GUI
        return;
    } else { // TODO: this is for running the wizard without arguments on CLI only build
        #[cfg(not(feature = "gui"))]
        {
            let cli = cli::cli_args::Cli::parse();
            setup_logging(&cli).unwrap();
            set_locale(&cli.locale);
            let settings = Settings::new(cli.config.clone(), cli.into_iter());
            match settings {
                Ok(settings) => {
                    let result = cli::wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("Wizard result: {:?}", r);
                            println!("Successfully installed IDF");
                            println!("Now you can start using IDF tools");
                        }
                        Err(err) => {
                            error!("Error: {}", err);
                            eprintln!("Error: {}", err);
                        }
                    }
                }
                Err(err) => {
                    error!("Error: {}", err);
                    eprintln!("Error: {}", err);
                }
            }
            return;
        }
    }

    // No arguments or CLI mode not active, start GUI
    #[cfg(feature = "gui")]
    {
        println!("Starting GUI mode");
        gui::run()
    }
    
    #[cfg(not(any(feature = "gui", feature = "cli")))]
    {
        eprintln!("Error: Neither GUI nor CLI features are enabled!");
    }
}