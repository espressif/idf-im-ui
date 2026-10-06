use std::path::PathBuf;

use anyhow::Context;
#[cfg(not(feature = "gui"))]
use clap::CommandFactory;
use cli_args::Cli;
use cli_args::Commands;
#[cfg(feature = "gui")]
use cli_args::InstallArgs;
use fern::Dispatch;
use idf_im_lib::get_log_directory;
use idf_im_lib::idf_config::{IdfInstallation, InstallationStatus, IDF_CONFIG_FILE_NAME};
use idf_im_lib::logging::formatter;
use idf_im_lib::telemetry;
use idf_im_lib::version_manager::get_selected_version;
use log::info;
use log::LevelFilter;
use rust_i18n::t;

pub mod cli_args;
mod commands;
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
        (0, true) => LevelFilter::Debug, // Non-interactive needs Debug minimum
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
        // Filter reqwest to only show warnings and errors
        .level_for("reqwest", LevelFilter::Warn)
        // Apply file at Trace level
        .chain(
            Dispatch::new()
                .level(file_level)
                .chain(fern::log_file(&log_file_path)?),
        )
        // Apply console at configurable level
        .chain(
            Dispatch::new()
                .level(console_level)
                .chain(std::io::stderr()),
        )
        .apply()?;

    log::trace!(
        "CLI logging initialized. Console: {:?}, File: {:?}",
        console_level,
        file_level
    );
    Ok(())
}

/// Global CLI options shared by the subcommand handlers.
pub struct CommandContext {
    pub config_path: Option<PathBuf>,
    pub esp_idf_json_path: Option<String>,
    pub do_not_track: bool,
    #[cfg(feature = "gui")]
    pub verbose: u8,
}

/// Resolves the IDF identifier to operate on: the one explicitly passed on
/// the command line, or the currently selected version if none was given.
fn resolve_idf_identifier(
    idf: Option<String>,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<String> {
    if let Some(idf_str) = idf {
        Ok(idf_str)
    } else if let Some(selected) = get_selected_version(config_path) {
        info!("{}", t!("cli.using_selected_idf", idf = selected.name));
        Ok(selected.id)
    } else {
        Err(anyhow::anyhow!(t!("cli.no_idf_specified_no_selected")))
    }
}

fn status_label(status: &InstallationStatus) -> String {
    match status {
        InstallationStatus::Finished => t!("list.status.finished").to_string(),
        InstallationStatus::InProgress => t!("list.status.in_progress").to_string(),
        InstallationStatus::Failed => t!("list.status.failed").to_string(),
        InstallationStatus::BeingRepaired => t!("list.status.being_repaired").to_string(),
        InstallationStatus::Broken => t!("list.status.broken").to_string(),
    }
}

/// Menu entries of the form `name [status]` for picking an installed version.
fn version_option_labels(versions: &[IdfInstallation]) -> Vec<String> {
    versions
        .iter()
        .map(|v| format!("{} [{}]", v.name, status_label(&v.status)))
        .collect()
}

fn config_path_for(esp_idf_json_path: Option<&String>) -> Option<PathBuf> {
    esp_idf_json_path.map(|p| PathBuf::from(p).join(IDF_CONFIG_FILE_NAME))
}

pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    let do_not_track = cli.do_not_track;
    telemetry::set_enabled(!do_not_track);
    // Initial tracking of CLI start
    #[cfg(feature = "gui")]
    let command = cli
        .clone()
        .command
        .unwrap_or(Commands::Gui(InstallArgs::default()));
    #[cfg(not(feature = "gui"))]
    if cli.clone().command.is_none() {
        Cli::command().print_help().expect(&t!("cli.no_command"));
        return Ok(());
    }
    #[cfg(not(feature = "gui"))]
    let command = cli.clone().command.unwrap();
    // Handle completions and help-json first, before any logging or output setup,
    // so their output stays pure (no log messages mixed into scripts or JSON).
    match &command {
        Commands::Completions { shell } => {
            commands::help::print_completions(*shell);
            return Ok(());
        }
        Commands::HelpJson => {
            commands::help::print_help_json();
            return Ok(());
        }
        _ => {}
    }

    prepare_output(&command, cli.verbose, cli.log_file)?;
    if !do_not_track {
        telemetry::track_cli_invoked(subcommand_name(&command));
    }
    let ctx = CommandContext {
        config_path: config_path_for(cli.esp_idf_json_path.as_ref()),
        esp_idf_json_path: cli.esp_idf_json_path,
        do_not_track,
        #[cfg(feature = "gui")]
        verbose: cli.verbose,
    };
    dispatch(command, &ctx).await
}

fn prepare_output(command: &Commands, verbose: u8, log_file: Option<String>) -> anyhow::Result<()> {
    match command {
        #[cfg(feature = "gui")]
        Commands::Gui(_) => {
            println!("{}", t!("gui.running"));
            // Skip CLI logging setup - tauri-plugin-log handles GUI logging
        }
        _ => {
            setup_cli(verbose, false, log_file.map(PathBuf::from))
                .context("Failed to setup logging")?;
            warn_if_elevated();
        }
    }
    Ok(())
}

