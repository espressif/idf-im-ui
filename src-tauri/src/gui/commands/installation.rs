use tauri::AppHandle;
use crate::gui::app_state;

// Checks if an installation is currently in progress
#[tauri::command]
pub fn is_installing(app_handle: AppHandle) -> bool {
    app_state::is_installation_in_progress(&app_handle)
}
