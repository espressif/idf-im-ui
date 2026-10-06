use idf_im_lib::system_dependencies;
use serde_json::Value;
use tauri::AppHandle;

use crate::gui::{
    app_state::update_settings,
    ui::{emit_log_message, InstallationStage, MessageLevel},
};

use super::{
    checks::missing_prerequisites,
    prerequisites::{install_prerequisites, python_install, python_sanity_check},
    progress::{emit_error, emit_progress},
    settings,
};

/// Name of the first (newest) version returned by `get_idf_versions`.
pub(super) fn first_version_name(versions: &[Value]) -> String {
    versions[0]["name"]
        .clone()
        .to_string()
        .trim_matches('"')
        .to_string()
}

pub(super) fn with_ide_feature(features: Option<Vec<String>>) -> Vec<String> {
    let mut features = features.unwrap_or_default();
    if !features.contains(&"ide".to_string()) {
        features.push("ide".to_string());
    }
    features
}

fn recheck_missing() -> Option<Vec<String>> {
    system_dependencies::check_prerequisites_with_result()
        .ok()
        .map(|recheck| recheck.missing.into_iter().map(|p| p.to_string()).collect())
}

/// Windows only: installs the missing prerequisites and returns what is still missing.
fn install_missing_prerequisites(
    app_handle: &AppHandle,
    mut prerequisites: Vec<String>,
) -> Result<Vec<String>, String> {
    emit_progress(
        app_handle,
        InstallationStage::Prerequisites,
        5,
        rust_i18n::t!("gui.simple_setup.installing_prerequisites").to_string(),
        Some(
            rust_i18n::t!("gui.simple_setup.missing", items = prerequisites.join(", ")).to_string(),
        ),
        None,
    );

    let installed = install_prerequisites(app_handle.clone());
    if let Some(recheck) = recheck_missing() {
        prerequisites = recheck;
    }
    if !installed {
        emit_error(
            app_handle,
            rust_i18n::t!("gui.simple_setup.prerequisites_failed").to_string(),
            Some(
                rust_i18n::t!("gui.simple_setup.missing", items = prerequisites.join(", "))
                    .to_string(),
            ),
            None,
        );
        return Err(rust_i18n::t!("gui.simple_setup.prerequisites_failed").to_string());
    }
    Ok(prerequisites)
}

pub(super) fn ensure_prerequisites(app_handle: &AppHandle, os: &str) -> Result<(), String> {
    let mut prerequisites = missing_prerequisites(
        app_handle,
        &rust_i18n::t!("gui.simple_setup.prerequisites_check_failed"),
    )?;

    if !prerequisites.is_empty() && os == "windows" {
        prerequisites = install_missing_prerequisites(app_handle, prerequisites)?;
    }

    if !prerequisites.is_empty() {
        emit_error(
            app_handle,
            rust_i18n::t!("gui.simple_setup.prerequisites_missing").to_string(),
            Some(
                rust_i18n::t!(
                    "gui.simple_setup.please_install",
                    items = prerequisites.join(", ")
                )
                .to_string(),
            ),
            None,
        );
        return Err(rust_i18n::t!("gui.simple_setup.prerequisites_missing").to_string());
    }

    emit_progress(
        app_handle,
        InstallationStage::Prerequisites,
        10,
        rust_i18n::t!("gui.simple_setup.prerequisites_verified").to_string(),
        None,
        None,
    );
    Ok(())
}

fn python_found(app_handle: &AppHandle) -> bool {
    python_sanity_check(app_handle.clone(), None)
        .iter()
        .all(|check| check.passed)
}

