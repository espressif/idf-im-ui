use std::path::PathBuf;

use crate::cli::helpers::{
    first_defaulted_multiselect, generic_confirm, generic_input, generic_select, run_with_spinner,
};
use dialoguer::theme::ColorfulTheme;
use dialoguer::MultiSelect;
use idf_im_lib::idf_features::FeatureInfo;
use idf_im_lib::{idf_features::RequirementsMetadata, settings::Settings};
use idf_im_lib::system_dependencies;
use log::{debug, info};
use rust_i18n::t;
use idf_im_lib::utils::calculate_mirrors_latency;
use crate::cli::helpers::generic_confirm_with_default;


pub async fn select_target() -> Result<Vec<String>, String> {
    let mut available_targets = idf_im_lib::idf_versions::get_avalible_targets().await?;
    available_targets.insert(0, "all".to_string());
    first_defaulted_multiselect("wizard.select_target.prompt", &available_targets)
}

pub async fn select_idf_version(
    target: &str,
    non_interactive: bool,
) -> Result<Vec<String>, String> {
    let mut avalible_versions = if target == "all" {
        //todo process vector of targets
        // in non-interactive mode, we want to skip pre-releases
        idf_im_lib::idf_versions::get_idf_names(!non_interactive).await
    } else {
        // in non-interactive mode, we want to skip pre-releases
        idf_im_lib::idf_versions::get_idf_name_by_target(&target.to_string().to_lowercase(),!non_interactive).await
    };
    avalible_versions.push("master".to_string());
    if non_interactive {
        debug!("{}", t!("noninteractive.default"));
        Ok(vec![avalible_versions.first().unwrap().clone()])
    } else {
        first_defaulted_multiselect("wizard.select_idf_version.prompt", &avalible_versions)
    }
}

fn check_prerequisites() -> Result<Vec<String>, String> {
    match system_dependencies::check_prerequisites() {
        Ok(prerequisites) => {
            if prerequisites.is_empty() {
                debug!("{}", t!("prerequisites.ok"));
                Ok(vec![])
            } else {
                info!("{} {:?}", t!("prerequisites.missing"), prerequisites);
                Ok(prerequisites.into_iter().map(|p| p.to_string()).collect())
            }
        }
        Err(err) => Err(err),
    }
}
pub fn check_and_install_prerequisites(
    non_interactive: bool,
    install_all_prerequisites: bool,
) -> Result<(), String> {
    let unsatisfied_prerequisites = if non_interactive {
        check_prerequisites()?
    } else {
        run_with_spinner(check_prerequisites)?
    };
    if !unsatisfied_prerequisites.is_empty() {
        info!(
            "{}",
            t!(
                "prerequisites.not_ok",
                l = unsatisfied_prerequisites.join(", ")
            )
        );
        if std::env::consts::OS == "windows" {
            let res = if !install_all_prerequisites && !non_interactive {
                generic_confirm("prerequisites.install.prompt")
            } else if install_all_prerequisites {
                Ok(true)
            } else {
                Ok(false)
            };
            if res.map_err(|e| e.to_string())? {
                system_dependencies::install_prerequisites(unsatisfied_prerequisites)
                    .map_err(|e| e.to_string())?;

                let remaining_prerequisites = run_with_spinner(check_prerequisites)?;
                if !remaining_prerequisites.is_empty() {
                    return Err(format!(
                        "{}",
                        t!(
                            "prerequisites.install.catastrophic",
                            l = remaining_prerequisites.join(", ")
                        ),
                    ));
                } else {
                    info!("{}", t!("prerequisites.ok"));
                }
            } else {
                return Err(t!("prerequisites.install.ask").to_string());
            }
        } else {
            return Err(t!("prerequisites.install.ask").to_string());
        }
    } else {
        info!("{}", t!("prerequisites.ok"))
    }

    Ok(())
}

