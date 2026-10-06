use idf_im_lib::{ensure_path, idf_config::InstallationStatus, settings::Settings};
use log::error;
use tauri::AppHandle;

use crate::gui::{
    app_state::set_installation_status,
    ui::{emit_log_message, InstallationStage, MessageLevel},
};

use super::{
    emit_installation_plan, idf_config_path, install_single_version, mark_pending_status,
    progress::{emit_error, emit_progress},
    InstallationPlan,
};

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// Start and end percentage of version `index` when `total` versions share 0-90%.
fn batch_version_range(index: usize, total: usize) -> (u32, u32) {
    (
        ((index * 90) / total) as u32,
        (((index + 1) * 90) / total) as u32,
    )
}

/// Stage reported after version `index` succeeded: the last one completes the batch.
fn batch_version_stage(index: usize, total: usize) -> InstallationStage {
    if index < total - 1 {
        InstallationStage::Configure
    } else {
        InstallationStage::Complete
    }
}

/// The versions selected for installation; reports and fails when there are none.
pub(super) fn selected_versions(
    app_handle: &AppHandle,
    settings: &Settings,
) -> Result<Vec<String>, String> {
    match &settings.idf_versions {
        Some(versions) if !versions.is_empty() => Ok(versions.clone()),
        _ => {
            emit_error(
                app_handle,
                rust_i18n::t!("gui.installation.no_versions_selected").to_string(),
                Some(rust_i18n::t!("gui.installation.select_version").to_string()),
                None,
            );

            emit_log_message(
                app_handle,
                MessageLevel::Warning,
                rust_i18n::t!("gui.installation.no_versions_warning").to_string(),
            );

            set_installation_status(app_handle, false)?;
            Err(rust_i18n::t!("gui.installation.no_versions_warning").to_string())
        }
    }
}

pub(super) fn plan(versions: &[String], current_version_index: Option<usize>) -> InstallationPlan {
    InstallationPlan {
        total_versions: versions.len(),
        versions: versions.to_vec(),
        current_version_index,
    }
}

pub(super) fn announce_batch(app_handle: &AppHandle, versions: &[String]) {
    emit_installation_plan(app_handle, plan(versions, None));

    let total_versions = versions.len();
    emit_progress(
        app_handle,
        InstallationStage::Checking,
        0,
        rust_i18n::t!(
            "gui.installation.starting_batch",
            count = total_versions,
            plural = plural_suffix(total_versions)
        )
        .to_string(),
        Some(
            rust_i18n::t!(
                "gui.installation.versions_list",
                versions = versions.join(", ")
            )
            .to_string(),
        ),
        None,
    );

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!(
            "gui.installation.batch_log",
            count = total_versions,
            versions = versions.join(", ")
        )
        .to_string(),
    );
}

/// Installs `versions[index]`, reporting progress inside its share of 0-90%.
pub(super) async fn install_batch_version(
    app_handle: &AppHandle,
    settings: &Settings,
    versions: &[String],
    index: usize,
) -> Result<(), String> {
    emit_installation_plan(app_handle, plan(versions, Some(index)));

    let version = &versions[index];
    let total_versions = versions.len();
    let (version_start_percentage, version_end_percentage) =
        batch_version_range(index, total_versions);

    emit_progress(
        app_handle,
        InstallationStage::Download,
        version_start_percentage,
        rust_i18n::t!("gui.installation.starting_version", version = version).to_string(),
        Some(
            rust_i18n::t!(
                "gui.installation.version_detail",
                current = index + 1,
                total = total_versions,
                version = version
            )
            .to_string(),
        ),
        Some(version.clone()),
    );

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!(
            "gui.installation.starting_version_log",
            version = version,
            current = index + 1,
            total = total_versions
        )
        .to_string(),
    );

    let error = match install_single_version(app_handle.clone(), settings, version.clone()).await {
        Ok(_) => {
            report_version_success(app_handle, versions, index, version_end_percentage);
            return Ok(());
        }
        Err(e) => e.to_string(),
    };
    report_version_failure(app_handle, settings, version, error)
}

fn report_version_success(
    app_handle: &AppHandle,
    versions: &[String],
    index: usize,
    percentage: u32,
) {
    let version = &versions[index];
    let total_versions = versions.len();
    emit_progress(
        app_handle,
        batch_version_stage(index, total_versions),
        percentage,
        rust_i18n::t!("gui.installation.version_success", version = version).to_string(),
        Some(
            rust_i18n::t!(
                "gui.installation.completed_versions",
                current = index + 1,
                total = total_versions
            )
            .to_string(),
        ),
        Some(version.clone()),
    );

    emit_log_message(
        app_handle,
        MessageLevel::Success,
        rust_i18n::t!(
            "gui.installation.version_success_log",
            version = version,
            current = index + 1,
            total = total_versions
        )
        .to_string(),
    );
}

