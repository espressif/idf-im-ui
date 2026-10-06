use std::path::PathBuf;

use crate::cli::helpers::{
    first_defaulted_multiselect, generic_confirm, generic_input, generic_select, run_with_spinner,
};
use dialoguer::theme::ColorfulTheme;
use dialoguer::MultiSelect;
use idf_im_lib::idf_features::FeatureInfo;
use idf_im_lib::idf_tools::ToolsFile;
use idf_im_lib::system_dependencies;
use idf_im_lib::tool_selection::{
    get_optional_tools, get_required_tools, get_tools_for_selection, ToolSelectionInfo,
};
use idf_im_lib::utils::calculate_mirrors_latency;
use idf_im_lib::{idf_features::RequirementsMetadata, settings::Settings};
use log::{debug, info};
use rust_i18n::t;

use crate::cli::helpers::generic_confirm_with_default;

pub async fn select_target() -> Result<Vec<String>, String> {
    let mut available_targets = idf_im_lib::idf_versions::get_available_targets().await?;
    available_targets.insert(0, "all".to_string());
    first_defaulted_multiselect("wizard.select_target.prompt", &available_targets)
}

pub async fn select_idf_version(
    target: &str,
    non_interactive: bool,
) -> Result<Vec<String>, String> {
    let mut available_versions = if target == "all" {
        //todo process vector of targets
        // in non-interactive mode, we want to skip pre-releases
        idf_im_lib::idf_versions::get_idf_names(!non_interactive).await
    } else {
        // in non-interactive mode, we want to skip pre-releases
        idf_im_lib::idf_versions::get_idf_name_by_target(
            &target.to_string().to_lowercase(),
            !non_interactive,
        )
        .await
    };
    available_versions.push("master".to_string());
    if non_interactive {
        debug!("{}", t!("noninteractive.default"));
        Ok(vec![available_versions.first().unwrap().clone()])
    } else {
        first_defaulted_multiselect("wizard.select_idf_version.prompt", &available_versions)
    }
}

#[derive(Debug, PartialEq)]
enum InstallChoice {
    Prompt,
    Install,
    Skip,
}

fn install_choice(install_all_prerequisites: bool, non_interactive: bool) -> InstallChoice {
    if install_all_prerequisites {
        InstallChoice::Install
    } else if non_interactive {
        InstallChoice::Skip
    } else {
        InstallChoice::Prompt
    }
}

fn ask_to_skip_prerequisites() -> Result<(), String> {
    let skip = generic_confirm("prerequisites.skip_prompt").map_err(|e| e.to_string())?;
    if !skip {
        return Err(t!("prerequisites.user_cancelled").to_string());
    }
    info!("{}", t!("prerequisites.skipping"));
    Ok(())
}

pub async fn check_and_install_prerequisites(
    non_interactive: bool,
    install_all_prerequisites: bool,
    tools_dir: PathBuf,
) -> Result<(), String> {
    // Run the prerequisites check
    let check_result = if non_interactive {
        system_dependencies::check_prerequisites_with_result()
    } else {
        run_with_spinner(system_dependencies::check_prerequisites_with_result)
    };

    let result = match check_result {
        Ok(result) => result,
        Err(err) => {
            // Error during checking (e.g., unsupported package manager)
            info!(
                "{}",
                t!("prerequisites.verification_error", error = err.clone())
            );
            if non_interactive {
                return Err(err);
            }
            return ask_to_skip_prerequisites();
        }
    };

    // Handle verification failures (shell failed or can't verify)
    if result.shell_failed || !result.can_verify {
        let message = if result.shell_failed {
            t!("prerequisites.shell_failed").to_string()
        } else {
            t!("prerequisites.verification_error", error = "unknown").to_string()
        };
        info!("{}", message);

        if non_interactive {
            return Err(t!("prerequisites.failed").to_string());
        }
        return ask_to_skip_prerequisites();
    }

    if result.missing.is_empty() {
        info!("{}", t!("prerequisites.ok"));
        return Ok(());
    }

    let unsatisfied_prerequisites: Vec<String> =
        result.missing.into_iter().map(|p| p.to_string()).collect();

    info!(
        "{} {:?}",
        t!("prerequisites.missing"),
        unsatisfied_prerequisites
    );
    info!(
        "{}",
        t!(
            "prerequisites.not_ok",
            l = unsatisfied_prerequisites.join(", ")
        )
    );

    if std::env::consts::OS != "windows" {
        return Err(t!("prerequisites.install.ask").to_string());
    }
    let install = match install_choice(install_all_prerequisites, non_interactive) {
        InstallChoice::Prompt => {
            generic_confirm("prerequisites.install.prompt").map_err(|e| e.to_string())?
        }
        InstallChoice::Install => true,
        InstallChoice::Skip => false,
    };
    if !install {
        return Err(t!("prerequisites.install.ask").to_string());
    }
    install_and_recheck_prerequisites(unsatisfied_prerequisites, tools_dir).await
}

