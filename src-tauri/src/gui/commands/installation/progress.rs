use tauri::AppHandle;

use crate::gui::{
    app_state::set_installation_status,
    ui::{emit_installation_event, InstallationProgress, InstallationStage},
};

pub(super) fn emit_progress(
    app_handle: &AppHandle,
    stage: InstallationStage,
    percentage: u32,
    message: String,
    detail: Option<String>,
    version: Option<String>,
) {
    emit_installation_event(
        app_handle,
        InstallationProgress {
            stage,
            percentage,
            message,
            detail,
            version,
        },
    );
}

/// Emits an `Error` stage event (always at 0%).
pub(super) fn emit_error(
    app_handle: &AppHandle,
    message: String,
    detail: Option<String>,
    version: Option<String>,
) {
    emit_progress(
        app_handle,
        InstallationStage::Error,
        0,
        message,
        detail,
        version,
    );
}

/// Clears the "installing" flag and fails with `error`. A failure to clear the
/// flag takes precedence over `error`.
pub(super) fn abort<T>(app_handle: &AppHandle, error: String) -> Result<T, String> {
    set_installation_status(app_handle, false)?;
    Err(error)
}

/// Percentage of a step `offset` (0-90) inside archive `archive_index` of `total_archives`.
pub(super) fn archive_percentage(
    archive_index: usize,
    total_archives: usize,
    offset: usize,
) -> u32 {
    ((archive_index * 90 + offset) / total_archives) as u32
}

/// Percentages for the per-version steps of one offline archive, which share
/// the archive's 40-85 slot.
#[derive(Debug, Clone, Copy)]
pub(super) struct OfflineVersionProgress {
    start: u32,
    end: u32,
    count: u32,
}

impl OfflineVersionProgress {
    pub(super) fn new(archive_index: usize, total_archives: usize, versions: usize) -> Self {
        Self {
            start: archive_percentage(archive_index, total_archives, 40),
            end: archive_percentage(archive_index, total_archives, 85),
            count: versions as u32,
        }
    }

    fn range(&self) -> u32 {
        self.end - self.start
    }

    pub(super) fn processing(&self, version_index: usize) -> u32 {
        self.start + (version_index as u32 * self.range() / self.count)
    }

    pub(super) fn copying_tools(&self, version_index: usize) -> u32 {
        self.start + ((version_index + 1) as u32 * self.range() / (self.count * 3))
    }

    pub(super) fn configuring_tools(&self, version_index: usize) -> u32 {
        self.start + ((version_index + 1) as u32 * self.range() * 2 / (self.count * 3))
    }

    pub(super) fn finalizing(&self) -> u32 {
        self.end - 5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_percentage_scales_offset_by_archive_count() {
        assert_eq!(archive_percentage(0, 1, 10), 10);
        assert_eq!(archive_percentage(0, 1, 88), 88);
        assert_eq!(archive_percentage(1, 2, 20), 55);
        assert_eq!(archive_percentage(2, 3, 35), 71);
    }

    #[test]
    fn offline_version_progress_single_archive_single_version() {
        let p = OfflineVersionProgress::new(0, 1, 1);
        assert_eq!(p.processing(0), 40);
        assert_eq!(p.copying_tools(0), 55);
        assert_eq!(p.configuring_tools(0), 70);
        assert_eq!(p.finalizing(), 80);
    }

    #[test]
    fn offline_version_progress_multiple_versions_in_second_archive() {
        let p = OfflineVersionProgress::new(1, 2, 2);
        // start = 130 / 2 = 65, end = 175 / 2 = 87, range = 22
        assert_eq!(p.processing(0), 65);
        assert_eq!(p.processing(1), 76);
        assert_eq!(p.copying_tools(0), 65 + 22 / 6);
        assert_eq!(p.copying_tools(1), 65 + 44 / 6);
        assert_eq!(p.configuring_tools(1), 65 + 88 / 6);
        assert_eq!(p.finalizing(), 82);
    }
}
