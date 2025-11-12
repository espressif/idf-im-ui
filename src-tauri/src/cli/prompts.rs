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
// no runtime creation here; we run inside the app's existing Tokio runtime

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

pub async fn select_mirrors(mut config: Settings) -> Result<Settings, String> {
    // Sort mirrors by latency and produce entries (url, score).
    async fn sorted_entries(mirrors: Vec<String>) -> Vec<(String, u32)> {
        let latency_map = idf_im_lib::utils::calculate_mirror_latency_map(&mirrors).await;
        let mut entries: Vec<(String, u32)> = mirrors
            .into_iter()
            .map(|m| {
                let score = *latency_map.get(&m).unwrap_or(&u32::MAX);
                (m, score)
            })
            .collect();
        entries.sort_by(|a, b| {
            let ascore = if a.1 == u32::MAX { u32::MAX } else { a.1 };
            let bscore = if b.1 == u32::MAX { u32::MAX } else { b.1 };
            ascore.cmp(&bscore)
        });
        entries
    }
    fn entries_to_display(entries: &[(String, u32)]) -> Vec<String> {
        entries
            .iter()
            .map(|(u, s)| {
                if *s == u32::MAX {
                    format!("{} (timeout)", u)
                } else {
                    format!("{} ({} ms)", u, s)
                }
            })
            .collect()
    }

    // IDF mirror
    if config.non_interactive == Some(false)
        && (config.wizard_all_questions.unwrap_or_default()
            || config.idf_mirror.is_none()
            || config.is_default("idf_mirror"))
    {
        let idf_candidates: Vec<String> = idf_im_lib::get_idf_mirrors_list().iter().map(|&s| s.to_string()).collect();
        let entries = sorted_entries(idf_candidates).await;
        let display = entries_to_display(&entries);
        let selected = generic_select("wizard.idf.mirror", &display)?;
        let url = selected
            .split(" (")
            .next()
            .unwrap_or(&selected)
            .to_string();
        config.idf_mirror = Some(url);
    } else if config.idf_mirror.is_none() || config.is_default("idf_mirror") {
        let idf_candidates: Vec<String> = idf_im_lib::get_idf_mirrors_list().iter().map(|&s| s.to_string()).collect();
        let entries = sorted_entries(idf_candidates).await;
        if let Some((url, score)) = entries.first() {
            if *score == u32::MAX {
                info!("Selected IDF mirror: {} (timeout)", url);
            } else {
                info!("Selected IDF mirror: {} ({} ms)", url, score);
            }
            config.idf_mirror = Some(url.clone());
        }
    }

    // Tools mirror
    if config.non_interactive == Some(false)
        && (config.wizard_all_questions.unwrap_or_default()
            || config.mirror.is_none()
            || config.is_default("mirror"))
    {
        let tools_candidates: Vec<String> = idf_im_lib::get_idf_tools_mirrors_list().iter().map(|&s| s.to_string()).collect();
        let entries = sorted_entries(tools_candidates).await;
        let display = entries_to_display(&entries);
        let selected = generic_select("wizard.tools.mirror", &display)?;
        let url = selected
            .split(" (")
            .next()
            .unwrap_or(&selected)
            .to_string();
        config.mirror = Some(url);
    } else if config.mirror.is_none() || config.is_default("mirror") {
        let tools_candidates: Vec<String> = idf_im_lib::get_idf_tools_mirrors_list().iter().map(|&s| s.to_string()).collect();
        let entries = sorted_entries(tools_candidates).await;
        if let Some((url, score)) = entries.first() {
            if *score == u32::MAX {
                info!("Selected Tools mirror: {} (timeout)", url);
            } else {
                info!("Selected Tools mirror: {} ({} ms)", url, score);
            }
            config.mirror = Some(url.clone());
        }
    }

    // PyPI mirror
    if config.non_interactive == Some(false)
        && (config.wizard_all_questions.unwrap_or_default()
            || config.pypi_mirror.is_none()
            || config.is_default("pypi_mirror"))
    {
        let pypi_candidates: Vec<String> = idf_im_lib::get_pypi_mirrors_list().iter().map(|&s| s.to_string()).collect();
        let entries = sorted_entries(pypi_candidates).await;
        let display = entries_to_display(&entries);
        let selected = generic_select("wizard.pypi.mirror", &display)?;
        let url = selected
            .split(" (")
            .next()
            .unwrap_or(&selected)
            .to_string();
        config.pypi_mirror = Some(url);
    } else if config.pypi_mirror.is_none() || config.is_default("pypi_mirror") {
        let pypi_candidates: Vec<String> = idf_im_lib::get_pypi_mirrors_list().iter().map(|&s| s.to_string()).collect();
        let entries = sorted_entries(pypi_candidates).await;
        if let Some((url, score)) = entries.first() {
            if *score == u32::MAX {
                info!("Selected PyPI mirror: {} (timeout)", url);
            } else {
                info!("Selected PyPI mirror: {} ({} ms)", url, score);
            }
            config.pypi_mirror = Some(url.clone());
        }
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
