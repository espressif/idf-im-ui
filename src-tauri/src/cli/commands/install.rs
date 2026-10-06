use std::collections::HashMap;
use std::path::{Path, PathBuf};

use idf_im_lib::settings::Settings;
use idf_im_lib::telemetry::{
    self as telemetry, ErrorKind, InstallMode, InstallOutcome, InstallationContext, Interface,
    OutcomeExtras,
};
use idf_im_lib::utils::is_valid_idf_directory;
use idf_im_lib::version_manager::prepare_settings_for_fix_idf_installation;
use log::{debug, error, info, warn};
use rust_i18n::t;

use crate::cli::cli_args::InstallArgs;
use crate::cli::{helpers, status_label, wizard, CommandContext};

pub async fn install(install_args: InstallArgs, ctx: &CommandContext) -> anyhow::Result<()> {
    let settings = Settings::new(install_args.config.clone(), install_args.clone());
    debug!("Returned settings: {:?}", settings);
    let mut settings = settings.map_err(|err| anyhow::anyhow!(err))?;
    debug!("Settings before adjustments: {:?}", settings);
    apply_esp_idf_json_path(&mut settings, ctx);
    if install_args.install_all_prerequisites.is_none() {
        // if cli argument is not set
        settings.install_all_prerequisites = Some(true); // The non-interactive install will always install all prerequisites
    }
    initialize_esp_ide_json(&settings);
    debug!("Settings after adjustments: {:?}", settings);
    if let Some(path) = already_installed_path(&settings, ctx.config_path.as_ref()) {
        info!("{}", t!("install.already_installed", path = path.display()));
        info!("{}", t!("install.use_fix_command"));
        return Ok(());
    }
    create_pending_entries(&mut settings);
    let wizard_result = t!("install.wizard_result", r = "Ok".to_string()).to_string();
    run_tracked_install(settings, InstallMode::Cli, wizard_result, ctx.do_not_track).await
}

pub async fn wizard(install_args: InstallArgs, ctx: &CommandContext) -> anyhow::Result<()> {
    info!("{}", t!("wizard.title"));
    let mut settings = Settings::new(install_args.config.clone(), install_args.clone())
        .map_err(|err| anyhow::anyhow!(err))?;
    apply_esp_idf_json_path(&mut settings, ctx);
    settings.non_interactive = Some(false);
    initialize_esp_ide_json(&settings);

    // Check for incomplete installations and offer fix/delete
    wizard::check_and_handle_incomplete_installations(&settings, ctx.config_path.as_ref()).await;

    create_pending_entries(&mut settings);
    let wizard_result = t!("install.wizard_result").to_string();
    run_tracked_install(
        settings,
        InstallMode::Wizard,
        wizard_result,
        ctx.do_not_track,
    )
    .await
}

