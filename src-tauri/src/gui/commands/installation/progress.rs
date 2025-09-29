use tauri::AppHandle;
use crate::gui::ui::{
    emit_installation_event, emit_log_message, InstallationProgress, InstallationStage, MessageLevel,
};

#[derive(Clone)]
pub struct ProgressEmitter {
    app: AppHandle,
}

impl ProgressEmitter {
    pub fn new(app: AppHandle) -> Self { Self { app } }

    pub fn info<S: Into<String>>(&self, msg: S)    { emit_log_message(&self.app, MessageLevel::Info, msg.into()); }
    pub fn warn<S: Into<String>>(&self, msg: S)    { emit_log_message(&self.app, MessageLevel::Warning, msg.into()); }
    pub fn error<S: Into<String>>(&self, msg: S)   { emit_log_message(&self.app, MessageLevel::Error, msg.into()); }
    pub fn success<S: Into<String>>(&self, msg: S) { emit_log_message(&self.app, MessageLevel::Success, msg.into()); }

    pub fn stage<S1, S2>(
        &self,
        stage: InstallationStage,
        pct: u32,
        message: S1,
        detail: Option<S2>,
        version: Option<String>,
    )
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        emit_installation_event(
            &self.app,
            InstallationProgress {
                stage,
                percentage: pct,
                message: message.into(),
                detail: detail.map(|d| d.into()),
                version,
            },
        );
    }
}