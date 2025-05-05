use log::{debug, info};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter}; // dep: fork = "0.1"


/// Emits a message to the frontend
pub fn emit_to_fe(app_handle: &AppHandle, event_name: &str, json_data: Value) {
    debug!("emit_to_fe: {} {:?}", event_name, json_data); //TODO: remove debug
    let _ = app_handle.emit(event_name, json_data);
}

/// Sends a user message with a specified type
pub fn send_message(app_handle: &AppHandle, message: String, message_type: String) {
    emit_to_fe(
        app_handle,
        "user-message",
        json!({ "type": message_type, "message": message }),
    );
}

/// Sends a tools-related message
pub fn send_tools_message(app_handle: &AppHandle, tool: String, action: String) {
    emit_to_fe(
        app_handle,
        "tools-message",
        json!({ "tool": tool, "action": action }),
    );
}

/// Sends an installation progress message for a specific version
pub fn send_install_progress_message(app_handle: &AppHandle, version: String, state: String) {
    emit_to_fe(
        app_handle,
        "install-progress-message",
        json!({ "version": version, "state": state }),
    );
}

/// Sends a simple setup message with a code and message
pub fn send_simple_setup_message(app_handle: &AppHandle, message_code: i32, message: String) {
    emit_to_fe(
        app_handle,
        "simple-setup-message",
        json!({ "code": message_code, "message": message }),
    );
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

    /// Completes the progress bar and hides it
    pub fn finish(&self) {
        info!("finish_progress_bar called");
        emit_to_fe(
            &self.app_handle,
            "progress-message",
            json!({
                "message": "",
                "percentage": 100,
                "display": false,
            }),
        );
    }
}
