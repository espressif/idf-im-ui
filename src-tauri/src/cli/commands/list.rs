use std::path::PathBuf;

use idf_im_lib::idf_config::IdfInstallation;
use idf_im_lib::version_manager::{FeatureListReport, ToolListReport};
use log::{debug, error, info, warn};
use rust_i18n::t;

use crate::cli::helpers;
use crate::cli::{status_label, version_option_labels};

pub fn list(config_path: Option<&PathBuf>) -> anyhow::Result<()> {
    info!("{}", t!("list.title"));
    match idf_im_lib::version_manager::get_esp_ide_config(config_path) {
        Ok(config) => {
            if config.idf_installed.is_empty() {
                warn!("{}", t!("list.no_versions"));
                return Ok(());
            }
            println!("{}", t!("list.installed_title"));
            for version in config.idf_installed {
                let sl = status_label(&version.status);
                if version.id == config.idf_selected_id {
                    println!(
                        "{}",
                        t!(
                            "list.version_selected",
                            name = version.name,
                            path = version.path,
                            status = sl
                        )
                    );
                } else {
                    println!(
                        "{}",
                        t!(
                            "list.version",
                            name = version.name,
                            path = version.path,
                            status = sl
                        )
                    );
                }
            }
            Ok(())
        }
        Err(err) => {
            info!("{}", t!("list.no_versions"));
            info!("{}", t!("cli.hint.custom_json_path"));
            debug!("Error: {}", err);
            Ok(())
        }
    }
}

pub fn list_tools(
    identifier: Option<String>,
    outdated: bool,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<()> {
    let identifier = match identifier {
        Some(id) => id,
        None => {
            let Some(versions) = installed_versions_for_listing(config_path) else {
                return Ok(());
            };
            let i = helpers::generic_select_index(
                &t!("list_tools.idf_prompt"),
                &version_option_labels(&versions),
            )
            .map_err(|err| anyhow::anyhow!(err))?;
            versions[i].name.clone()
        }
    };

    match idf_im_lib::version_manager::list_idf_tools(Some(&identifier), outdated, config_path) {
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

pub fn list_features(
    identifier: Option<String>,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<()> {
    let identifier = match identifier {
        Some(id) => id,
        None => {
            let Some(versions) = installed_versions_for_listing(config_path) else {
                return Ok(());
            };
            let options: Vec<String> = versions.iter().map(|v| v.name.clone()).collect();
            helpers::generic_select(&t!("list_features.idf_prompt"), &options)
                .map_err(|err| anyhow::anyhow!(err))?
        }
    };

    match idf_im_lib::version_manager::list_idf_features(Some(&identifier), config_path) {
        Ok(report) => {
            format_feature_list_report(&report);
            Ok(())
        }
        Err(err) => {
            error!("{}", err);
            Err(anyhow::anyhow!(err))
        }
    }
}

/// Returns `None` (after warning the user) when there is nothing to pick from.
fn installed_versions_for_listing(config_path: Option<&PathBuf>) -> Option<Vec<IdfInstallation>> {
    match idf_im_lib::version_manager::list_installed_versions(config_path) {
        Ok(versions) if versions.is_empty() => {
            warn!("{}", t!("list.no_versions"));
            None
        }
        Ok(versions) => Some(versions),
        Err(err) => {
            debug!("Error: {}", err);
            warn!("{}", t!("list.no_versions"));
            info!("{}", t!("cli.hint.custom_json_path"));
            None
        }
    }
}

fn format_tool_list_report(report: &ToolListReport) {
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
        for vi in entry
            .version_inspections
            .iter()
            .filter(|vi| vi.has_platform_download)
        {
            let installed_marker = match &vi.installed {
                Some(info) => t!("list_tools.installed", version = info.version),
                None => t!("list_tools.not_installed"),
            };
            println!(
                "  - {} ({}){}",
                vi.version.name, vi.version.status, installed_marker
            );
        }
    }
    if report.outdated_only {
        print_outdated_tools(report);
    }
}

fn print_outdated_tools(report: &ToolListReport) {
    println!();
    if report.outdated.is_empty() {
        println!("{}", t!("list_tools.no_outdated"));
        return;
    }
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

fn format_feature_list_report(report: &FeatureListReport) {
    println!(
        "{}",
        t!(
            "list_features.title",
            name = report.idf.name,
            path = report.idf.path
        )
    );
    println!();
    for entry in &report.features {
        println!(
            "{}",
            feature_line(
                &entry.feature.name,
                entry.feature.description.as_deref(),
                entry.feature.optional,
                entry.installed
            )
        );
    }
}

fn feature_line(name: &str, description: Option<&str>, optional: bool, installed: bool) -> String {
    let name_and_desc = match description {
        Some(description) => format!("{}: {}", name, description),
        None => name.to_string(),
    };
    let optional_marker = if optional {
        t!("list_features.optional_marker").to_string()
    } else {
        String::new()
    };
    let installed_marker = if installed {
        t!("list_features.installed").to_string()
    } else {
        t!("list_features.not_installed").to_string()
    };
    format!("{}{}{}", name_and_desc, optional_marker, installed_marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_line_with_description_and_optional_marker() {
        let line = feature_line("docs", Some("Documentation"), true, true);
        assert_eq!(
            line,
            format!(
                "docs: Documentation{}{}",
                t!("list_features.optional_marker"),
                t!("list_features.installed")
            )
        );
    }

    #[test]
    fn feature_line_without_description_or_optional_marker() {
        let line = feature_line("core", None, false, false);
        assert_eq!(line, format!("core{}", t!("list_features.not_installed")));
    }
}
