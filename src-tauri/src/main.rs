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
use log::{debug, info};
#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "cli")]
rust_i18n::i18n!("locales", fallback = "en");

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

#[cfg(all(target_os = "windows", feature = "gui", feature = "cli"))]
fn setup_interactive_console() -> bool {
    use winapi::um::consoleapi::{AllocConsole, GetConsoleMode, SetConsoleMode};
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    use winapi::um::wincon::{ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT};

    unsafe {
        if AllocConsole() == 0 {
            return false;
        }

        // Get handles
        let stdin_handle = GetStdHandle(STD_INPUT_HANDLE);
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let stderr_handle = GetStdHandle(STD_ERROR_HANDLE);

        // Configure console mode for interactive input
        let mut mode: u32 = 0;
        if GetConsoleMode(stdin_handle, &mut mode) != 0 {
            mode |= ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT;
            SetConsoleMode(stdin_handle, mode);
        }
    }

    true
}

#[cfg(all(target_os = "windows", feature = "cli"))]
fn is_interactive_command() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        return false;
    }

    // Commands that need interactive console
    let interactive_commands = vec!["select", "rename", "remove"];

    for arg in args.iter().skip(1) {
        for cmd in &interactive_commands {
            if arg == "wizard" {
                return true;
            } else if arg == cmd && args.len() == 2 {
                return true;
            }
        }
    }

    false
}

#[tokio::main]
async fn main() {
    #[cfg(not(any(feature = "gui", feature = "cli")))]
    {
        eprintln!("Error: Neither GUI nor CLI features are enabled!");
        return;
    }
    #[cfg(all(target_os = "windows", feature = "gui", feature = "cli"))]
    if is_interactive_command() {
        setup_interactive_console()
    } else {
        false
    };
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
        println!("Pressing Enter to exit...");
        detach_console();
    } else {
        debug!("This is the end...");
    }
    std::process::exit(0);
}
