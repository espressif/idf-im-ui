use std::path::{Path, PathBuf};

use idf_im_lib::{
    ensure_path,
    idf_config::{IdfInstallation, InstallationStatus},
    settings::Settings,
    version_manager::prepare_settings_for_fix_idf_installation,
};
use log::{debug, error, info};
use tauri::AppHandle;

use crate::gui::{
    get_installed_versions,
    ui::{emit_log_message, InstallationStage, MessageLevel},
};

use super::{
    install_single_version, mark_pending_status, merge_extra_selection,
    progress::{abort, emit_error, emit_progress},
};

/// Extra tools/features the user asked to add while repairing.
pub(super) struct ExtraSelection {
    pub(super) tools: Option<Vec<String>>,
    pub(super) features: Option<Vec<String>>,
}

impl ExtraSelection {
    pub(super) fn apply(&self, settings: &mut Settings, version_key: &str) {
        merge_extra_selection(settings, version_key, &self.tools, &self.features);
    }
}

pub(super) fn find_installation(
    app_handle: &AppHandle,
    id: &str,
) -> Result<IdfInstallation, String> {
    let versions = get_installed_versions(app_handle.clone());
    match versions.iter().find(|v| v.id == id) {
        Some(inst) => {
            emit_progress(
                app_handle,
                InstallationStage::Checking,
                10,
                rust_i18n::t!("gui.fix.found_installation", name = inst.name.clone()).to_string(),
                Some(
                    rust_i18n::t!("gui.installation.path_detail", path = inst.path.clone())
                        .to_string(),
                ),
                Some(inst.name.clone()),
            );

            emit_log_message(
                app_handle,
                MessageLevel::Info,
                rust_i18n::t!(
                    "gui.fix.found_at",
                    name = inst.name.clone(),
                    path = inst.path.clone()
                )
                .to_string(),
            );

            Ok(inst.clone())
        }
        None => {
            let error_msg = rust_i18n::t!("gui.fix.not_found_detail", id = id).to_string();
            error!("{}", error_msg);

            emit_error(
                app_handle,
                rust_i18n::t!("gui.fix.not_found").to_string(),
                Some(error_msg.clone()),
                None,
            );

            emit_log_message(app_handle, MessageLevel::Error, error_msg.clone());
            abort(app_handle, error_msg)
        }
    }
}

pub(super) async fn prepare_fix_settings(
    app_handle: &AppHandle,
    installation: &IdfInstallation,
    fix_config_path: Option<&PathBuf>,
) -> Result<Settings, String> {
    emit_progress(
        app_handle,
        InstallationStage::Checking,
        20,
        rust_i18n::t!("gui.fix.preparing_config").to_string(),
        Some(rust_i18n::t!("gui.fix.setting_up").to_string()),
        Some(installation.name.clone()),
    );

    match prepare_settings_for_fix_idf_installation(
        PathBuf::from(installation.path.clone()),
        fix_config_path,
    )
    .await
    {
        Ok(settings) => {
            emit_progress(
                app_handle,
                InstallationStage::Prerequisites,
                30,
                rust_i18n::t!("gui.fix.config_prepared").to_string(),
                Some(rust_i18n::t!("gui.fix.ready_to_repair").to_string()),
                Some(installation.name.clone()),
            );

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                rust_i18n::t!("gui.fix.config_prepared_log").to_string(),
            );

            Ok(settings)
        }
        Err(e) => {
            let error_msg =
                rust_i18n::t!("gui.fix.prepare_failed_detail", error = e.to_string()).to_string();
            error!("{}", error_msg);

            emit_error(
                app_handle,
                rust_i18n::t!("gui.fix.prepare_failed").to_string(),
                Some(e.to_string()),
                Some(installation.name.clone()),
            );

            emit_log_message(app_handle, MessageLevel::Error, error_msg.clone());
            abort(app_handle, error_msg)
        }
    }
}