async fn install_and_recheck_prerequisites(
    unsatisfied_prerequisites: Vec<String>,
    tools_dir: PathBuf,
) -> Result<(), String> {
    system_dependencies::install_prerequisites(unsatisfied_prerequisites, tools_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Re-check after installation to verify prerequisites were installed
    let recheck_result = run_with_spinner(system_dependencies::check_prerequisites_with_result)?;
    if !recheck_result.missing.is_empty() {
        return Err(t!(
            "prerequisites.install.catastrophic",
            l = recheck_result
                .missing
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .to_string());
    }
    info!("{}", t!("prerequisites.ok"));
    Ok(())
}

fn python_sanity_check(python: Option<&str>, offline: bool) -> Result<(), String> {
    let results = idf_im_lib::python_utils::python_sanity_check(python, offline);
    let mut all_ok = true;
    for result in &results {
        let name = t!(result.check.display_key()).to_string();
        if result.passed {
            debug!("[PASS] {}: {}", name, result.message);
            println!("  [PASS] {}", name);
        } else {
            all_ok = false;
            debug!("[FAIL] {}: {}", name, result.message);
            println!("  [FAIL] {}", name);
            println!(
                "         Hint: {}",
                t!(result.check.hint_key_for_os(std::env::consts::OS))
            );
        }
    }
    if all_ok {
        debug!("{}", t!("debug.python_sanity_check"));
        Ok(())
    } else {
        // Per-check [FAIL] lines with hints were already printed above.
        // Return a short error to signal failure without repeating advice.
        Err(t!("python.sanitycheck.fail").to_string())
    }
}
pub async fn check_and_install_python(
    non_interactive: bool,
    install_all_prerequisites: bool,
    python_version_override: Option<String>,
    offline: bool,
    tools_dir: PathBuf,
) -> Result<(), String> {
    info!("{}", t!("python.sanitycheck.info"));
    let check_result = if non_interactive {
        python_sanity_check(None, offline)
    } else {
        run_with_spinner(|| python_sanity_check(None, offline))
    };
    if check_result.is_ok() {
        info!("{}", t!("python.sanitycheck.ok"));
        return Ok(());
    }
    if std::env::consts::OS != "windows" {
        // Details were already printed per-check — just signal the failure.
        return Err(t!("python.sanitycheck.fail").to_string());
    }
    let install = match install_choice(install_all_prerequisites, non_interactive) {
        InstallChoice::Prompt => {
            generic_confirm("python.install.prompt").map_err(|e| e.to_string())?
        }
        InstallChoice::Install => {
            info!("{}", t!("python.sanitycheck.fail_but_will_install"));
            true
        }
        InstallChoice::Skip => {
            info!("{}", t!("python.sanitycheck.fail"));
            false
        }
    };
    if !install {
        return Err(t!("python.install.refuse").to_string());
    }
    install_and_recheck_python(python_version_override, offline, tools_dir).await
}

async fn install_and_recheck_python(
    python_version_override: Option<String>,
    offline: bool,
    tools_dir: PathBuf,
) -> Result<(), String> {
    system_dependencies::install_prerequisites(
        vec![python_version_override.unwrap_or_else(|| {
            idf_im_lib::system_dependencies::PYTHON_NAME_TO_INSTALL.to_string()
        })],
        tools_dir.clone(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let usable_python = tools_dir
        .join("python")
        .join("python.exe")
        .to_str()
        .ok_or_else(|| t!("error.path_to_string").to_string())?
        .to_string();
    debug!("{}", t!("debug.using_python", path = usable_python));
    match run_with_spinner(|| python_sanity_check(Some(&usable_python), offline)) {
        Ok(_) => {
            info!("{}", t!("python.install.success"));
            Ok(())
        }
        Err(err) => Err(format!("{} {:?}", t!("python.install.failure"), err)),
    }
}

struct MirrorField {
    field_name: &'static str,
    field: fn(&mut Settings) -> &mut Option<String>,
    candidates: fn() -> &'static [&'static str],
    wizard_key: &'static str,
    log_prefix: &'static str,
}

const MIRROR_FIELDS: [MirrorField; 3] = [
    MirrorField {
        field_name: "idf_mirror",
        field: |c| &mut c.idf_mirror,
        candidates: idf_im_lib::get_idf_mirrors_list,
        wizard_key: "wizard.idf.mirror",
        log_prefix: "IDF",
    },
    MirrorField {
        field_name: "mirror",
        field: |c| &mut c.mirror,
        candidates: idf_im_lib::get_idf_tools_mirrors_list,
        wizard_key: "wizard.tools.mirror",
        log_prefix: "Tools",
    },
    MirrorField {
        field_name: "pypi_mirror",
        field: |c| &mut c.pypi_mirror,
        candidates: idf_im_lib::get_pypi_mirrors_list,
        wizard_key: "wizard.pypi.mirror",
        log_prefix: "PyPI",
    },
];

async fn select_single_mirror(config: &mut Settings, mirror: &MirrorField) -> Result<(), String> {
    let MirrorField {
        field_name,
        field,
        wizard_key,
        log_prefix,
        ..
    } = *mirror;
    let candidates = (mirror.candidates)();
    // Interactive by default when non_interactive is None
    let interactive = !config.non_interactive.unwrap_or_default();
    let wizard_all = config.wizard_all_questions.unwrap_or_default();
    let needs_value = field(config).is_none() || config.is_default(field_name);

    // Only measure mirror latency if we actually need a value (or wizard wants to ask)
    if interactive && (wizard_all || needs_value) {
        let entries = calculate_mirrors_latency(candidates).await;
        let display = entries
            .iter()
            .map(|e| match e.latency {
                Some(latency) => format!("{} ({latency:?} ms)", e.url),
                None => format!("{} (timeout)", e.url),
            })
            .collect::<Vec<String>>();
        let selected = generic_select(wizard_key, &display)?;
        let url = selected.split(" (").next().unwrap_or(&selected).to_string();
        *field(config) = Some(url);
    } else if needs_value && config.config_file.is_none() {
        // Only auto-select based on latency if no config file was loaded
        // This prevents overriding user's mirror selection from GUI/config file
        let entries = calculate_mirrors_latency(candidates).await;
        if let Some(entry) = entries.first() {
            if let Some(latency) = entry.latency {
                // The first entry is best mirror to select
                info!("Selected {log_prefix} mirror: {}", entry.url);
                debug!("Selected {log_prefix} mirror latency: {latency:?} ms");
                *field(config) = Some(entry.url.clone());
            }
        } else {
            // If the first entry is timeout or None there are no good mirrors to select try logging a proper message and return an error
            info!("No good {log_prefix} mirrors found, please check your internet connection and try again");
            return Err(format!("No good {log_prefix} mirrors found, please check your internet connection and try again"));
        }
    }

    Ok(())
}

pub async fn select_mirrors(mut config: Settings) -> Result<Settings, String> {
    for mirror in &MIRROR_FIELDS {
        select_single_mirror(&mut config, mirror).await?;
    }
    Ok(config)
}

pub fn select_installation_path(mut config: Settings) -> Result<Settings, String> {
    if (config.wizard_all_questions.unwrap_or_default()
        || config.path.is_none()
        || config.is_default("path"))
        && config.non_interactive == Some(false)
    {
        let path = match generic_input(
            "wizard.installation_path.prompt",
            "wizard.installation_path.unselected",
            config.path.clone().unwrap_or_default().to_str().unwrap(),
        ) {
            Ok(path) => PathBuf::from(path),
            Err(e) => {
                log::error!("Error: {}", e);
                config.path.clone().unwrap_or_default()
            }
        };
        config.path = Some(path);
    }

    Ok(config)
}

pub fn save_config_if_desired(config: &Settings) -> Result<(), String> {
    let res =
        if config.non_interactive.unwrap_or_default() && config.config_file_save_path.is_some() {
            debug!("{}", t!("debug.non_interactive_save"));
            Ok(true)
        } else if config.non_interactive.unwrap_or_default() {
            debug!("{}", t!("debug.skip_save"));
            Ok(false)
        } else {
            generic_confirm_with_default("wizard.after_install.save_config.prompt", true)
        };
    if let Ok(true) = res {
        config
            .save()
            .map_err(|e| format!("{} {:?}", t!("wizard.after_install.config.save_failed"), e))?;
        println!("{}", t!("wizard.after_install.config.saved"));
    }
    Ok(())
}

/// Select features from requirements metadata with interactive or non-interactive mode
///
/// # Arguments
/// * `metadata` - The requirements metadata containing available features
/// * `non_interactive` - If true, returns all required features by default
/// * `include_optional` - If true, allows selection of optional features (interactive mode only)
///
/// # Returns
/// * `Ok(Vec<FeatureInfo>)` - Selected features
/// * `Err(String)` - Error message
pub fn select_features(
    metadata: &RequirementsMetadata,
    non_interactive: bool,
    include_optional: bool,
) -> Result<Vec<FeatureInfo>, String> {
    if non_interactive {
        // Non-interactive mode: return all required features
        println!("Non-interactive mode: selecting all required features by default");
        let required = metadata.required_features().into_iter().cloned().collect();
        Ok(required)
    } else {
        // Interactive mode: let user select features
        select_features_interactive(metadata, include_optional)
    }
}

/// Features from `metadata` whose names are listed in `names`, in metadata order
pub fn features_matching_names(
    metadata: &RequirementsMetadata,
    names: &[String],
) -> Vec<FeatureInfo> {
    metadata
        .features
        .iter()
        .filter(|f| names.contains(&f.name))
        .cloned()
        .collect()
}

fn describe_item(name: &str, description: Option<&str>) -> String {
    format!("{} - {}", name, description.unwrap_or("No description"))
}

/// Helper function to get features for a specific version
/// Handles both per-version features (GUI) and global features (CLI)
pub fn get_features_for_version(
    config: &Settings,
    version: &str,
    requirements_files: &RequirementsMetadata,
) -> Result<Vec<FeatureInfo>, String> {
    // First check if we have per-version features (from GUI)
    if let Some(per_version) = &config.idf_features_per_version {
        if let Some(feature_names) = per_version.get(version) {
            return Ok(features_matching_names(requirements_files, feature_names));
        }
    }

    // Fall back to global idf_features (from CLI)
    if let Some(global_features) = &config.idf_features {
        return Ok(features_matching_names(requirements_files, global_features));
    }

    // If no features specified, use interactive selection (CLI) or return required only
    if config.non_interactive.unwrap_or_default() {
        // Non-interactive: return only required features
        Ok(requirements_files
            .features
            .iter()
            .filter(|f| !f.optional)
            .cloned()
            .collect())
    } else {
        // Interactive: prompt user
        select_features(
            requirements_files,
            config.non_interactive.unwrap_or_default(),
            true,
        )
    }
}
/// Interactive feature selection with multi-select dialog
fn select_features_interactive(
    metadata: &RequirementsMetadata,
    include_optional: bool,
) -> Result<Vec<FeatureInfo>, String> {
    let features_to_show: Vec<&FeatureInfo> = if include_optional {
        metadata.features.iter().collect()
    } else {
        metadata.required_features()
    };

    if features_to_show.is_empty() {
        return Err("No features available for selection".to_string());
    }

    // Create display strings for each feature
    let items: Vec<String> = features_to_show
        .iter()
        .map(|f| describe_item(&f.name, f.description.as_deref()))
        .collect();

    // Pre-select all required features
    let defaults: Vec<bool> = features_to_show.iter().map(|f| !f.optional).collect();

    // Show multi-select dialog
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select ESP-IDF features to install (Space to toggle, Enter to confirm)")
        .items(&items)
        .defaults(&defaults)
        .interact()
        .map_err(|e| format!("Selection failed: {}", e))?;

    if selections.is_empty() {
        return Err("No features selected. At least one feature must be selected.".to_string());
    }

    // Return selected features
    let selected_features: Vec<FeatureInfo> = selections
        .into_iter()
        .map(|idx| features_to_show[idx].clone())
        .collect();

    Ok(selected_features)
}

/// Select features and return their names only
pub fn select_feature_names(
    metadata: &RequirementsMetadata,
    non_interactive: bool,
    include_optional: bool,
) -> Result<Vec<String>, String> {
    select_features_mapped(metadata, non_interactive, include_optional, |f| f.name)
}

/// Select features and return their requirement paths
pub fn select_requirement_paths(
    metadata: &RequirementsMetadata,
    non_interactive: bool,
    include_optional: bool,
) -> Result<Vec<String>, String> {
    select_features_mapped(metadata, non_interactive, include_optional, |f| {
        f.requirement_path
    })
}

fn select_features_mapped(
    metadata: &RequirementsMetadata,
    non_interactive: bool,
    include_optional: bool,
    map: fn(FeatureInfo) -> String,
) -> Result<Vec<String>, String> {
    let features = select_features(metadata, non_interactive, include_optional)?;
    Ok(features.into_iter().map(map).collect())
}

/// Advanced selection: filter by specific criteria
pub struct FeatureSelectionOptions {
    pub non_interactive: bool,
    pub include_optional: bool,
    pub show_only_optional: bool,
    pub filter_by_name: Option<Vec<String>>,
}

impl Default for FeatureSelectionOptions {
    fn default() -> Self {
        Self {
            non_interactive: false,
            include_optional: true,
            show_only_optional: false,
            filter_by_name: None,
        }
    }
}

/// Advanced feature selection with filtering options
pub fn select_features_advanced(
    metadata: &RequirementsMetadata,
    options: FeatureSelectionOptions,
) -> Result<Vec<FeatureInfo>, String> {
    // Apply filters
    let mut filtered_features: Vec<&FeatureInfo> = metadata.features.iter().collect();

    // Filter by optional/required
    if options.show_only_optional {
        filtered_features.retain(|f| f.optional);
    } else if !options.include_optional {
        filtered_features.retain(|f| !f.optional);
    }

    // Filter by name if specified
    if let Some(ref names) = options.filter_by_name {
        filtered_features.retain(|f| names.contains(&f.name));
    }

    if filtered_features.is_empty() {
        return Err("No features match the specified criteria".to_string());
    }

    if options.non_interactive {
        // Return all filtered features in non-interactive mode
        println!(
            "Non-interactive mode: selecting {} filtered feature(s)",
            filtered_features.len()
        );
        Ok(filtered_features.into_iter().cloned().collect())
    } else {
        // Interactive selection from filtered features
        let items: Vec<String> = filtered_features
            .iter()
            .map(|f| {
                format!(
                    "{} {} - {}",
                    if f.optional { "[ ]" } else { "[*]" },
                    f.name,
                    f.description.as_deref().unwrap_or("No description")
                )
            })
            .collect();

        let defaults: Vec<bool> = filtered_features.iter().map(|f| !f.optional).collect();

        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Select ESP-IDF features (Space to toggle, Enter to confirm)")
            .items(&items)
            .defaults(&defaults)
            .interact()
            .map_err(|e| format!("Selection failed: {}", e))?;

        if selections.is_empty() {
            return Err("No features selected".to_string());
        }

        Ok(selections
            .into_iter()
            .map(|idx| filtered_features[idx].clone())
            .collect())
    }
}

/// Select tools interactively (CLI)
pub fn select_tools_interactive(
    tools: &[ToolSelectionInfo],
    pre_selected: Option<&[String]>,
) -> Result<Vec<String>, String> {
    use dialoguer::{theme::ColorfulTheme, MultiSelect};

    let optional_tools: Vec<&ToolSelectionInfo> = get_optional_tools(tools);
    let required_tools: Vec<&ToolSelectionInfo> = get_required_tools(tools);

    if optional_tools.is_empty() && required_tools.is_empty() {
        return Err("No tools available for selection".to_string());
    }

    // Always include required tools
    let mut selected: Vec<String> = required_tools.iter().map(|t| t.name.clone()).collect();

    if optional_tools.is_empty() {
        info!(
            "No optional tools available. Using {} required tools.",
            selected.len()
        );
        return Ok(selected);
    }

    // Create display strings for optional tools
    let items: Vec<String> = optional_tools
        .iter()
        .map(|t| describe_item(&t.name, t.description.as_deref()))
        .collect();

    // Determine defaults based on pre_selected or default to none
    let defaults: Vec<bool> = optional_tools
        .iter()
        .map(|t| pre_selected.map(|ps| ps.contains(&t.name)).unwrap_or(false))
        .collect();

    println!("\nRequired tools (will be installed automatically):");
    for tool in &required_tools {
        println!(
            "  [*] {} - {}",
            tool.name,
            tool.description.as_deref().unwrap_or("")
        );
    }
    println!();

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select additional tools to install (Space to toggle, Enter to confirm)")
        .items(&items)
        .defaults(&defaults)
        .interact()
        .map_err(|e| format!("Selection failed: {}", e))?;

    // Add selected optional tools
    for idx in selections {
        selected.push(optional_tools[idx].name.clone());
    }

    Ok(selected)
}

/// Select tools non-interactively (return required tools only, or all if specified)
pub fn select_tools_non_interactive(
    tools: &[ToolSelectionInfo],
    include_optional: bool,
) -> Vec<String> {
    if include_optional {
        tools.iter().map(|t| t.name.clone()).collect()
    } else {
        get_required_tools(tools)
            .iter()
            .map(|t| t.name.clone())
            .collect()
    }
}

/// Select tools - checks for existing selection first, then falls back to interactive/non-interactive
/// This mirrors the pattern used for feature selection
pub fn select_tools(
    tools_file: &ToolsFile,
    non_interactive: bool,
    include_optional: bool,
    targets: Option<&[String]>,
    existing_selection: Option<&[String]>,
) -> Result<Vec<ToolSelectionInfo>, String> {
    let available = get_tools_for_selection(tools_file, targets).map_err(|e| e.to_string())?;

    if available.is_empty() {
        return Err("No tools available for selection".to_string());
    }

    // If we have existing selection, convert tool names back to ToolSelectionInfo
    if let Some(existing) = existing_selection {
        let selected: Vec<ToolSelectionInfo> = available
            .iter()
            .filter(|t| existing.contains(&t.name) || t.install == "always")
            .cloned()
            .collect();
        return Ok(selected);
    }

    // No existing selection - do interactive or non-interactive selection
    if non_interactive {
        // Non-interactive mode: return required tools, optionally include all
        info!(
            "Non-interactive mode: selecting {} tools by default (QEMU excluded)",
            if include_optional { "all" } else { "required" }
        );
        let selected: Vec<ToolSelectionInfo> = if include_optional {
            available
                .into_iter()
                .filter(|t| !t.name.contains("qemu"))
                .collect()
        } else {
            available
                .into_iter()
                .filter(|t| t.install == "always")
                .collect()
        };
        Ok(selected)
    } else {
        // Interactive mode: prompt user
        let selected_names = select_tools_interactive(&available, None)?;
        let selected: Vec<ToolSelectionInfo> = available
            .into_iter()
            .filter(|t| selected_names.contains(&t.name))
            .collect();
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tool(name: &str, install: &str) -> ToolSelectionInfo {
        ToolSelectionInfo {
            name: name.to_string(),
            install: install.to_string(),
            editable: install == "on_request",
            description: None,
            supported_targets: None,
        }
    }

    fn feature(name: &str, optional: bool) -> FeatureInfo {
        FeatureInfo {
            name: name.to_string(),
            description: None,
            optional,
            requirement_path: format!("{}.txt", name),
        }
    }

    #[test]
    fn install_choice_prefers_install_all_then_non_interactive() {
        assert_eq!(install_choice(true, true), InstallChoice::Install);
        assert_eq!(install_choice(true, false), InstallChoice::Install);
        assert_eq!(install_choice(false, true), InstallChoice::Skip);
        assert_eq!(install_choice(false, false), InstallChoice::Prompt);
    }

    #[test]
    fn describe_item_falls_back_to_no_description() {
        assert_eq!(describe_item("core", Some("Core")), "core - Core");
        assert_eq!(describe_item("core", None), "core - No description");
    }

    #[test]
    fn features_matching_names_keeps_metadata_order() {
        let metadata = RequirementsMetadata {
            version: 1,
            features: vec![
                feature("core", false),
                feature("gdb", true),
                feature("ci", true),
            ],
        };
        let selected = features_matching_names(&metadata, &["ci".to_string(), "core".to_string()]);
        let names: Vec<&str> = selected.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["core", "ci"]);
        assert!(features_matching_names(&metadata, &[]).is_empty());
    }

    #[test]
    fn mirror_fields_are_asked_in_order_and_map_to_settings() {
        let keys: Vec<&str> = MIRROR_FIELDS.iter().map(|m| m.wizard_key).collect();
        assert_eq!(
            keys,
            vec![
                "wizard.idf.mirror",
                "wizard.tools.mirror",
                "wizard.pypi.mirror"
            ]
        );
        let mut config = Settings::default();
        for mirror in &MIRROR_FIELDS {
            *(mirror.field)(&mut config) = Some(mirror.field_name.to_string());
        }
        assert_eq!(config.idf_mirror.as_deref(), Some("idf_mirror"));
        assert_eq!(config.mirror.as_deref(), Some("mirror"));
        assert_eq!(config.pypi_mirror.as_deref(), Some("pypi_mirror"));
        assert_eq!(
            (MIRROR_FIELDS[0].candidates)(),
            idf_im_lib::get_idf_mirrors_list()
        );
        assert_eq!(
            (MIRROR_FIELDS[2].candidates)(),
            idf_im_lib::get_pypi_mirrors_list()
        );
    }

    #[test]
    fn test_select_tools_non_interactive() {
        let tools = vec![
            create_test_tool("required1", "always"),
            create_test_tool("optional1", "on_request"),
            create_test_tool("required2", "always"),
        ];

        // Without optional
        let selected = select_tools_non_interactive(&tools, false);
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&"required1".to_string()));
        assert!(selected.contains(&"required2".to_string()));

        // With optional
        let selected = select_tools_non_interactive(&tools, true);
        assert_eq!(selected.len(), 3);
    }
}
