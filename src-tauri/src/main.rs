// Apply windows_subsystem = "windows" only if "gui" feature is present and not in debug mode
#![cfg_attr(
    all(feature = "gui", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(feature = "gui")]
pub mod gui;

#[cfg(feature = "cli")]
use clap::Parser;
#[cfg(feature = "cli")]
use config::ConfigError;
#[cfg(feature = "cli")]
use idf_im_lib::get_log_directory;
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

#[cfg(all(target_os = "windows", feature = "cli"))]
fn has_console() -> bool {
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_OUTPUT_HANDLE;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        !(handle == INVALID_HANDLE_VALUE || handle.is_null())
    }
}

#[cfg(all(target_os = "windows", feature = "cli"))]
fn attach_console() -> bool {
    use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

#[cfg(all(target_os = "windows", feature = "cli"))]
fn detach_console() {
    use winapi::um::wincon::{FreeConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        FreeConsole(); // Detach from any allocated or attached console
    }
}

#[tokio::main]
async fn main() {
    #[cfg(not(any(feature = "gui", feature = "cli")))]
    {
        eprintln!("Error: Neither GUI nor CLI features are enabled!");
        return;
    }
    #[cfg(not(feature = "gui"))]
    {
        let cli = cli::cli_args::Cli::parse();
        set_locale(&cli.locale);

        cli::run_cli(cli).await;

        return;
    }
    #[cfg(not(feature = "cli"))]
    {
        return gui::run();
    }
    // both GUI and CLI features are enabled
    #[cfg(target_os = "windows")]
    let mut console_attached_or_allocated = false;

    let has_args = std::env::args().len() > 1;

    // Remove the windows_subsystem attribute if we're in CLI mode with arguments
    #[cfg(target_os = "windows")]
    {
        if has_args {
            let has_existing_console = has_console();
            if !has_existing_console {
                let attached = attach_console();
                if !attached {
                    unsafe {
                        winapi::um::consoleapi::AllocConsole();
                        console_attached_or_allocated = true;
                    }
                } else {
                    console_attached_or_allocated = true;
                }
            }
        }
    }
    let cli = cli::cli_args::Cli::parse();
    set_locale(&cli.locale);

    cli::run_cli(cli).await; // Run the GUI by default if no arguments are provided

    #[cfg(target_os = "windows")]
    if console_attached_or_allocated {
        detach_console();
    }
}
