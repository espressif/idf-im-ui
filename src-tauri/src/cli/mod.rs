use std::path::PathBuf;

use anyhow::Context;
use anyhow::anyhow;
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
use idf_im_lib::idf_config::{IDF_CONFIG_FILE_NAME, InstallationStatus};
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
use rust_i18n::t;

#[cfg(feature = "gui")]
use crate::gui;

use idf_im_lib::telemetry::{
    self as telemetry, ErrorKind, InstallMode, InstallOutcome, InstallationContext, Interface,
    OutcomeExtras,
};

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
        // Filter reqwest to only show warnings and errors
        .level_for("reqwest", LevelFilter::Warn)
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
                .chain(std::io::stderr())
        )
        .apply()?;

    log::trace!("CLI logging initialized. Console: {:?}, File: {:?}", console_level, file_level);
    Ok(())
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

fn format_tool_list_report(report: &idf_im_lib::version_manager::ToolListReport) {
    println!(
        "{}",
        t!(
            "list_tools.title",
            name = report.idf.name,
            path = report.idf.path
        )
    );
    println!();
    for entry in &report.tools {
        if entry.tool.install == "on_request" {
            println!(
                "{}: {}{}",
                entry.tool.name,
                entry.tool.description,
                t!("list_tools.optional_marker")
            );
        } else {
            println!("{}: {}", entry.tool.name, entry.tool.description);
        }
        for vi in &entry.version_inspections {
            if !vi.has_platform_download {
                continue;
            }
            if let Some(info) = &vi.installed {
                println!(
                    "  - {} ({}){}",
                    vi.version.name,
                    vi.version.status,
                    t!("list_tools.installed", version = info.version)
                );
            } else {
                println!(
                    "  - {} ({}){}",
                    vi.version.name,
                    vi.version.status,
                    t!("list_tools.not_installed")
                );
            }
        }
    }
    if report.outdated_only {
        println!();
        if report.outdated.is_empty() {
            println!("{}", t!("list_tools.no_outdated"));
        } else {
            println!("{}", t!("list_tools.outdated_header"));
            for o in &report.outdated {
                println!(
                    "{}",
                    t!(
                        "list_tools.outdated_line",
                        name = o.name,
                        installed = o.installed,
                        available = o.available
                    )
                );
            }
        }
    }
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
        Cli::command()
            .print_help()
            .expect(&t!("cli.no_command"));
        return Ok(());
    }
    #[cfg(not(feature = "gui"))]
    let command = cli.clone().command.unwrap();
    // Handle completions first, before any logging or output setup.
    // This ensures shell completion scripts are pure without any log messages.
    if let Commands::Completions { shell } = &command {
        let mut cmd = Cli::command();
        let bin_name = env!("CARGO_PKG_NAME");
        generate(*shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }
    // Handle help-json before any logging or output setup to ensure pure JSON output.
    if let Commands::HelpJson = &command {
        let cmd = Cli::command();

        fn build_command_json(cmd: &clap::Command) -> serde_json::Value {
            let mut subcommands = Vec::new();
            for subcommand in cmd.get_subcommands() {
                subcommands.push(serde_json::json!({
                    "name": subcommand.get_name().to_string(),
                    "about": subcommand.get_about().map(|s| s.to_string()),
                    "args": build_args_json(subcommand),
                }));
            }

            serde_json::json!({
                "name": cmd.get_name().to_string(),
                "about": cmd.get_about().map(|s| s.to_string()),
                "version": cmd.get_version().map(|v| v.to_string()),
                "subcommands": subcommands,
                "global_args": build_args_json(cmd),
            })
        }

        fn build_args_json(cmd: &clap::Command) -> serde_json::Value {
            let mut args = Vec::new();
            for arg in cmd.get_arguments() {
                let short = arg.get_short().map(|c| format!("-{}", c));
                let long = arg.get_long().map(|s| format!("--{}", s));
                let default_value = arg.get_default_values().first()
                    .and_then(|v| v.to_str())
                    .map(|s| s.to_string());
                let possible_values_vec = arg.get_possible_values();
                let possible_values: Option<Vec<&str>> = if possible_values_vec.is_empty() {
                    None
                } else {
                    Some(possible_values_vec.iter().map(|pv| pv.get_name()).collect())
                };

                args.push(serde_json::json!({
                    "id": arg.get_id().to_string(),
                    "short": short,
                    "long": long,
                    "help": arg.get_help().map(|s| s.to_string()),
                    "required": arg.is_required_set(),
                    "default_value": default_value,
                    "possible_values": possible_values,
                }));
            }
            serde_json::json!(args)
        }

        let json_help = build_command_json(&cmd);
        println!("{}", serde_json::to_string_pretty(&json_help).expect("Failed to serialize help to JSON"));
        return Ok(());
    }

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
            let is_elevated = idf_im_lib::utils::is_running_elevated();
            if is_elevated {
                log::warn!("Running as elevated user. This is not recommended but it is required if you want to install drivers.");
                if cfg!(target_os = "windows") {
                    println!("{}", t!("cli.running_as_elevated_windows"));
                } else {
                    println!("{}", t!("cli.running_as_elevated_posix"));
                }
            }
        }
    }
    if !do_not_track {
        telemetry::track_cli_invoked(subcommand_name(&command));
    }
    let cli_esp_idf_json_path = cli.esp_idf_json_path;
    let config_path = cli_esp_idf_json_path.as_ref().map(|p| PathBuf::from(p).join(IDF_CONFIG_FILE_NAME));
    match command {
        Commands::Completions { .. } => unreachable!(),
        Commands::HelpJson => unreachable!(),
        Commands::Install(install_args) => {
            let settings = Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            );
            debug!("Returned settings: {:?}", settings);
            match settings {
                Ok(mut settings) => {
                  debug!("Settings before adjustments: {:?}", settings);
                  if let Some(ref p) = cli_esp_idf_json_path {
                    settings.esp_idf_json_path = Some(p.clone());
                  }
                  if install_args.install_all_prerequisites.is_none() { // if cli argument is not set
                    settings.install_all_prerequisites = Some(true); // The non-interactive install will always install all prerequisites
                  }
                  match settings.initialize_esp_ide_json() {
                    Ok(_) => debug!("ESP-IDF JSON initialized at configured path."),
                    Err(e) => warn!("Failed to initialize ESP-IDF JSON: {}. IDE integration may not work correctly.", e),
                  }
                  debug!("Settings after adjustments: {:?}", settings);
                  // Check if the provided path is already an installed IDF
                  if let Some(ref path) = settings.path {
                      match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                          Ok(versions) => {
                            debug!("Checking provided path against installed versions. Provided path: '{}'", path.display());
                            if let Some(provided_path) = idf_im_lib::utils::normalize_path_for_comparison(&path.to_string_lossy()) {
                              if versions.iter().any(|version| {
                                let version_path = idf_im_lib::utils::normalize_path_for_comparison(&version.path);
                                debug!("Normalized version_path for '{}': {:?}", version.path, version_path);
                                version_path.map_or(false, |p| p == provided_path)
                              }) {
                                info!("{}", t!("install.already_installed", path = path.display()));
                                info!("{}", t!("install.use_fix_command"));
                                return Ok(());
                              }
                            }
                          }
                          Err(err) => {
                              debug!("Could not list installed versions: {}", err);
                          }
                      }
                  } else {
                      debug!("No path provided in settings, skipping installed version check");
                  }
                  // Create InProgress entries before installation starts so interruptions are detectable
                  if let Err(e) = settings.create_pending_esp_ide_json() {
                      warn!("Failed to create pending installation entries: {}", e);
                  }

                  let ctx = build_cli_context(&settings, InstallMode::Cli);
                  let extras = build_cli_extras(&settings);
                  if !do_not_track {
                      telemetry::track_install_started(&ctx);
                  }
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("{}", t!("install.wizard_result", r = "Ok".to_string()));
                            info!("{}", t!("install.success"));
                            info!("{}", t!("install.ready"));
                            if !do_not_track {
                              telemetry::track_install_outcome(
                                  &ctx,
                                  InstallOutcome::Success,
                                  None,
                                  None,
                                  extras,
                              );
                            }
                            Ok(())
                        }
                        Err(err) => {
                          if !do_not_track {
                            let wrapped = anyhow::anyhow!(err.clone());
                            telemetry::track_install_outcome(
                                &ctx,
                                InstallOutcome::Failure,
                                Some(ErrorKind::from_message(&err)),
                                Some(&wrapped),
                                extras,
                            );
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
            match idf_im_lib::version_manager::get_esp_ide_config(config_path.as_ref()) {
                Ok(config) => {
                    if config.idf_installed.is_empty() {
                        warn!("{}", t!("list.no_versions"));
                        Ok(())
                    } else {
                        println!("{}", t!("list.installed_title"));
                        for version in config.idf_installed {
                            let sl = status_label(&version.status);
                            if version.id == config.idf_selected_id {
                                println!("{}", t!("list.version_selected", name = version.name, path = version.path, status = sl));
                            } else {
                                println!("{}", t!("list.version", name = version.name, path = version.path, status = sl));
                            }
                        }
                        Ok(())
                    }
                }
                Err(err) => {
                    info!("{}", t!("list.no_versions"));
                    info!("{}", t!("cli.hint.custom_json_path"));
                    debug!("Error: {}", err);
                    Ok(())
                }
            }
        }
        Commands::ListTools { identifier, outdated } => {
            let identifier = if let Some(id) = identifier {
                Some(id)
            } else {
                match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                    Ok(versions) if versions.is_empty() => {
                        warn!("{}", t!("list.no_versions"));
                        return Ok(());
                    }
                    Ok(versions) => {
                        let options: Vec<String> = versions
                            .iter()
                            .map(|v| format!("{} [{}]", v.name, status_label(&v.status)))
                            .collect();
                        match helpers::generic_select_index(&t!("list_tools.idf_prompt"), &options) {
                            Ok(i) => Some(versions[i].name.clone()),
                            Err(err) => return Err(anyhow::anyhow!(err)),
                        }
                    }
                    Err(err) => {
                        debug!("Error: {}", err);
                        warn!("{}", t!("list.no_versions"));
                        info!("{}", t!("cli.hint.custom_json_path"));
                        return Ok(());
                    }
                }
            };

            let report = idf_im_lib::version_manager::list_idf_tools(
                identifier.as_deref(),
                outdated,
                config_path.as_ref(),
            );
            match report {
                Ok(report) => {
                    format_tool_list_report(&report);
                    Ok(())
                }
                Err(err) => {
                    error!("{}", err);
                    Err(anyhow::anyhow!(err))
                }
            }
        }
        Commands::Select { version } => {
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                    Ok(versions) => {
                        if versions.is_empty() {
                            warn!("{}", t!("select.no_versions"));
                            Ok(())
                        } else {
                            println!("{}", t!("select.available_title"));
                            let options: Vec<String> = versions
                                .iter()
                                .map(|v| format!("{} [{}]", v.name, status_label(&v.status)))
                                .collect();
                            match helpers::generic_select_index(&t!("select.prompt"), &options) {
                                Ok(i) => match select_idf_version(&versions[i].name, config_path.as_ref()) {
                                    Ok(_) => {
                                        println!("{}", t!("select.success", version = versions[i].name));
                                        if let Some(selected) = get_selected_version(config_path.as_ref()) {
                                          println!("{}", t!("wizard.separator.line"));
                                          println!("{}", t!("cli.select.activation_instructions"));
                                          let script = selected.activation_script.as_deref().unwrap_or("");
                                          match std::env::consts::OS {
                                            "windows" => println!(". \"{}\"", script),
                                            _ => println!("source \"{}\"", script),
                                          }
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
                        info!("{}", t!("cli.hint.custom_json_path"));
                        debug!("Error: {}", err);
                        Err(anyhow::anyhow!(err))
                    }
                }
            } else {
                match select_idf_version(&version.clone().unwrap(), config_path.as_ref()) {
                    Ok(_) => {
                        info!("{}", t!("select.success", version = version.clone().unwrap()));
                        if let Some(selected) = get_selected_version(config_path.as_ref()) {
                          info!("{}", t!("wizard.separator.line"));
                          info!("{}", t!("cli.select.activation_instructions"));
                          let script = selected.activation_script.as_deref().unwrap_or("");
                          match std::env::consts::OS {
                            "windows" => info!(". {}", script),
                            _ => info!("source {}", script),
                          };
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
            } else if let Some(selected) = get_selected_version(config_path.as_ref()) {
                info!("{}", t!("run.using_selected", idf = selected.name));
                selected.id
            } else {
                return Err(anyhow::anyhow!(t!("run.no_idf_specified_no_selected")));
            };

            match run_command_in_context(&idf_identifier, &command, config_path.as_ref()) {
                Ok(status) => {
                    if !status.success() {
                        return Err(anyhow::anyhow!(t!("run.command_failed")));
                    }
                    Ok(())
                }
                Err(err) => Err(err),
            }
        }
        Commands::Rename { version, new_name } => {
            if version.is_none() {
                match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                    Ok(versions) => {
                        if versions.is_empty() {
                            warn!("{}", t!("rename.no_versions"));
                            Ok(())
                        } else {
                            let options: Vec<String> = versions
                                .iter()
                                .map(|v| format!("{} [{}]", v.name, status_label(&v.status)))
                                .collect();
                            let version = match helpers::generic_select_index(
                                &t!("rename.prompt"),
                                &options,
                            ) {
                                Ok(i) => versions[i].name.clone(),
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
                                &version, new_name, config_path.as_ref(),
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
                        info!("{}", t!("cli.hint.custom_json_path"));
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
                    config_path.as_ref(),
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
                    config_path.as_ref(),
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
                match idf_im_lib::utils::parse_tool_set_config(&config_file, config_path.as_ref()) {
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
                match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                    Ok(versions) => {
                        if versions.is_empty() {
                            info!("{}", t!("remove.no_versions"));
                            Ok(())
                        } else {
                            println!("{}", t!("remove.available_title"));
                            let options: Vec<String> = versions
                                .iter()
                                .map(|v| format!("{} [{}]", v.name, status_label(&v.status)))
                                .collect();
                            match helpers::generic_select_index(&t!("remove.prompt"), &options) {
                                Ok(i) => match remove_single_idf_version(&versions[i].name, false, config_path.as_ref()) {
                                    Ok(_) => {
                                        info!("{}", t!("remove.success", version = versions[i].name));
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
                match remove_single_idf_version(&version.clone().unwrap(), false, config_path.as_ref()) {
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
            match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                Ok(versions) => {
                    if versions.is_empty() {
                        println!("{}", t!("purge.no_versions"));
                        Ok(())
                    } else {
                        let mut failed = false;
                        for version in versions {
                            info!("{}", t!("purge.removing", version = version.name));
                            match remove_single_idf_version(&version.name, false, config_path.as_ref()) {
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
                    if let Some(ref p) = cli_esp_idf_json_path {
                      settings.esp_idf_json_path = Some(p.clone());
                    }
                    settings.non_interactive = Some(false);
                    match settings.initialize_esp_ide_json() {
                      Ok(_) => debug!("ESP-IDF JSON initialized at configured path."),
                      Err(e) => warn!("Failed to initialize ESP-IDF JSON: {}. IDE integration may not work correctly.", e),
                    }

                    // Check for incomplete installations and offer fix/delete
                    wizard::check_and_handle_incomplete_installations(&settings, config_path.as_ref()).await;

                    // Create InProgress entries before installation starts so interruptions are detectable
                    if let Err(e) = settings.create_pending_esp_ide_json() {
                        warn!("Failed to create pending installation entries: {}", e);
                    }

                    let ctx = build_cli_context(&settings, InstallMode::Cli);
                    let extras = build_cli_extras(&settings);
                    if !do_not_track {
                      telemetry::track_install_started(&ctx);
                    }
                    let result = wizard::run_wizzard_run(settings).await;
                    match result {
                        Ok(r) => {
                            info!("{}", t!("install.wizard_result"));
                            info!("{}", t!("install.success"));
                            info!("{}", t!("install.ready"));
                            if !do_not_track {
                              telemetry::track_install_outcome(
                                  &ctx,
                                  InstallOutcome::Success,
                                  None,
                                  None,
                                  extras,
                              );
                            }
                            Ok(())
                        }
                        Err(err) => {
                          if !do_not_track {
                            let wrapped = anyhow::anyhow!(err.clone());
                            telemetry::track_install_outcome(
                                &ctx,
                                InstallOutcome::Failure,
                                Some(ErrorKind::from_message(&err)),
                                Some(&wrapped),
                                extras,
                            );
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
              match idf_im_lib::version_manager::list_installed_versions(config_path.as_ref()) {
                Ok(versions) => {
                  if versions.is_empty() {
                      warn!("{}", t!("fix.no_versions"));
                      return Ok(());
                  } else {
                    let options: Vec<String> = versions
                        .iter()
                        .map(|v| format!("{} ({}) [{}]", v.name, v.path, status_label(&v.status)))
                        .collect();
                    let version_path = match helpers::generic_select_index(
                        &t!("fix.prompt"),
                        &options,
                    ) {
                        Ok(i) => versions[i].path.clone(),
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
          let settings = prepare_settings_for_fix_idf_installation(path_to_fix.clone(), config_path.as_ref()).await?;
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
        Commands::Gui(install_args) => {
            #[cfg(not(feature = "gui"))]
            unimplemented!("GUI not present in this type of build");
            let log_level = match cli.verbose {
                0 => LevelFilter::Info,
                1 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            };
            let do_not_track = cli.do_not_track;
            let settings = match Settings::new(
                install_args.config.clone(),
                install_args.clone().into_iter(),
            ) {
              Ok(mut settings) => {
                if let Some(ref p) = cli_esp_idf_json_path {
                  settings.esp_idf_json_path = Some(p.clone());
                }
                Some(settings)
              }
              Err(_) => None
            };
            gui::run(settings, Some(log_level), do_not_track);
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

fn subcommand_name(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Install(_) => "install",
        Commands::List => "list",
        Commands::ListTools { .. } => "list-tools",
        Commands::Select { .. } => "select",
        Commands::Discover => "discover",
        Commands::Remove { .. } => "remove",
        Commands::Rename { .. } => "rename",
        Commands::Run { .. } => "run",
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

fn build_cli_context(settings: &Settings, mode: InstallMode) -> InstallationContext {
    let installation_ids: Vec<String> = settings
        .pending_installation_ids
        .as_ref()
        .map(|m| m.values().cloned().collect())
        .unwrap_or_default();
    let versions = settings.idf_versions.clone().unwrap_or_default();
    telemetry::new_session(Interface::Cli, mode, versions, installation_ids)
}

fn build_cli_extras(settings: &Settings) -> OutcomeExtras {
    let feature_count = settings
        .idf_features
        .as_ref()
        .map(|v| v.len())
        .or_else(|| {
            settings
                .idf_features_per_version
                .as_ref()
                .map(|m| m.values().map(|v| v.len()).sum())
        });
    let tool_count = settings
        .idf_tools
        .as_ref()
        .map(|v| v.len())
        .or_else(|| {
            settings
                .idf_tools_per_version
                .as_ref()
                .map(|m| m.values().map(|v| v.len()).sum())
        });
    let target_count = settings.target.as_ref().map(|v| v.len());
    let non_interactive = settings.non_interactive;
    let used_existing_idf = settings
        .path
        .as_ref()
        .and_then(|p| is_valid_idf_directory(p.to_str().unwrap_or_default()).then_some(true));
    OutcomeExtras {
        feature_count,
        tool_count,
        target_count,
        non_interactive,
        used_existing_idf,
    }
}