pub(super) fn ensure_python(app_handle: &AppHandle, os: &str) -> Result<(), String> {
    let mut found = python_found(app_handle);

    if !found && os == "windows" {
        emit_progress(
            app_handle,
            InstallationStage::Python,
            15,
            rust_i18n::t!("gui.simple_setup.installing_python").to_string(),
            None,
            None,
        );

        if !python_install(app_handle.clone()) {
            emit_error(
                app_handle,
                rust_i18n::t!("gui.simple_setup.python_failed").to_string(),
                Some(rust_i18n::t!("gui.simple_setup.python_install_failed").to_string()),
                None,
            );
            return Err(rust_i18n::t!("gui.simple_setup.python_failed").to_string());
        }

        found = python_found(app_handle);
    }

    if !found {
        emit_error(
            app_handle,
            rust_i18n::t!("gui.simple_setup.python_not_found").to_string(),
            Some(rust_i18n::t!("gui.simple_setup.install_python_manually").to_string()),
            None,
        );
        return Err(rust_i18n::t!("gui.simple_setup.python_not_found").to_string());
    }

    emit_progress(
        app_handle,
        InstallationStage::Python,
        20,
        rust_i18n::t!("gui.simple_setup.python_ready").to_string(),
        None,
        None,
    );
    Ok(())
}

/// Picks the newest available version when none is configured.
pub(super) async fn select_default_version(app_handle: &AppHandle) -> Result<(), String> {
    emit_progress(
        app_handle,
        InstallationStage::Configure,
        25,
        rust_i18n::t!("gui.simple_setup.fetching_versions").to_string(),
        None,
        None,
    );

    let versions = settings::get_idf_versions(app_handle.clone(), false).await;

    if versions.is_empty() {
        emit_error(
            app_handle,
            rust_i18n::t!("gui.simple_setup.fetch_failed").to_string(),
            Some(rust_i18n::t!("gui.simple_setup.retrieve_failed").to_string()),
            None,
        );
        return Err(rust_i18n::t!("gui.simple_setup.fetch_failed").to_string());
    }

    let version = first_version_name(&versions);

    if let Err(e) = update_settings(app_handle, |settings| {
        settings.idf_versions = Some(vec![version.clone()]);
    }) {
        emit_error(
            app_handle,
            rust_i18n::t!("gui.simple_setup.config_failed").to_string(),
            Some(e.to_string()),
            None,
        );
        return Err(e);
    }

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!(
            "gui.simple_setup.version_selected",
            version = version.clone()
        )
        .to_string(),
    );

    emit_progress(
        app_handle,
        InstallationStage::Configure,
        30,
        rust_i18n::t!(
            "gui.simple_setup.version_selected_event",
            version = version.clone()
        )
        .to_string(),
        None,
        Some(version.clone()),
    );
    Ok(())
}

pub(super) fn add_ide_feature(app_handle: &AppHandle) -> Result<(), String> {
    if let Err(e) = update_settings(app_handle, |settings| {
        settings.idf_features = Some(with_ide_feature(settings.idf_features.clone()));
    }) {
        emit_error(
            app_handle,
            rust_i18n::t!("gui.simple_setup.config_ide_failed").to_string(),
            Some(e.to_string()),
            None,
        );
        return Err(e);
    }

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!("gui.simple_setup.ide_feature_added").to_string(),
    );

    emit_progress(
        app_handle,
        InstallationStage::Configure,
        35,
        rust_i18n::t!("gui.simple_setup.ide_feature_configured").to_string(),
        None,
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn first_version_name_strips_json_quotes() {
        let versions = vec![json!({"name": "v5.4"}), json!({"name": "v5.3"})];
        assert_eq!(first_version_name(&versions), "v5.4");
    }

    #[test]
    fn first_version_name_of_missing_name_is_null() {
        assert_eq!(first_version_name(&[json!({})]), "null");
    }

    #[test]
    fn with_ide_feature_appends_once() {
        assert_eq!(with_ide_feature(None), vec!["ide".to_string()]);
        assert_eq!(
            with_ide_feature(Some(vec!["core".to_string()])),
            vec!["core".to_string(), "ide".to_string()]
        );
        assert_eq!(
            with_ide_feature(Some(vec!["ide".to_string(), "core".to_string()])),
            vec!["ide".to_string(), "core".to_string()]
        );
    }
}
