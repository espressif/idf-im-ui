use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct InstallationPlan {
    pub total_versions: usize,
    pub versions: Vec<String>,
    pub current_version_index: Option<usize>,
}

pub fn segment_progress(idx: usize, total: usize, range: (u32, u32)) -> (u32, u32) {
    let span = range.1 - range.0;
    let start = range.0 + ((idx as u32) * span / total as u32);
    let end   = range.0 + (((idx as u32 + 1) * span) / total as u32);
    (start, end)
}