fn report_version_failure(
    app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    error: String,
) -> Result<(), String> {
    error!("Failed to install version {}: {}", version, error);

    mark_pending_status(
        settings,
        &idf_config_path(settings),
        version,
        InstallationStatus::Failed,
    );

    emit_error(
        app_handle,
        rust_i18n::t!("gui.installation.version_failed", version = version).to_string(),
        Some(error.clone()),
        Some(version.to_string()),
    );

    emit_log_message(
        app_handle,
        MessageLevel::Error,
        rust_i18n::t!(
            "gui.installation.version_failed_log",
            version = version,
            error = error.clone()
        )
        .to_string(),
    );

    set_installation_status(app_handle, false)?;
    Err(rust_i18n::t!(
        "gui.installation.failed_for_version",
        version = version,
        error = error
    )
    .to_string())
}

/// Configuration phase (90-95%): saves the IDE JSON. A save failure is only a warning.
pub(super) fn save_ide_config(app_handle: &AppHandle, settings: &Settings) {
    emit_progress(
        app_handle,
        InstallationStage::Configure,
        90,
        rust_i18n::t!("gui.installation.configuring_environment").to_string(),
        Some(rust_i18n::t!("gui.installation.saving_config").to_string()),
        None,
    );

    let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
    let _ = ensure_path(&ide_json_path);

    match settings.save_esp_ide_json() {
        Ok(_) => {
            emit_progress(
                app_handle,
                InstallationStage::Configure,
                93,
                rust_i18n::t!("gui.installation.config_saved").to_string(),
                Some(
                    rust_i18n::t!(
                        "gui.installation.config_saved_to",
                        path = ide_json_path.clone()
                    )
                    .to_string(),
                ),
                None,
            );

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                rust_i18n::t!("gui.installation.ide_json_saved", path = ide_json_path).to_string(),
            );
        }
        Err(e) => {
            emit_progress(
                app_handle,
                InstallationStage::Configure,
                93,
                rust_i18n::t!("gui.installation.config_save_warning").to_string(),
                Some(e.to_string()),
                None,
            );

            emit_log_message(
                app_handle,
                MessageLevel::Warning,
                rust_i18n::t!("gui.installation.ide_json_failed", error = e.to_string())
                    .to_string(),
            );
        }
    }
}

/// Final completion (95-100%).
pub(super) async fn finish_batch(app_handle: &AppHandle, versions: &[String]) {
    emit_progress(
        app_handle,
        InstallationStage::Configure,
        97,
        rust_i18n::t!("gui.installation.finalizing").to_string(),
        Some(rust_i18n::t!("gui.installation.completing_setup").to_string()),
        None,
    );

    // Small delay to show finalization
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let total_versions = versions.len();
    emit_progress(
        app_handle,
        InstallationStage::Complete,
        100,
        rust_i18n::t!(
            "gui.installation.all_versions_success",
            count = total_versions,
            plural = plural_suffix(total_versions)
        )
        .to_string(),
        Some(
            rust_i18n::t!(
                "gui.installation.completed_list",
                versions = versions.join(", ")
            )
            .to_string(),
        ),
        None,
    );

    emit_log_message(
        app_handle,
        MessageLevel::Success,
        rust_i18n::t!("gui.installation.batch_completed", count = total_versions).to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_suffix_only_for_counts_other_than_one() {
        assert_eq!(plural_suffix(1), "");
        assert_eq!(plural_suffix(0), "s");
        assert_eq!(plural_suffix(3), "s");
    }

    #[test]
    fn batch_version_range_splits_ninety_percent_evenly() {
        assert_eq!(batch_version_range(0, 1), (0, 90));
        assert_eq!(batch_version_range(0, 3), (0, 30));
        assert_eq!(batch_version_range(1, 3), (30, 60));
        assert_eq!(batch_version_range(2, 3), (60, 90));
        assert_eq!(batch_version_range(1, 4), (22, 45));
    }

    #[test]
    fn batch_version_stage_completes_on_last_version() {
        assert!(matches!(
            batch_version_stage(0, 2),
            InstallationStage::Configure
        ));
        assert!(matches!(
            batch_version_stage(1, 2),
            InstallationStage::Complete
        ));
        assert!(matches!(
            batch_version_stage(0, 1),
            InstallationStage::Complete
        ));
    }

    #[test]
    fn plan_lists_all_versions_with_current_index() {
        let versions = vec!["v5.1".to_string(), "v5.2".to_string()];
        let p = plan(&versions, Some(1));
        assert_eq!(p.total_versions, 2);
        assert_eq!(p.versions, versions);
        assert_eq!(p.current_version_index, Some(1));
        assert_eq!(plan(&versions, None).current_version_index, None);
    }
}
