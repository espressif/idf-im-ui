use std::path::PathBuf;

use idf_im_lib::{
    ensure_path,
    offline_installer::{
        copy_idf_from_offline_archive, install_prerequisites_offline, use_offline_archive,
    },
    settings::{Settings, VersionPaths},
    utils::copy_dir_contents,
};
use log::{debug, error, info, warn};
use tauri::AppHandle;
use tempfile::TempDir;

use crate::gui::{
    app_state::get_settings_non_blocking,
    commands::idf_tools::setup_tools,
    ui::{emit_log_message, InstallationStage, MessageLevel},
    utils::{get_file_name, is_path_empty_or_nonexistent},
};

use super::{
    checks::precheck_posix_prerequisites,
    progress::{abort, archive_percentage, emit_error, emit_progress, OfflineVersionProgress},
};

/// Tools directory of the first configured version, used for the Windows offline prerequisites.
pub(super) fn offline_tools_dir(settings: &Settings) -> Option<PathBuf> {
    let Some(idf_versions) = &settings.idf_versions else {
        warn!("No IDF versions set in settings");
        return None;
    };
    let Some(version) = idf_versions.first() else {
        warn!("No IDF versions found in settings");
        return None;
    };
    match settings.get_version_paths(version) {
        Ok(paths) => {
            info!(
                "Using tools dir from get_version_paths: {:?}",
                paths.tool_install_directory
            );
            Some(paths.tool_install_directory)
        }
        Err(e) => {
            warn!(
                "Failed to get version paths: {}, falling back to manual calculation",
                e
            );
            None
        }
    }
}

/// Installs one offline archive (`index` of `total`) with its progress slot.
pub(super) struct ArchiveInstall<'a> {
    pub(super) app_handle: &'a AppHandle,
    pub(super) archive: &'a str,
    pub(super) index: usize,
    pub(super) total: usize,
}