fn warn_if_elevated() {
    if idf_im_lib::utils::is_running_elevated() {
        log::warn!("Running as elevated user. This is not recommended but it is required if you want to install drivers.");
        if cfg!(target_os = "windows") {
            println!("{}", t!("cli.running_as_elevated_windows"));
        } else {
            println!("{}", t!("cli.running_as_elevated_posix"));
        }
    }
}

async fn dispatch(command: Commands, ctx: &CommandContext) -> anyhow::Result<()> {
    let config_path = ctx.config_path.as_ref();
    match command {
        Commands::Completions { .. } | Commands::HelpJson => unreachable!(),
        Commands::Install(install_args) => commands::install::install(install_args, ctx).await,
        Commands::Wizard(install_args) => commands::install::wizard(install_args, ctx).await,
        Commands::Fix { install_args } => commands::install::fix(install_args, config_path).await,
        #[cfg(feature = "gui")]
        Commands::Gui(install_args) => commands::install::gui(install_args, ctx),
        Commands::List => commands::list::list(config_path),
        Commands::ListTools {
            identifier,
            outdated,
        } => commands::list::list_tools(identifier, outdated, config_path),
        Commands::ListFeatures { identifier } => {
            commands::list::list_features(identifier, config_path)
        }
        Commands::Select { version } => commands::manage::select(version, config_path),
        Commands::Rename { version, new_name } => {
            commands::manage::rename(version, new_name, config_path)
        }
        Commands::Remove { version } => commands::manage::remove(version, config_path),
        Commands::Purge => commands::manage::purge(config_path),
        Commands::Import { path } => commands::manage::import(path, config_path),
        Commands::Run { command, idf } => commands::run::run(command, idf, config_path),
        Commands::Shell { idf } => commands::run::shell(idf, config_path),
        Commands::Discover => {
            // TODO: Implement version discovery. Planned shape: list the IDF folders
            // found by `version_manager::find_esp_idf_folders("/")` under the
            // `discover.title` heading, then report each with `discover.found`.
            unimplemented!("Version discovery not implemented yet")
        }
        Commands::InstallDrivers => commands::run::install_drivers().await,
    }
}

fn subcommand_name(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Install(_) => "install",
        Commands::List => "list",
        Commands::ListTools { .. } => "list-tools",
        Commands::ListFeatures { .. } => "list-features",
        Commands::Select { .. } => "select",
        Commands::Discover => "discover",
        Commands::Remove { .. } => "remove",
        Commands::Rename { .. } => "rename",
        Commands::Run { .. } => "run",
        Commands::Shell { .. } => "shell",
        Commands::Import { .. } => "import",
        Commands::Purge => "purge",
        Commands::Wizard(_) => "wizard",
        Commands::Fix { .. } => "fix",
        Commands::InstallDrivers => "install-drivers",
        Commands::Completions { .. } => "completions",
        Commands::HelpJson => "help-json",
        #[cfg(feature = "gui")]
        Commands::Gui(_) => "gui",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn installation(name: &str, status: InstallationStatus) -> IdfInstallation {
        IdfInstallation {
            activation_script: None,
            id: format!("id-{}", name),
            idf_tools_path: String::new(),
            name: name.to_string(),
            path: format!("/esp/{}", name),
            python: None,
            installation_config: None,
            status,
        }
    }

    #[test]
    fn version_option_labels_include_status() {
        let versions = vec![
            installation("v5.4", InstallationStatus::Finished),
            installation("v5.3", InstallationStatus::Broken),
        ];
        assert_eq!(
            version_option_labels(&versions),
            vec![
                format!("v5.4 [{}]", t!("list.status.finished")),
                format!("v5.3 [{}]", t!("list.status.broken")),
            ]
        );
    }

    #[test]
    fn version_option_labels_empty() {
        assert!(version_option_labels(&[]).is_empty());
    }

    #[test]
    fn config_path_for_joins_config_file_name() {
        let dir = "/tmp/eim".to_string();
        assert_eq!(
            config_path_for(Some(&dir)),
            Some(PathBuf::from("/tmp/eim").join(IDF_CONFIG_FILE_NAME))
        );
        assert_eq!(config_path_for(None), None);
    }

    #[test]
    fn resolve_idf_identifier_prefers_explicit_value() {
        let id = resolve_idf_identifier(Some("v5.4".to_string()), None).unwrap();
        assert_eq!(id, "v5.4");
    }

    #[test]
    fn subcommand_name_matches_cli_spelling() {
        for (args, expected) in [
            (vec!["eim", "list"], "list"),
            (vec!["eim", "list-tools"], "list-tools"),
            (vec!["eim", "list-features"], "list-features"),
            (vec!["eim", "purge"], "purge"),
            (vec!["eim", "install-drivers"], "install-drivers"),
            (vec!["eim", "help-json"], "help-json"),
            (vec!["eim", "run", "idf.py build"], "run"),
            (vec!["eim", "rename", "old", "new"], "rename"),
        ] {
            let cli = Cli::try_parse_from(&args).unwrap();
            assert_eq!(subcommand_name(&cli.command.unwrap()), expected);
        }
    }
}
