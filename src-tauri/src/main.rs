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

    // ... (rest of the logging setup code)
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build(log_file_name)
        .map_err(|e| ConfigError::Message(format!("Failed to build file appender: {}", e)))?;

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}\n")))
        .build();

    let log_level = match cli.verbose {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    let config = log4rs::Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(log4rs::filter::threshold::ThresholdFilter::new(
                    LevelFilter::Trace,
                )))
                .build("file", Box::new(logfile)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(log4rs::filter::threshold::ThresholdFilter::new(
                    log_level,
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

#[tokio::main]
async fn main() {
    #[cfg(feature = "cli")]
    {
        println!("CLI build!");
        let cli = cli::cli_args::Cli::parse();

        #[cfg(not(feature = "gui"))]
        setup_logging(&cli).unwrap();
        set_locale(&cli.locale);

        let settings = Settings::new(cli.config.clone(), cli.into_iter());
        // let settings = cli_args::Settings::new();
        match settings {
            Ok(settings) => {
                let result = cli::wizard::run_wizzard_run(settings).await;
                match result {
                    Ok(r) => {
                        info!("Wizard result: {:?}", r);
                        println!("Successfully installed IDF");
                        println!("Now you can start using IDF tools");
                    }
                    Err(err) => error!("Error: {}", err),
                }
            }
            Err(err) => error!("Error: {}", err),
        }
    }
    #[cfg(feature = "gui")]
    {
        gui::run()
    }
}
