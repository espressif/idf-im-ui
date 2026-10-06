// use crate::models::{settings::Settings, wizard::WizardData};
use idf_im_lib::settings::Settings;
use idf_im_lib::telemetry::InstallationContext;
use idf_im_lib::utils::MirrorEntry;
use log::error;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::AppHandle;

use tauri::Manager; // dep: fork = "0.1"

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct WizardData {
    /// Tracks which steps have been completed in the installation wizard
    pub step_completed: Vec<bool>,
}

/// Application state that is managed by Tauri and accessible across commands
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    pub wizard_data: Mutex<WizardData>,
    pub settings: Mutex<Settings>,
    pub is_installing: Mutex<bool>,
    pub is_simple_installation: Mutex<bool>,
    pub idf_mirror_latency_entries: Mutex<Option<Vec<MirrorEntry>>>,
    pub tools_mirror_latency_entries: Mutex<Option<Vec<MirrorEntry>>>,
    pub pypi_mirror_latency_entries: Mutex<Option<Vec<MirrorEntry>>>,
    #[serde(skip)]
    pub telemetry_session: Mutex<Option<InstallationContext>>,
}

fn store_locked<T>(slot: &Mutex<T>, value: T) -> Result<(), String> {
    *slot.lock().map_err(|_| "Lock error".to_string())? = value;
    Ok(())
}

fn store_in_state<T>(
    app_handle: &AppHandle,
    slot: fn(&AppState) -> &Mutex<T>,
    value: T,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    store_locked(slot(&app_state), value)
}

fn store_mirror_entries(
    app_handle: &AppHandle,
    slot: fn(&AppState) -> &Mutex<Option<Vec<MirrorEntry>>>,
    entries: &[MirrorEntry],
) -> Result<(), String> {
    store_in_state(app_handle, slot, Some(entries.to_vec()))
}

fn read_from_state<T: Clone>(
    app_handle: &AppHandle,
    slot: fn(&AppState) -> &Mutex<T>,
    fallback: T,
    poisoned_message: &str,
) -> T {
    let app_state = app_handle.state::<AppState>();
    read_locked(slot(&app_state), fallback, poisoned_message)
}

fn read_locked<T: Clone>(slot: &Mutex<T>, fallback: T, poisoned_message: &str) -> T {
    slot.lock().map(|guard| guard.clone()).unwrap_or_else(|_| {
        error!("{}", poisoned_message);
        fallback
    })
}

pub fn set_idf_mirror_latency_entries(
    app_handle: &AppHandle,
    entries: &[MirrorEntry],
) -> Result<(), String> {
    store_mirror_entries(app_handle, |s| &s.idf_mirror_latency_entries, entries)
}

pub fn set_tools_mirror_latency_entries(
    app_handle: &AppHandle,
    entries: &[MirrorEntry],
) -> Result<(), String> {
    store_mirror_entries(app_handle, |s| &s.tools_mirror_latency_entries, entries)
}

pub fn set_pypi_mirror_latency_entries(
    app_handle: &AppHandle,
    entries: &[MirrorEntry],
) -> Result<(), String> {
    store_mirror_entries(app_handle, |s| &s.pypi_mirror_latency_entries, entries)
}

pub fn get_idf_mirror_latency_entries(app_handle: &AppHandle) -> Option<Vec<MirrorEntry>> {
    read_from_state(
        app_handle,
        |state| &state.idf_mirror_latency_entries,
        None,
        "Failed to acquire idf_mirror_latency_entries lock, returning None",
    )
}

pub fn get_tools_mirror_latency_entries(app_handle: &AppHandle) -> Option<Vec<MirrorEntry>> {
    read_from_state(
        app_handle,
        |state| &state.tools_mirror_latency_entries,
        None,
        "Failed to acquire tools_mirror_latency_entries lock, returning None",
    )
}

pub fn get_pypi_mirror_latency_entries(app_handle: &AppHandle) -> Option<Vec<MirrorEntry>> {
    read_from_state(
        app_handle,
        |state| &state.pypi_mirror_latency_entries,
        None,
        "Failed to acquire pypi_mirror_latency_entries lock, returning None",
    )
}

pub fn set_is_simple_installation(app_handle: &AppHandle, is_simple: bool) -> Result<(), String> {
    store_in_state(app_handle, |state| &state.is_simple_installation, is_simple)
}

pub fn is_simple_installation(app_handle: &AppHandle) -> bool {
    read_from_state(
        app_handle,
        |state| &state.is_simple_installation,
        false,
        "Failed to acquire is_simple_installation lock, assuming false",
    )
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
    read_from_state(
        app_handle,
        |state| &state.is_installing,
        false,
        "Failed to acquire is_installing lock, assuming not installing",
    )
}

/// Sets the installation status
pub fn set_installation_status(app_handle: &AppHandle, status: bool) -> Result<(), String> {
    store_in_state(app_handle, |state| &state.is_installing, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poisoned<T>(value: T) -> Mutex<T> {
        let slot = Mutex::new(value);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = slot.lock().unwrap();
            panic!("poison the mutex");
        }));
        assert!(slot.is_poisoned());
        slot
    }

    #[test]
    fn store_locked_replaces_value() {
        let slot = Mutex::new(false);
        assert_eq!(store_locked(&slot, true), Ok(()));
        assert!(*slot.lock().unwrap());
    }

    #[test]
    fn store_locked_reports_poisoned_mutex() {
        let slot = poisoned(false);
        assert_eq!(store_locked(&slot, true), Err("Lock error".to_string()));
    }

    #[test]
    fn read_locked_returns_current_value() {
        let slot = Mutex::new(Some(vec![1, 2]));
        assert_eq!(read_locked(&slot, None, "unused"), Some(vec![1, 2]));
    }

    #[test]
    fn read_locked_returns_fallback_when_poisoned() {
        let slot = poisoned(true);
        assert!(!read_locked(&slot, false, "poisoned"));
    }
}