/// Reinstalls the version; on failure marks its entry as Broken.
pub(super) async fn run_repair(
    app_handle: &AppHandle,
    settings: &Settings,
    installation: &IdfInstallation,
    config_path: &Path,
) -> Result<(), String> {
    emit_progress(
        app_handle,
        InstallationStage::Download,
        35,
        rust_i18n::t!(
            "gui.fix.starting_repair_version",
            version = installation.name.clone()
        )
        .to_string(),
        Some(rust_i18n::t!("gui.fix.beginning_reinstall").to_string()),
        Some(installation.name.clone()),
    );

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        rust_i18n::t!(
            "gui.fix.starting_repair_log",
            version = installation.name.clone()
        )
        .to_string(),
    );

    let error =
        match install_single_version(app_handle.clone(), settings, installation.name.clone()).await
        {
            Ok(_) => {
                emit_log_message(
                    app_handle,
                    MessageLevel::Success,
                    rust_i18n::t!(
                        "gui.fix.repair_success",
                        version = installation.name.clone()
                    )
                    .to_string(),
                );
                return Ok(());
            }
            Err(e) => e.to_string(),
        };

    let error_msg =
        rust_i18n::t!("gui.fix.repair_failed_detail", error = error.clone()).to_string();
    error!("{}", error_msg);

    emit_error(
        app_handle,
        rust_i18n::t!("gui.fix.repair_failed", version = installation.name.clone()).to_string(),
        Some(error),
        Some(installation.name.clone()),
    );

    emit_log_message(app_handle, MessageLevel::Error, error_msg.clone());

    mark_pending_status(
        settings,
        config_path,
        &installation.name,
        InstallationStatus::Broken,
    );

    abort(app_handle, error_msg)
}

/// Settings used to write the repaired installation into the IDE JSON.
/// `esp_idf_json_path` must be the directory: `save_esp_ide_json()` appends the filename.
pub(super) fn repaired_settings(
    mut settings: Settings,
    config_path: &Path,
    version: &str,
    idf_path: PathBuf,
) -> Settings {
    if let Some(parent_dir) = config_path.parent() {
        settings.esp_idf_json_path = Some(parent_dir.to_string_lossy().to_string());
    } else {
        settings.esp_idf_json_path = Some(config_path.to_string_lossy().to_string());
    }
    settings.idf_versions = Some(vec![version.to_string()]);
    settings.idf_path = Some(idf_path);
    settings
}

fn ensure_ide_json_parent(ide_json_path: &str) {
    info!("IDE JSON path to save: {}", ide_json_path);

    if let Some(parent_dir) = Path::new(ide_json_path).parent() {
        info!("Parent directory: {}", parent_dir.display());
        info!("Parent directory exists: {}", parent_dir.exists());
        match ensure_path(parent_dir.to_str().unwrap()) {
            Ok(_) => info!("Parent directory ensured successfully"),
            Err(e) => error!("Failed to ensure parent directory: {}", e),
        }
    }
}

/// Persists the repaired installation to the IDE JSON (90-95%).
pub(super) async fn save_repaired_config(
    app_handle: &AppHandle,
    installation: &IdfInstallation,
    settings: &Settings,
    fix_config_path: Option<&PathBuf>,
    config_path: &Path,
    extras: &ExtraSelection,
) -> Result<(), String> {
    emit_progress(
        app_handle,
        InstallationStage::Configure,
        90,
        rust_i18n::t!("gui.fix.updating_config").to_string(),
        Some(rust_i18n::t!("gui.fix.saving_info").to_string()),
        Some(installation.name.clone()),
    );

    // Reload this installation's stored settings fresh from disk rather than reusing
    // the `settings` snapshot captured before the (potentially long-running) install:
    // another operation may have updated this installation's tools/features while we
    // were repairing, and persisting our stale in-memory copy would silently discard
    // that update. Re-apply just the extras this call itself requested.
    let mut fresh_settings = prepare_settings_for_fix_idf_installation(
        PathBuf::from(installation.path.clone()),
        fix_config_path,
    )
    .await
    .unwrap_or_else(|_| settings.clone());
    extras.apply(&mut fresh_settings, &installation.name);

    // single_version_post_install() only writes the activation script/shortcut, not
    // eim_idf.json, so reconstruct the settings and save the config ourselves.
    let paths = fresh_settings
        .get_version_paths(&installation.name)
        .map_err(|err| {
            error!("Failed to get version paths after repair: {}", err);
            format!("Failed to get version paths after repair: {}", err)
        })?;

    debug!("Config path for IDE JSON: {}", config_path.display());
    debug!("Config path exists: {}", config_path.exists());
    debug!("Config path is file: {}", config_path.is_file());
    debug!("Config path is dir: {}", config_path.is_dir());

    let updated_settings = repaired_settings(
        fresh_settings,
        config_path,
        &installation.name,
        paths.idf_path.clone(),
    );

    ensure_ide_json_parent(updated_settings.esp_idf_json_path.as_ref().unwrap());

    let ide_json_path = updated_settings
        .esp_idf_json_path
        .clone()
        .unwrap_or_default();

    // Always save: the pre-repair entry is marked BeingRepaired/InProgress in place
    // (not removed), so its name+path already exist in the config regardless of
    // whether the status has been brought back to Finished yet.
    match updated_settings.save_esp_ide_json() {
        Ok(_) => {
            emit_progress(
                app_handle,
                InstallationStage::Configure,
                95,
                rust_i18n::t!("gui.fix.config_saved_success").to_string(),
                Some(
                    rust_i18n::t!(
                        "gui.installation.config_saved_to",
                        path = ide_json_path.clone()
                    )
                    .to_string(),
                ),
                Some(installation.name.clone()),
            );

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                rust_i18n::t!("gui.fix.ide_json_updated", path = ide_json_path.clone()).to_string(),
            );

            info!("IDE JSON saved to {}", ide_json_path);
            Ok(())
        }
        Err(e) => {
            // The repair/tools/features install itself succeeded, but without this save
            // the entry stays persisted as BeingRepaired/InProgress forever (nothing else
            // writes eim_idf.json for this path) — so this must be a terminal error, not
            // just a logged warning.
            let error_msg = rust_i18n::t!(
                "gui.installation.ide_config_save_failed_detail",
                error = e.to_string()
            )
            .to_string();
            error!("{}", error_msg);

            emit_error(
                app_handle,
                rust_i18n::t!("gui.fix.config_save_warning").to_string(),
                Some(e.to_string()),
                Some(installation.name.clone()),
            );

            emit_log_message(app_handle, MessageLevel::Error, error_msg.clone());
            abort(app_handle, error_msg)
        }
    }
}