impl ArchiveInstall<'_> {
    fn pct(&self, offset: usize) -> u32 {
        archive_percentage(self.index, self.total, offset)
    }

    fn step(&self, stage: InstallationStage, offset: usize, message: String, detail: String) {
        emit_progress(
            self.app_handle,
            stage,
            self.pct(offset),
            message,
            Some(detail),
            None,
        );
    }

    fn log(&self, level: MessageLevel, message: String) {
        emit_log_message(self.app_handle, level, message);
    }

    /// Emits an error event, clears the "installing" flag and fails with `error_msg`.
    fn fail<T>(
        &self,
        message: String,
        error_msg: String,
        version: Option<String>,
    ) -> Result<T, String> {
        emit_error(self.app_handle, message, Some(error_msg.clone()), version);
        abort(self.app_handle, error_msg)
    }

    pub(super) async fn run(&self, install_path: &str) -> Result<(), String> {
        let archive_path = self.validate()?;
        let offline_archive_dir = self.create_workspace()?;
        let settings = self.configure_settings(archive_path, install_path)?;
        let mut settings = self.extract(settings, &offline_archive_dir)?;
        self.prepare_prerequisites(&mut settings, &offline_archive_dir)
            .await?;
        self.copy_idf(&offline_archive_dir, &settings)?;
        self.setup_versions(&mut settings, &offline_archive_dir)
            .await?;
        self.save_ide_config(&settings)?;

        self.log(
            MessageLevel::Success,
            rust_i18n::t!(
                "gui.offline.archive_processed",
                name = get_file_name(self.archive),
                current = self.index + 1,
                total = self.total
            )
            .to_string(),
        );
        Ok(())
    }

    fn validate(&self) -> Result<PathBuf, String> {
        let archive_path = PathBuf::from(self.archive);

        emit_progress(
            self.app_handle,
            InstallationStage::Checking,
            (self.index * 10 / self.total) as u32,
            rust_i18n::t!(
                "gui.offline.validating_archive",
                name = get_file_name(self.archive)
            )
            .to_string(),
            Some(
                rust_i18n::t!(
                    "gui.offline.archive_number",
                    current = self.index + 1,
                    total = self.total
                )
                .to_string(),
            ),
            None,
        );

        if !archive_path.try_exists().unwrap_or(false) {
            return self.fail(
                rust_i18n::t!("gui.offline.archive_not_found").to_string(),
                rust_i18n::t!("gui.offline.archive_not_exist", path = self.archive).to_string(),
                None,
            );
        }

        self.log(
            MessageLevel::Info,
            rust_i18n::t!("gui.offline.validated", path = self.archive).to_string(),
        );
        Ok(archive_path)
    }

    fn create_workspace(&self) -> Result<TempDir, String> {
        self.step(
            InstallationStage::Extract,
            10,
            rust_i18n::t!("gui.offline.creating_workspace").to_string(),
            rust_i18n::t!("gui.offline.preparing_extraction").to_string(),
        );

        let offline_archive_dir = TempDir::new().map_err(|e| {
            let error_msg =
                rust_i18n::t!("gui.offline.temp_dir_failed", error = e.to_string()).to_string();
            emit_error(
                self.app_handle,
                rust_i18n::t!("gui.offline.workspace_failed").to_string(),
                Some(error_msg.clone()),
                None,
            );
            error_msg
        })?;

        self.log(
            MessageLevel::Info,
            rust_i18n::t!(
                "gui.offline.temp_dir_created",
                path = offline_archive_dir.path().display().to_string()
            )
            .to_string(),
        );
        Ok(offline_archive_dir)
    }

    fn configure_settings(
        &self,
        archive_path: PathBuf,
        install_path: &str,
    ) -> Result<Settings, String> {
        self.step(
            InstallationStage::Extract,
            15,
            rust_i18n::t!("gui.offline.configuring_settings").to_string(),
            rust_i18n::t!("gui.offline.preparing_config").to_string(),
        );

        let mut settings = get_settings_non_blocking(self.app_handle)?;
        if !install_path.is_empty() && is_path_empty_or_nonexistent(install_path, &[]) {
            settings.path = Some(PathBuf::from(install_path));
            self.log(
                MessageLevel::Info,
                rust_i18n::t!("gui.offline.custom_path", path = install_path).to_string(),
            );
        }
        settings.use_local_archive = Some(archive_path);
        Ok(settings)
    }

    fn extract(
        &self,
        settings: Settings,
        offline_archive_dir: &TempDir,
    ) -> Result<Settings, String> {
        self.step(
            InstallationStage::Extract,
            20,
            rust_i18n::t!(
                "gui.offline.extracting_archive",
                name = get_file_name(self.archive)
            )
            .to_string(),
            rust_i18n::t!("gui.offline.processing_contents").to_string(),
        );

        match use_offline_archive(settings, offline_archive_dir) {
            Ok(updated_config) => {
                self.log(
                    MessageLevel::Success,
                    rust_i18n::t!("gui.offline.extraction_success").to_string(),
                );
                Ok(updated_config)
            }
            Err(err) => {
                let error_msg = rust_i18n::t!(
                    "gui.offline.extraction_failed_detail",
                    error = err.to_string()
                )
                .to_string();
                error!("{}", error_msg);
                self.fail(
                    rust_i18n::t!("gui.offline.extraction_failed").to_string(),
                    error_msg,
                    None,
                )
            }
        }
    }

    async fn prepare_prerequisites(
        &self,
        settings: &mut Settings,
        offline_archive_dir: &TempDir,
    ) -> Result<(), String> {
        if std::env::consts::OS == "windows" {
            self.install_windows_prerequisites(settings, offline_archive_dir)
                .await
        } else {
            self.verify_posix_prerequisites()
        }
    }

    async fn install_windows_prerequisites(
        &self,
        settings: &mut Settings,
        offline_archive_dir: &TempDir,
    ) -> Result<(), String> {
        info!("About to install prerequisites from offline archive");

        let tools_dir = offline_tools_dir(settings).unwrap();

        self.step(
            InstallationStage::Prerequisites,
            25,
            rust_i18n::t!("gui.offline.installing_prerequisites").to_string(),
            rust_i18n::t!("gui.offline.installing_windows_components").to_string(),
        );

        info!("Tools dir set to: {:?}", tools_dir);
        info!("Archive dir path: {:?}", offline_archive_dir.path());

        match install_prerequisites_offline(offline_archive_dir, tools_dir).await {
            Ok(_) => {
                self.log(
                    MessageLevel::Success,
                    rust_i18n::t!("gui.offline.prerequisites_success").to_string(),
                );
                settings.skip_prerequisites_check = Some(true);
                Ok(())
            }
            Err(err) => self.fail(
                rust_i18n::t!("gui.offline.prerequisites_failed").to_string(),
                rust_i18n::t!(
                    "gui.offline.prerequisites_failed_detail",
                    error = err.to_string()
                )
                .to_string(),
                None,
            ),
        }
    }

    fn verify_posix_prerequisites(&self) -> Result<(), String> {
        self.step(
            InstallationStage::Prerequisites,
            25,
            rust_i18n::t!("gui.offline.checking_prerequisites").to_string(),
            rust_i18n::t!("gui.offline.verifying_components").to_string(),
        );

        if let Err(error_msg) = precheck_posix_prerequisites(self.app_handle) {
            return abort(self.app_handle, error_msg);
        }

        self.log(
            MessageLevel::Success,
            rust_i18n::t!("gui.offline.prerequisites_verified").to_string(),
        );
        Ok(())
    }

    fn copy_idf(&self, offline_archive_dir: &TempDir, settings: &Settings) -> Result<(), String> {
        self.step(
            InstallationStage::Download,
            35,
            rust_i18n::t!("gui.offline.installing_idf").to_string(),
            rust_i18n::t!("gui.offline.copying_files").to_string(),
        );

        match copy_idf_from_offline_archive(offline_archive_dir, settings) {
            Ok(_) => {
                self.log(
                    MessageLevel::Success,
                    rust_i18n::t!("gui.offline.idf_copy_success").to_string(),
                );
                Ok(())
            }
            Err(err) => {
                let error_msg =
                    rust_i18n::t!("gui.offline.idf_copy_failed", error = err.to_string())
                        .to_string();
                error!("{}", error_msg);
                self.fail(
                    rust_i18n::t!("gui.offline.idf_install_failed").to_string(),
                    error_msg,
                    None,
                )
            }
        }
    }

    async fn setup_versions(
        &self,
        settings: &mut Settings,
        offline_archive_dir: &TempDir,
    ) -> Result<(), String> {
        let versions = settings.idf_versions.clone().unwrap_or_default();
        let progress = OfflineVersionProgress::new(self.index, self.total, versions.len());
        for (version_index, idf_version) in versions.iter().enumerate() {
            let version = OfflineVersion {
                archive: self,
                name: idf_version,
                index: version_index,
                count: versions.len(),
                progress,
            };
            version.setup(settings, offline_archive_dir).await?;
        }
        Ok(())
    }

    fn save_ide_config(&self, settings: &Settings) -> Result<(), String> {
        self.step(
            InstallationStage::Configure,
            88,
            rust_i18n::t!("gui.offline.saving_ide_config").to_string(),
            rust_i18n::t!("gui.offline.updating_settings").to_string(),
        );

        let ide_conf_path_tmp =
            PathBuf::from(&settings.esp_idf_json_path.clone().unwrap_or_default());
        debug!("IDE configuration path: {}", ide_conf_path_tmp.display());

        if let Err(err) = ensure_path(ide_conf_path_tmp.to_str().unwrap()) {
            let error_msg =
                rust_i18n::t!("gui.offline.ide_dir_failed", error = err.to_string()).to_string();
            error!("{}", error_msg);
            return self.fail(
                rust_i18n::t!("gui.offline.ide_config_failed").to_string(),
                error_msg,
                None,
            );
        }
        self.log(
            MessageLevel::Info,
            rust_i18n::t!("gui.offline.ide_dir_created").to_string(),
        );

        if let Err(err) = settings.save_esp_ide_json() {
            let error_msg = rust_i18n::t!(
                "gui.offline.ide_config_save_failed_detail",
                error = err.to_string()
            )
            .to_string();
            error!("{}", error_msg);
            return self.fail(
                rust_i18n::t!("gui.offline.ide_config_save_failed").to_string(),
                error_msg,
                None,
            );
        }
        self.log(
            MessageLevel::Success,
            rust_i18n::t!("gui.offline.ide_config_saved").to_string(),
        );
        debug!("IDE configuration saved.");
        Ok(())
    }
}

