use idf_im_lib::telemetry::{self, ErrorKind, FailureClass, FailureStage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter}; // dep: fork = "0.1"

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
    Success,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStage {
    Checking,
    Prerequisites,
    Download,
    Extract,
    Tools,
    Python,
    Configure,
    Complete,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallationProgress {
    pub stage: InstallationStage,
    pub percentage: u32,
    pub message: String,
    pub detail: Option<String>,
    pub version: Option<String>,
}

/// Emits a message to the frontend
pub fn emit_to_fe(app_handle: &AppHandle, event_name: &str, json_data: Value) {
    let _ = app_handle.emit(event_name, json_data);
}

/// Unified message emitter for all installation events
pub fn emit_installation_event(app_handle: &AppHandle, progress: InstallationProgress) {
    let _ = app_handle.emit("installation-progress", &progress);
}

/// Emits an error stage to the frontend and records why it happened for
/// telemetry.
///
/// Use this instead of a bare `emit_installation_event` with
/// `InstallationStage::Error`. The `message`/`detail` shown to the user are
/// localized, so they are useless for classification; `raw` must be the
/// untranslated error text (`err.to_string()`), and `kind`/`class`/`stage`
/// state what actually went wrong. `missing` carries tool names for
/// `DependencyMissing`.
pub fn emit_install_error(
    app_handle: &AppHandle,
    progress: InstallationProgress,
    kind: ErrorKind,
    class: FailureClass,
    stage: FailureStage,
    raw: impl Into<String>,
    missing: Vec<String>,
) {
    // Same normalization the CLI uses, so GUI and CLI produce identical
    // missingPrerequisites values for the same machine state.
    let missing = telemetry::normalize_missing_names(missing);
    telemetry::note_failure(kind, class, Some(stage), raw, missing);
    emit_installation_event(app_handle, progress);
}

/// `emit_install_error` for the common case where the kind implies the class
/// and there are no missing prerequisites to report.
pub fn emit_install_error_kind(
    app_handle: &AppHandle,
    progress: InstallationProgress,
    kind: ErrorKind,
    stage: FailureStage,
    raw: impl Into<String>,
) {
    emit_install_error(
        app_handle,
        progress,
        kind,
        kind.default_class(),
        stage,
        raw,
        Vec::new(),
    );
}

/// `emit_install_error` for wrappers around steps that classify themselves.
///
/// Keeps the inner step's classification when there is one, and only falls
/// back to `kind`/`class` when the failure came from somewhere that does not
/// classify. Use this wherever the error has already been flattened into a
/// localized string by an inner layer.
pub fn emit_install_error_fallback(
    app_handle: &AppHandle,
    progress: InstallationProgress,
    kind: ErrorKind,
    class: FailureClass,
    stage: FailureStage,
    raw: impl Into<String>,
) {
    telemetry::note_failure_if_absent(kind, class, Some(stage), raw);
    emit_installation_event(app_handle, progress);
}

/// Classifies a filesystem failure in our own pipeline. A full disk or a
/// denied path is the user's machine, anything else is an EIM defect.
pub fn classify_fs_failure(err: &std::io::Error) -> (ErrorKind, FailureClass) {
    match telemetry::kind_from_io_error(err) {
        Some(kind) => (kind, FailureClass::Environment),
        None => (ErrorKind::Filesystem, FailureClass::Installer),
    }
}

/// Same as `classify_fs_failure` for errors that have already been flattened
/// into a string and so no longer carry an `io::ErrorKind`.
pub fn classify_fs_failure_message(msg: &str) -> (ErrorKind, FailureClass) {
    match ErrorKind::from_message(msg) {
        kind @ (ErrorKind::DiskSpace | ErrorKind::Permission) => (kind, FailureClass::Environment),
        _ => (ErrorKind::Filesystem, FailureClass::Installer),
    }
}

/// Emit log messages (for detailed output)
pub fn emit_log_message(app_handle: &AppHandle, level: MessageLevel, message: String) {
    let _ = app_handle.emit(
        "log-message",
        json!({
            "level": level,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }),
    );
}

/// Legacy wrapper - gradually phase this out
pub fn send_message(app_handle: &AppHandle, message: String, message_type: String) {
    let level = match message_type.as_str() {
        "error" => MessageLevel::Error,
        "warning" => MessageLevel::Warning,
        "success" => MessageLevel::Success,
        _ => MessageLevel::Info,
    };
    emit_log_message(app_handle, level, message);
}

/// Progress bar for displaying installation progress
#[derive(Clone)]
pub struct ProgressBar {
    app_handle: AppHandle,
}

impl ProgressBar {
    /// Creates a new progress bar with the given message
    pub fn new(app_handle: AppHandle, message: &str) -> Self {
        let progress = Self { app_handle };
        progress.create(message);
        progress
    }

    /// Initializes the progress bar display
    pub fn create(&self, message: &str) {
        emit_to_fe(
            &self.app_handle,
            "progress-message",
            json!({
                "message": message,
                "status": "info",
                "percentage": 0,
                "display": true,
            }),
        );
    }

    /// Updates the progress bar with a new percentage and optional message
    pub fn update(&self, percentage: u64, message: Option<&str>) {
        emit_to_fe(
            &self.app_handle,
            "progress-message",
            json!({
                "percentage": percentage,
                "message": message.unwrap_or_default(),
                "status": "info",
                "display": true,
            }),
        );
    }
}