fn python_sanity_check(python: Option<&str>) -> Result<(), String> {
    let outpusts = idf_im_lib::python_utils::python_sanity_check(python);
    let mut all_ok = true;
    for output in outpusts {
        match output {
            Ok(_) => {}
            Err(err) => {
                all_ok = false;
                println!("{:?}", err)
            }
        }
    }
    if all_ok {
        debug!("{}", t!("debug.python_sanity_check"));
        Ok(())
    } else {
        Err(t!("python.sanitycheck.fail").to_string())
    }
}
pub fn check_and_install_python(
    non_interactive: bool,
    install_all_prerequisites: bool,
    python_version_override: Option<String>,
) -> Result<(), String> {
    info!("{}", t!("python.sanitycheck.info"));
    let check_result = if non_interactive {
        python_sanity_check(None)
    } else {
        run_with_spinner(|| python_sanity_check(None))
    };
    if let Err(err) = check_result {
        if std::env::consts::OS == "windows" {
            let res = if !install_all_prerequisites && !non_interactive {
                generic_confirm("python.install.prompt")
            } else if install_all_prerequisites {
                info!("{}", t!("python.sanitycheck.fail_but_will_install"));
                Ok(true)
            } else {
                info!("{}", t!("python.sanitycheck.fail"));
                Ok(false)
            };

            if res.map_err(|e| e.to_string())? {
                system_dependencies::install_prerequisites(vec![python_version_override.unwrap_or_else(|| idf_im_lib::system_dependencies::PYTHON_NAME_TO_INSTALL.to_string())])
                    .map_err(|e| e.to_string())?;
                let scp = system_dependencies::get_scoop_path();
                let usable_python = match scp {
                    Some(path) => {
                        let mut python_path = PathBuf::from(path);
                        python_path.push("python3.exe");
                        python_path
                            .to_str()
                            .map(|s| s.to_string())
                            .ok_or_else(|| t!("error.path_to_string").to_string())?
                    }
                    None => "python3.exe".to_string(),
                };
                debug!("{}", t!("debug.using_python", path = usable_python));
                match run_with_spinner(|| python_sanity_check(Some(&usable_python))) {
                    Ok(_) => info!("{}", t!("python.install.success")),
                    Err(err) => return Err(format!("{} {:?}", t!("python.install.failure"), err)),
                }
            } else {
                return Err(t!("python.install.refuse").to_string());
            }
        } else {
            return Err(format!("{} {:?}", t!("python.sanitycheck.fail"), err));
        }
    } else {
        info!("{}", t!("python.sanitycheck.ok"))
    }
    Ok(())
}

async fn select_single_mirror<FGet, FSet>(
    config: &mut Settings,
    field_name: &str,    // e.g. "idf_mirror"
    get_value: FGet,     // e.g. |c: &Settings| &c.idf_mirror
    set_value: FSet,     // e.g. |c: &mut Settings, v| c.idf_mirror = Some(v)
    candidates: &[&str], // list of mirror URLs
    wizard_key: &str,    // e.g. "wizard.idf.mirror"
    log_prefix: &str,    // e.g. "IDF", "Tools", "PyPI"
) -> Result<(), String>
where
    FGet: Fn(&Settings) -> &Option<String>,
    FSet: Fn(&mut Settings, String),
{
    // Interactive by default when non_interactive is None
    let interactive = !config.non_interactive.unwrap_or_default();
    let wizard_all = config.wizard_all_questions.unwrap_or_default();
    let current = get_value(config);
    let needs_value = current.is_none() || config.is_default(field_name);

    // Only measure mirror latency if we actually need a value (or wizard wants to ask)
    if interactive && (wizard_all || needs_value) {
        let entries = calculate_mirrors_latency(candidates).await;
        let display = entries.iter().map(|e| {
            if e.latency.is_none() {
                format!("{} (timeout)", e.url)
            } else {
                format!("{} ({:?} ms)", e.url, e.latency.unwrap())
                }
            })
            .collect::<Vec<String>>();
        let selected = generic_select(wizard_key, &display)?;
        let url = selected.split(" (").next().unwrap_or(&selected).to_string();
        set_value(config, url);
    } else if needs_value {
        let entries = calculate_mirrors_latency(candidates).await;
        if let Some(entry) = entries.first() {
            if entry.latency.is_some() {
                // The first entry is best mirror to select
                info!("Selected {log_prefix} mirror: {} ({:?} ms)", entry.url, entry.latency.unwrap());
                set_value(config, entry.url.clone());
            }
        } else {
            // If the first entry is timeout or None there are no good mirrors to select try logging a proper message and return an error
            info!("No good {log_prefix} mirrors found, please check your internet connection and try again§");
            return Err(format!("No good {log_prefix} mirrors found, please check your internet connection and try again"));
        }
    }

    Ok(())
}

