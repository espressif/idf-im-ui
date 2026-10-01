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