pub(super) fn emit_repair_completed(app_handle: &AppHandle, installation: &IdfInstallation) {
    emit_progress(
        app_handle,
        InstallationStage::Complete,
        100,
        rust_i18n::t!(
            "gui.fix.repair_completed",
            version = installation.name.clone()
        )
        .to_string(),
        Some(rust_i18n::t!("gui.fix.repaired_at", path = installation.path.clone()).to_string()),
        Some(installation.name.clone()),
    );

    emit_log_message(
        app_handle,
        MessageLevel::Success,
        rust_i18n::t!("gui.fix.completed_log", version = installation.name.clone()).to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repaired_settings_points_json_path_at_config_dir() {
        let config_path = PathBuf::from("/cfg/dir").join("eim_idf.json");
        let idf_path = PathBuf::from("/idf/v5.3/esp-idf");
        let s = repaired_settings(Settings::default(), &config_path, "v5.3", idf_path.clone());
        assert_eq!(
            s.esp_idf_json_path,
            Some(PathBuf::from("/cfg/dir").to_string_lossy().to_string())
        );
        assert_eq!(s.idf_versions, Some(vec!["v5.3".to_string()]));
        assert_eq!(s.idf_path, Some(idf_path));
    }

    #[test]
    fn repaired_settings_keeps_parentless_path_as_is() {
        let config_path = PathBuf::from("/");
        let s = repaired_settings(Settings::default(), &config_path, "v5.3", PathBuf::new());
        assert_eq!(s.esp_idf_json_path, Some("/".to_string()));
    }

    #[test]
    fn extra_selection_adds_missing_tools_and_features_for_version() {
        let mut settings = Settings {
            idf_tools: Some(vec!["cmake".to_string()]),
            idf_features: Some(vec!["core".to_string()]),
            ..Settings::default()
        };
        let extras = ExtraSelection {
            tools: Some(vec!["cmake".to_string(), "qemu".to_string()]),
            features: Some(vec!["ide".to_string()]),
        };
        extras.apply(&mut settings, "v5.3");

        let tools = settings.idf_tools_per_version.unwrap();
        assert_eq!(tools["v5.3"], vec!["cmake".to_string(), "qemu".to_string()]);
        let features = settings.idf_features_per_version.unwrap();
        assert_eq!(
            features["v5.3"],
            vec!["core".to_string(), "ide".to_string()]
        );
    }

    #[test]
    fn extra_selection_empty_lists_leave_settings_untouched() {
        let mut settings = Settings::default();
        let before_tools = settings.idf_tools_per_version.clone();
        let before_features = settings.idf_features_per_version.clone();
        let extras = ExtraSelection {
            tools: Some(vec![]),
            features: None,
        };
        extras.apply(&mut settings, "v5.3");
        assert_eq!(settings.idf_tools_per_version, before_tools);
        assert_eq!(settings.idf_features_per_version, before_features);
    }
}