pub async fn fix(install_args: InstallArgs, config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    let Some(path_to_fix) = resolve_path_to_fix(install_args.path.clone(), config_path)? else {
        return Ok(());
    };
    info!("{}", t!("fix.fixing", path = path_to_fix.display()));
    // The fix logic is just installation with use of existing repository.
    // Start from the settings the installation was originally created with (so tools,
    // features, target etc. are preserved), then let any CLI args the user explicitly
    // passed to `fix` override those preserved values (and the defaults).
    let mut settings =
        prepare_settings_for_fix_idf_installation(path_to_fix.clone(), config_path).await?;
    settings.apply_cli_overrides(install_args)?;
    match wizard::run_wizard_run(settings).await {
        Ok(_r) => {
            info!("{}", t!("fix.result", r = "Ok"));
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
pub fn gui(install_args: InstallArgs, ctx: &CommandContext) -> anyhow::Result<()> {
    let settings = match Settings::new(install_args.config.clone(), install_args.clone()) {
        Ok(mut settings) => {
            apply_esp_idf_json_path(&mut settings, ctx);
            Some(settings)
        }
        Err(_) => None,
    };
    crate::gui::run(settings, Some(gui_log_level(ctx.verbose)), ctx.do_not_track);
    Ok(())
}

#[cfg(feature = "gui")]
fn gui_log_level(verbose: u8) -> log::LevelFilter {
    match verbose {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}

fn apply_esp_idf_json_path(settings: &mut Settings, ctx: &CommandContext) {
    if let Some(ref p) = ctx.esp_idf_json_path {
        settings.esp_idf_json_path = Some(p.clone());
    }
}

fn initialize_esp_ide_json(settings: &Settings) {
    match settings.initialize_esp_ide_json() {
        Ok(_) => debug!("ESP-IDF JSON initialized at configured path."),
        Err(e) => warn!(
            "Failed to initialize ESP-IDF JSON: {}. IDE integration may not work correctly.",
            e
        ),
    }
}

fn create_pending_entries(settings: &mut Settings) {
    // Create InProgress entries before installation starts so interruptions are detectable
    if let Err(e) = settings.create_pending_esp_ide_json() {
        warn!("Failed to create pending installation entries: {}", e);
    }
}

/// Returns the requested install path when it matches an IDF that is already installed
/// and no explicit version name was given.
fn already_installed_path<'a>(
    settings: &'a Settings,
    config_path: Option<&PathBuf>,
) -> Option<&'a PathBuf> {
    let Some(path) = settings.path.as_ref() else {
        debug!("No path provided in settings, skipping installed version check");
        return None;
    };
    match idf_im_lib::version_manager::list_installed_versions(config_path) {
        Ok(versions) => {
            debug!(
                "Checking provided path against installed versions. Provided path: '{}'",
                path.display()
            );
            let matches = settings.version_name.is_none()
                && path_matches_any(path, versions.iter().map(|v| v.path.as_str()));
            matches.then_some(path)
        }
        Err(err) => {
            debug!("Could not list installed versions: {}", err);
            None
        }
    }
}

fn path_matches_any<'a>(path: &Path, version_paths: impl IntoIterator<Item = &'a str>) -> bool {
    let Some(provided_path) =
        idf_im_lib::utils::normalize_path_for_comparison(&path.to_string_lossy())
    else {
        return false;
    };
    version_paths.into_iter().any(|version_path_raw| {
        let version_path = idf_im_lib::utils::normalize_path_for_comparison(version_path_raw);
        debug!(
            "Normalized version_path for '{}': {:?}",
            version_path_raw, version_path
        );
        version_path.is_some_and(|p| p == provided_path)
    })
}

/// Returns `None` when there are no installations to choose from.
fn resolve_path_to_fix(
    path: Option<String>,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = path {
        // If a path is provided, fix the IDF installation at that path
        if is_valid_idf_directory(&path) {
            return Ok(Some(PathBuf::from(path)));
        }
        error!("{}", t!("fix.invalid_directory", path = path));
        return Err(anyhow::anyhow!(t!("fix.invalid_directory", path = path)));
    }
    let versions = match idf_im_lib::version_manager::list_installed_versions(config_path) {
        Ok(versions) => versions,
        Err(err) => {
            debug!("Error: {}", err);
            return Err(anyhow::anyhow!(t!("fix.no_versions_found")));
        }
    };
    if versions.is_empty() {
        warn!("{}", t!("fix.no_versions"));
        return Ok(None);
    }
    let options: Vec<String> = versions
        .iter()
        .map(|v| fix_option_label(&v.name, &v.path, &status_label(&v.status)))
        .collect();
    match helpers::generic_select_index(&t!("fix.prompt"), &options) {
        Ok(i) => Ok(Some(PathBuf::from(versions[i].path.clone()))),
        Err(err) => {
            error!("Error: {}", err);
            Err(anyhow::anyhow!(err))
        }
    }
}

fn fix_option_label(name: &str, path: &str, status: &str) -> String {
    format!("{} ({}) [{}]", name, path, status)
}

async fn run_tracked_install(
    settings: Settings,
    mode: InstallMode,
    wizard_result: String,
    do_not_track: bool,
) -> anyhow::Result<()> {
    let ctx = build_cli_context(&settings, mode);
    let extras = build_cli_extras(&settings);
    if !do_not_track {
        telemetry::track_install_started(&ctx);
    }
    match wizard::run_wizard_run(settings).await {
        Ok(_r) => {
            info!("{}", wizard_result);
            info!("{}", t!("install.success"));
            info!("{}", t!("install.ready"));
            if !do_not_track {
                telemetry::track_install_outcome(&ctx, InstallOutcome::Success, None, None, extras);
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
    let used_existing_idf = settings
        .path
        .as_ref()
        .and_then(|p| is_valid_idf_directory(p.to_str().unwrap_or_default()).then_some(true));
    OutcomeExtras {
        feature_count: selection_count(
            settings.idf_features.as_ref(),
            settings.idf_features_per_version.as_ref(),
        ),
        tool_count: selection_count(
            settings.idf_tools.as_ref(),
            settings.idf_tools_per_version.as_ref(),
        ),
        target_count: settings.target.as_ref().map(|v| v.len()),
        non_interactive: settings.non_interactive,
        used_existing_idf,
    }
}

/// Counts a global selection, falling back to the sum of the per-version selections.
fn selection_count(
    global: Option<&Vec<String>>,
    per_version: Option<&HashMap<String, Vec<String>>>,
) -> Option<usize> {
    global
        .map(|v| v.len())
        .or_else(|| per_version.map(|m| m.values().map(|v| v.len()).sum()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn selection_count_prefers_global_list() {
        let global = strings(&["a", "b"]);
        let per_version = HashMap::from([("v5.4".to_string(), strings(&["x", "y", "z"]))]);
        assert_eq!(selection_count(Some(&global), Some(&per_version)), Some(2));
    }

    #[test]
    fn selection_count_sums_per_version_lists() {
        let per_version = HashMap::from([
            ("v5.4".to_string(), strings(&["x", "y"])),
            ("v5.3".to_string(), strings(&["z"])),
        ]);
        assert_eq!(selection_count(None, Some(&per_version)), Some(3));
    }

    #[test]
    fn selection_count_is_none_without_selections() {
        assert_eq!(selection_count(None, None), None);
    }

    #[test]
    fn build_cli_extras_counts_settings() {
        let settings = Settings {
            idf_features: Some(strings(&["a"])),
            idf_features_per_version: None,
            idf_tools_per_version: Some(HashMap::from([(
                "v5.4".to_string(),
                strings(&["t1", "t2"]),
            )])),
            target: Some(strings(&["esp32", "esp32s3", "esp32c6"])),
            non_interactive: Some(true),
            idf_tools: None,
            path: None,
            ..Settings::default()
        };
        let extras = build_cli_extras(&settings);
        assert_eq!(extras.feature_count, Some(1));
        assert_eq!(extras.tool_count, Some(2));
        assert_eq!(extras.target_count, Some(3));
        assert_eq!(extras.non_interactive, Some(true));
        assert_eq!(extras.used_existing_idf, None);
    }

    #[test]
    fn fix_option_label_includes_name_path_and_status() {
        assert_eq!(
            fix_option_label("v5.4", "/esp/v5.4", "OK"),
            "v5.4 (/esp/v5.4) [OK]"
        );
    }

    #[test]
    fn path_matches_any_finds_same_directory() {
        let dir = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let dir_str = dir.path().to_string_lossy().to_string();
        let with_slash = format!("{}/", dir_str);
        let other_str = other.path().to_string_lossy().to_string();
        assert!(path_matches_any(
            dir.path(),
            [other_str.as_str(), with_slash.as_str()]
        ));
        assert!(!path_matches_any(dir.path(), [other_str.as_str()]));
    }

    #[test]
    fn path_matches_any_is_false_for_missing_paths() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("missing");
        let missing_str = missing.to_string_lossy().to_string();
        assert!(!path_matches_any(&missing, [missing_str.as_str()]));
    }

    #[cfg(feature = "gui")]
    #[test]
    fn gui_log_level_follows_verbosity() {
        assert_eq!(gui_log_level(0), log::LevelFilter::Info);
        assert_eq!(gui_log_level(1), log::LevelFilter::Debug);
        assert_eq!(gui_log_level(2), log::LevelFilter::Trace);
        assert_eq!(gui_log_level(9), log::LevelFilter::Trace);
    }
}