pub async fn select_mirrors(mut config: Settings) -> Result<Settings, String> {
    // IDF mirror
    let idf_candidates = idf_im_lib::get_idf_mirrors_list();

    select_single_mirror(
        &mut config,
        "idf_mirror",
        |c: &Settings| &c.idf_mirror,
        |c: &mut Settings, v| c.idf_mirror = Some(v),
        idf_candidates,
        "wizard.idf.mirror",
        "IDF",
    )
    .await?;

    // Tools mirror
    let tools_candidates = idf_im_lib::get_idf_tools_mirrors_list();

    select_single_mirror(
        &mut config,
        "mirror",
        |c: &Settings| &c.mirror,
        |c: &mut Settings, v| c.mirror = Some(v),
        tools_candidates,
        "wizard.tools.mirror",
        "Tools",
    )
    .await?;

    // PyPI mirror
    let pypi_candidates = idf_im_lib::get_pypi_mirrors_list();

    select_single_mirror(
        &mut config,
        "pypi_mirror",
        |c: &Settings| &c.pypi_mirror,
        |c: &mut Settings, v| c.pypi_mirror = Some(v),
        pypi_candidates,
        "wizard.pypi.mirror",
        "PyPI",
    )
    .await?;

    Ok(config)
}

pub fn select_installation_path(mut config: Settings) -> Result<Settings, String> {
    if (config.wizard_all_questions.unwrap_or_default()
        || config.path.is_none()
        || config.is_default("path"))
        && config.non_interactive == Some(false)
    {
        let path = match generic_input(
            "wizard.instalation_path.prompt",
            "wizard.instalation_path.unselected",
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
        let required = metadata
            .required_features()
            .into_iter()
            .cloned()
            .collect();
        Ok(required)
    } else {
        // Interactive mode: let user select features
        select_features_interactive(metadata, include_optional)
    }
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
            // Convert feature names back to FeatureInfo
            let features: Vec<FeatureInfo> = requirements_files.features
                .iter()
                .filter(|f| feature_names.contains(&f.name))
                .cloned()
                .collect();
            return Ok(features);
        }
    }

    // Fall back to global idf_features (from CLI)
    if let Some(global_features) = &config.idf_features {
        let features: Vec<FeatureInfo> = requirements_files.features
            .iter()
            .filter(|f| global_features.contains(&f.name))
            .cloned()
            .collect();
        return Ok(features);
    }

    // If no features specified, use interactive selection (CLI) or return required only
    if config.non_interactive.unwrap_or_default() {
        // Non-interactive: return only required features
        Ok(requirements_files.features
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
        .map(|f| {
            format!(
                "{} - {}",
                f.name,
                f.description.as_deref().unwrap_or("No description")
            )
        })
        .collect();

    // Pre-select all required features
    let defaults: Vec<bool> = features_to_show
        .iter()
        .map(|f| !f.optional)
        .collect();

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
    let features = select_features(metadata, non_interactive, include_optional)?;
    Ok(features.into_iter().map(|f| f.name).collect())
}

/// Select features and return their requirement paths
pub fn select_requirement_paths(
    metadata: &RequirementsMetadata,
    non_interactive: bool,
    include_optional: bool,
) -> Result<Vec<String>, String> {
    let features = select_features(metadata, non_interactive, include_optional)?;
    Ok(features.into_iter().map(|f| f.requirement_path).collect())
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
