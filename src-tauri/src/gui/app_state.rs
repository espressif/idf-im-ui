// use crate::models::{settings::Settings, wizard::WizardData};
use log::error;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::AppHandle;
use idf_im_lib::settings::Settings;

use tauri::Manager; // dep: fork = "0.1"


#[derive(Default, Clone, Serialize, Deserialize)]
pub struct WizardData {
    /// Tracks which steps have been completed in the installation wizard
    pub step_completed: Vec<bool>,
}

/// Application state that is managed by Tauri and accessible across commands
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    wizard_data: Mutex<WizardData>,
    settings: Mutex<Settings>,
    is_installing: Mutex<bool>,
}

/// Gets the current settings from the app state
///
/// This function acquires a lock on the settings mutex, which may block if another
/// thread is currently modifying the settings.
pub fn get_locked_settings(app_handle: &AppHandle) -> Result<Settings, String> {
    let app_state = app_handle.state::<AppState>();
    app_state
        .settings
        .lock()
        .map(|guard| (*guard).clone())
        .map_err(|_| {
            "Failed to obtain lock on AppState. Please retry the last action later.".to_string()
        })
}

/// Gets the current settings without blocking
///
/// This function tries to acquire a lock on the settings mutex without blocking.
/// If the lock is currently held, it will retry a few times with a small delay.
pub fn get_settings_non_blocking(app_handle: &AppHandle) -> Result<Settings, String> {
    let app_state = app_handle.state::<AppState>();

    // First try with a non-blocking try_lock
    if let Ok(guard) = app_state.settings.try_lock() {
        let settings_copy = (*guard).clone();
        return Ok(settings_copy);
    }

    // If we couldn't get the lock immediately, wait a little and retry
    for _ in 0..5 {
        // Small sleep to avoid busy waiting
        std::thread::sleep(std::time::Duration::from_millis(10));

        if let Ok(guard) = app_state.settings.try_lock() {
            let settings_copy = (*guard).clone();
            return Ok(settings_copy);
        }
    }

    Err("Settings are currently locked. Try again later.".to_string())
}

/// Updates the settings using a provided function
///
/// This function acquires a lock on the settings mutex and then applies the provided
/// update function to modify the settings.
pub fn update_settings<F>(app_handle: &AppHandle, updater: F) -> Result<(), String>
where
    F: FnOnce(&mut Settings),
{
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state.settings.lock().map_err(|_| {
        "Failed to obtain lock on AppState. Please retry the last action later.".to_string()
    })?;
    updater(&mut settings);
    log::debug!("Settings after update: {:?}", settings);
    Ok(())
}

/// Checks if installation is currently in progress
pub fn is_installation_in_progress(app_handle: &AppHandle) -> bool {
    let app_state = app_handle.state::<AppState>();
    app_state
        .is_installing
        .lock()
        .map(|guard| *guard)
        .unwrap_or_else(|_| {
            error!("Failed to acquire is_installing lock, assuming not installing");
            false
        })
}

/// Sets the installation status
pub fn set_installation_status(app_handle: &AppHandle, status: bool) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let mut is_installing = app_state
        .is_installing
        .lock()
        .map_err(|_| "Lock error".to_string())?;
    *is_installing = status;
    Ok(())
}
