#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "gui")]
pub mod gui;

#[cfg(feature = "cli")]
use clap::Parser;
#[cfg(feature = "cli")]
use config::ConfigError;
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
    let startup_time = std::time::SystemTime::now();
    let log_dir = get_log_directory().unwrap_or_else(|| PathBuf::from("."));
    let log_path = log_dir.join("eim_startup.log");
    
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = writeln!(file, "Application starting at {:?}", startup_time);
        let _ = writeln!(file, "Arguments: {:?}", std::env::args().collect::<Vec<_>>());
        let _ = writeln!(file, "OS: {}", std::env::consts::OS);
    }

    let has_args = std::env::args().len() > 1;
    
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    {
        if has_args {
            setup_windows_console(true);
        } else {
            // Ensure we don't have a console in GUI mode
            setup_windows_console(false);
        }
    }
    
    // Mac-specific PATH settings
    #[cfg(target_os = "macos")]
    {
        std::env::set_var("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/local/bin:/opt/local/sbin");
    }
    
    // Process as CLI mode if arguments are provided
    if has_args {
        #[cfg(feature = "cli")]
        {
            println!("Starting in CLI mode");
            process_cli_mode().await;
            // Exit immediately to prevent GUI from starting
            return;
        }
        
        #[cfg(not(feature = "cli"))]
        {
            eprintln!("CLI mode is not available in this build");
            // Continue to GUI mode as fallback
        }
    }
    
    // Start GUI mode (if no CLI args or CLI processing is complete)
    #[cfg(feature = "gui")]
    {
        println!("Starting in GUI mode");
        match std::panic::catch_unwind(|| {
            gui::run();
        }) {
            Ok(_) => println!("GUI exited normally"),
            Err(e) => {
                eprintln!("GUI crashed. Check logs for details.");
                
                // Log GUI crash details
                if let Some(log_dir) = get_log_directory() {
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let crash_log = log_dir.join(format!("gui_crash_{}.log", timestamp));
                    
                    let message = match e.downcast_ref::<String>() {
                        Some(s) => format!("GUI crashed with message: {}", s),
                        None => match e.downcast_ref::<&str>() {
                            Some(s) => format!("GUI crashed with message: {}", s),
                            None => "GUI crashed with unknown error".to_string(),
                        },
                    };
                    
                    let _ = std::fs::write(crash_log, message);
                }
            }
        }
    }
    
    #[cfg(not(feature = "gui"))]
    {
        eprintln!("GUI mode is not available in this build");
    }
    
    #[cfg(not(any(feature = "gui", feature = "cli")))]
    {
        eprintln!("Error: Neither GUI nor CLI features are enabled!");
    }
}

#[cfg(feature = "cli")]
async fn process_cli_mode() {
    if let Some(log_dir) = get_log_directory() {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("eim_cli.log"))
            .map(|mut file| {
                use std::io::Write;
                let _ = writeln!(file, "Entering CLI mode at {:?}", std::time::SystemTime::now());
                let _ = writeln!(file, "Args: {:?}", std::env::args().collect::<Vec<_>>());
            });
    }
    
    let cli = cli::cli_args::Cli::parse();
    
    // Setup logging with better error handling
    if let Err(err) = setup_logging(&cli) {
        eprintln!("Failed to setup logging: {}", err);
        
        // Try to set up a minimal logger as fallback
        if let Some(log_dir) = get_log_directory() {
            let minimal_log = log_dir.join("eim_cli_minimal.log");
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(minimal_log)
                .map(|mut file| {
                    use std::io::Write;
                    let _ = writeln!(file, "Using minimal logging due to setup error: {}", err);
                });
        }
    }
    
    set_locale(&cli.locale);
    
    // Process CLI commands with better error handling
    let settings = match Settings::new(cli.config.clone(), cli.into_iter()) {
        Ok(s) => s,
        Err(err) => {
            error!("Error creating settings: {}", err);
            eprintln!("Error: {}", err);
            return;
        }
    };
    
    // Run the wizard with proper error handling
    match cli::wizard::run_wizzard_run(settings).await {
        Ok(r) => {
            info!("Wizard result: {:?}", r);
            println!("Successfully installed IDF");
            println!("Now you can start using IDF tools");
        }
        Err(err) => {
            error!("Error running wizard: {}", err);
            eprintln!("Error: {}", err);
        }
    }
}

#[cfg(all(target_os = "windows", not(debug_assertions)))]
fn setup_windows_console(force_cli_mode: bool) {
    use std::io::{self, Write};
    
    // Only set up console if this is definitely CLI mode
    if !force_cli_mode {
        // For GUI mode, ensure we don't have a console
        return;
    }
    
    // Log this operation
    if let Some(log_dir) = get_log_directory() {
        let log_path = log_dir.join("eim_console_setup.log");
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map(|mut file| {
                let _ = writeln!(file, "Setting up Windows console for CLI mode");
            });
    }
    
    // Windows-specific console setup code
    use winapi::um::consoleapi::{AllocConsole, GetConsoleMode, SetConsoleMode};
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS, ENABLE_VIRTUAL_TERMINAL_PROCESSING};
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::{STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    
    let mut console_attached = false;
    
    // Try to attach to parent console first
    unsafe {
        console_attached = AttachConsole(ATTACH_PARENT_PROCESS) != 0;
        
        // Log the attempt
        if let Some(log_dir) = get_log_directory() {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("eim_console_setup.log"))
                .map(|mut file| {
                    let _ = writeln!(file, "AttachConsole result: {}", console_attached);
                });
        }
    }
    
    // Only allocate a new console if absolutely necessary and in CLI mode
    if !console_attached && force_cli_mode {
        unsafe {
            let allocated = AllocConsole() != 0;
            console_attached = allocated;
            
            // Log the allocation attempt
            if let Some(log_dir) = get_log_directory() {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_dir.join("eim_console_setup.log"))
                    .map(|mut file| {
                        let _ = writeln!(file, "AllocConsole result: {}", allocated);
                    });
            }
        }
    }
    
    // Only set up ANSI colors if we have a console
    if console_attached {
        unsafe {
            let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
            let stderr_handle = GetStdHandle(STD_ERROR_HANDLE);
            
            if stdout_handle != INVALID_HANDLE_VALUE && !stdout_handle.is_null() {
                let mut mode: u32 = 0;
                if GetConsoleMode(stdout_handle, &mut mode) != 0 {
                    let _ = SetConsoleMode(stdout_handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }
            
            if stderr_handle != INVALID_HANDLE_VALUE && !stderr_handle.is_null() {
                let mut mode: u32 = 0;
                if GetConsoleMode(stderr_handle, &mut mode) != 0 {
                    let _ = SetConsoleMode(stderr_handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }
            
            // Force reinitialize standard streams
            let _ = io::stdout().write_all(b"");
            let _ = io::stderr().write_all(b"");
        }
    }
}