/// Sets up one IDF version contained in an offline archive.
struct OfflineVersion<'a> {
    archive: &'a ArchiveInstall<'a>,
    name: &'a String,
    index: usize,
    count: usize,
    progress: OfflineVersionProgress,
}

impl OfflineVersion<'_> {
    fn step(&self, stage: InstallationStage, percentage: u32, message: String, detail: String) {
        emit_progress(
            self.archive.app_handle,
            stage,
            percentage,
            message,
            Some(detail),
            Some(self.name.clone()),
        );
    }

    fn fail<T>(&self, message: String, error_msg: String) -> Result<T, String> {
        error!("{}", error_msg);
        self.archive
            .fail(message, error_msg, Some(self.name.clone()))
    }

    async fn setup(
        &self,
        settings: &mut Settings,
        offline_archive_dir: &TempDir,
    ) -> Result<(), String> {
        self.step(
            InstallationStage::Tools,
            self.progress.processing(self.index),
            rust_i18n::t!("gui.offline.processing_version", version = self.name).to_string(),
            rust_i18n::t!(
                "gui.offline.setting_up_version",
                current = self.index + 1,
                total = self.count
            )
            .to_string(),
        );

        let paths = self.version_paths(settings)?;

        settings.idf_path = Some(paths.idf_path.clone());
        idf_im_lib::add_path_to_path(paths.idf_path.to_str().unwrap());

        self.copy_tools(offline_archive_dir, &paths)?;

        idf_im_lib::add_path_to_path(paths.tool_install_directory.to_str().unwrap());

        self.configure_tools(settings, offline_archive_dir, paths)
            .await
    }

    fn version_paths(&self, settings: &Settings) -> Result<VersionPaths, String> {
        match settings.get_version_paths(self.name) {
            Ok(paths) => {
                self.archive.log(
                    MessageLevel::Info,
                    rust_i18n::t!(
                        "gui.offline.version_paths_configured",
                        version = self.name,
                        path = paths.idf_path.display().to_string()
                    )
                    .to_string(),
                );
                Ok(paths)
            }
            Err(err) => self.fail(
                rust_i18n::t!("gui.offline.path_config_failed").to_string(),
                rust_i18n::t!(
                    "gui.offline.path_config_failed_detail",
                    error = err.to_string()
                )
                .to_string(),
            ),
        }
    }

    fn copy_tools(
        &self,
        offline_archive_dir: &TempDir,
        paths: &VersionPaths,
    ) -> Result<(), String> {
        self.step(
            InstallationStage::Tools,
            self.progress.copying_tools(self.index),
            rust_i18n::t!("gui.offline.installing_tools").to_string(),
            rust_i18n::t!("gui.offline.copying_tools").to_string(),
        );

        match copy_dir_contents(
            &offline_archive_dir.path().join("dist"),
            &paths.tool_download_directory,
        ) {
            Ok(_) => {
                self.archive.log(
                    MessageLevel::Success,
                    rust_i18n::t!("gui.offline.tools_copy_success").to_string(),
                );
                Ok(())
            }
            Err(err) => self.fail(
                rust_i18n::t!("gui.offline.tools_install_failed").to_string(),
                rust_i18n::t!("gui.offline.tools_copy_failed", error = err.to_string()).to_string(),
            ),
        }
    }

    async fn configure_tools(
        &self,
        settings: &Settings,
        offline_archive_dir: &TempDir,
        paths: VersionPaths,
    ) -> Result<(), String> {
        let app_handle = self.archive.app_handle;
        self.step(
            InstallationStage::Tools,
            self.progress.configuring_tools(self.index),
            rust_i18n::t!("gui.offline.configuring_tools").to_string(),
            rust_i18n::t!("gui.offline.setting_up_environment").to_string(),
        );

        let (export_paths, export_vars) = match setup_tools(
            app_handle,
            settings,
            &paths.idf_path,
            &paths.actual_version,
            Some(offline_archive_dir.path()),
        )
        .await
        {
            Ok((paths, vars)) => {
                self.archive.log(
                    MessageLevel::Success,
                    rust_i18n::t!("gui.offline.tools_configured").to_string(),
                );
                (paths, vars)
            }
            Err(err) => {
                return self.fail(
                    rust_i18n::t!("gui.offline.tools_config_failed").to_string(),
                    rust_i18n::t!("gui.offline.tools_setup_failed", error = err.to_string())
                        .to_string(),
                )
            }
        };

        self.step(
            InstallationStage::Configure,
            self.progress.finalizing(),
            rust_i18n::t!("gui.offline.finalizing").to_string(),
            rust_i18n::t!("gui.offline.completing_setup").to_string(),
        );

        idf_im_lib::single_version_post_install(
            paths.activation_script_path.to_string_lossy().as_ref(),
            paths.idf_path.to_string_lossy().as_ref(),
            &paths.actual_version,
            paths.tool_install_directory.to_string_lossy().as_ref(),
            export_paths,
            paths.python_venv_path.to_str(),
            Some(export_vars),
            &paths.python_path.to_string_lossy(),
            false, // create_cmd_bat
            true,  // is_offline_install
            true,  // is_gui
        );

        self.archive.log(
            MessageLevel::Success,
            rust_i18n::t!("gui.offline.version_configured", version = self.name).to_string(),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_tools_dir_without_versions_is_none() {
        let settings = Settings {
            idf_versions: None,
            ..Settings::default()
        };
        assert_eq!(offline_tools_dir(&settings), None);

        let settings = Settings {
            idf_versions: Some(vec![]),
            ..Settings::default()
        };
        assert_eq!(offline_tools_dir(&settings), None);
    }

    #[test]
    fn offline_tools_dir_without_base_path_is_none() {
        let settings = Settings {
            idf_versions: Some(vec!["v5.3".to_string()]),
            path: None,
            ..Settings::default()
        };
        assert_eq!(offline_tools_dir(&settings), None);
    }
}
