use crate::gui::{
    app_state::{self, get_locked_settings, get_settings_non_blocking, update_settings, AppState},
    ui::send_message,
    utils::is_path_empty_or_nonexistent,
};
use idf_im_lib::{
    settings::{self, Settings},
    to_absolute_path,
    utils::{is_valid_idf_directory, MirrorEntry},
};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use log::info;
use rust_i18n::t;
use serde_json::{json, Value};

fn settings_or_report(app_handle: &AppHandle) -> Option<Settings> {
    get_settings_non_blocking(app_handle)
        .map_err(|e| send_message(app_handle, e, "error".to_string()))
        .ok()
}

fn read_setting<T>(app_handle: &AppHandle, fallback: T, read: impl FnOnce(Settings) -> T) -> T {
    settings_or_report(app_handle).map_or(fallback, read)
}

pub(crate) fn update_settings_and_notify<F>(
    app_handle: &AppHandle,
    success_message: String,
    updater: F,
) -> Result<(), String>
where
    F: FnOnce(&mut Settings),
{
    update_settings(app_handle, updater)?;
    send_message(app_handle, success_message, "info".to_string());
    Ok(())
}

fn prepend_if_missing<'a>(mut mirrors: Vec<&'a str>, selected: &'a str) -> Vec<&'a str> {
    if !mirrors.contains(&selected) {
        mirrors.insert(0, selected);
    }
    mirrors
}

fn mirror_urls_json(selected: &str, known_mirrors: &[&str]) -> Value {
    let available_mirrors = if selected.is_empty() {
        known_mirrors.to_vec()
    } else {
        prepend_if_missing(known_mirrors.to_vec(), selected)
    };
    json!({
      "mirrors": available_mirrors,
      "selected": selected,
    })
}

fn mirror_urls(
    app_handle: &AppHandle,
    selected_mirror: fn(&Settings) -> Option<String>,
    known_mirrors: &[&str],
) -> Value {
    let Some(settings) = settings_or_report(app_handle) else {
        return json!({
            "mirrors": Vec::<String>::new(),
            "selected": "",
        });
    };
    let selected = selected_mirror(&settings).unwrap_or_default();
    mirror_urls_json(&selected, known_mirrors)
}

async fn mirror_latency_entries(
    app_handle: &AppHandle,
    selected_mirror: fn(&Settings) -> Option<String>,
    known_mirrors: &[&str],
    store_entries: fn(&AppHandle, &[MirrorEntry]) -> Result<(), String>,
) -> Value {
    let Some(settings) = settings_or_report(app_handle) else {
        return json!({
            "entries": Vec::<String>::new(),
        });
    };
    let mirror = selected_mirror(&settings).unwrap_or_default();
    let available_mirrors = prepend_if_missing(known_mirrors.to_vec(), &mirror);

    let mirror_latency_entries =
        idf_im_lib::utils::calculate_mirrors_latency(&available_mirrors).await;
    if let Err(e) = store_entries(app_handle, &mirror_latency_entries) {
        send_message(app_handle, e, "error".to_string());
    }
    json!({
        "entries": mirror_latency_entries,
    })
}

/// Gets the current settings
#[tauri::command]
pub fn get_settings(app_handle: tauri::AppHandle) -> settings::Settings {
    get_settings_non_blocking(&app_handle).unwrap_or_default()
}

/// Loads settings from a file
#[tauri::command]
pub fn load_settings(app_handle: AppHandle, path: &str) -> Result<(), String> {
    let mut load_error: Option<String> = None;

    update_settings(&app_handle, |settings| {
        if let Err(e) = settings.load(path) {
            log::error!("settings failed to load: {:?}", e);
            load_error = Some(e.to_string());
        }
    })
    .map_err(|e| {
        log::error!("Failed to update settings: {}", e);
        format!("Failed to update settings: {}", e)
    })?;

    if let Some(e) = load_error {
        send_message(
            &app_handle,
            t!("gui.settings.failed_to_load", path = path).to_string(),
            "error".to_string(),
        );
        return Err(e);
    }

    send_message(
        &app_handle,
        t!("gui.settings.loaded_successfully", path = path).to_string(),
        "info".to_string(),
    );
    Ok(())
}

/// Saves the current config to a file
#[tauri::command]
pub fn save_config(app_handle: tauri::AppHandle, path: String) {
    let mut settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(_) => {
            return send_message(
                &app_handle,
                t!("gui.settings.save_config_error").to_string(),
                "error".to_string(),
            )
        }
    };

    settings.config_file_save_path = Some(PathBuf::from(path));
    let _ = settings.save();
}

/// Gets the installation path
#[tauri::command]
pub fn get_installation_path(app_handle: AppHandle) -> String {
    read_setting(&app_handle, String::new(), |settings| {
        let path = settings.path.unwrap_or_default();
        path.to_str().unwrap_or_default().to_string()
    })
}

/// Sets the installation path
#[tauri::command]
pub fn set_installation_path(app_handle: AppHandle, path: String) -> Result<(), String> {
    info!("Setting installation path: {}", path);
    update_settings(&app_handle, |settings| {
        let p = match to_absolute_path(&path) {
            Ok(p) => p,
            Err(e) => {
                send_message(
                    &app_handle,
                    t!(
                        "gui.settings.failed_to_set_installation_path",
                        error = e.to_string()
                    )
                    .to_string(),
                    "error".to_string(),
                );
                return;
            }
        };
        settings.path = Some(PathBuf::from(p));
    })?;

    send_message(
        &app_handle,
        t!("gui.settings.installation_path_updated").to_string(),
        "info".to_string(),
    );
    Ok(())
}

/// Gets the list of available IDF targets
#[tauri::command]
pub async fn get_available_targets(app_handle: AppHandle) -> Vec<Value> {
    let Some(settings) = settings_or_report(&app_handle) else {
        return Vec::new();
    };

    let targets = settings.target.clone().unwrap_or_default();
    let available_targets = idf_im_lib::idf_versions::get_available_targets()
        .await
        .unwrap_or_default();

    available_targets
        .into_iter()
        .map(|t| {
            json!({
              "name": t,
              "selected": targets.contains(&t),
            })
        })
        .collect()
}

/// Sets the selected targets
#[tauri::command]
pub fn set_targets(app_handle: AppHandle, targets: Vec<String>) -> Result<(), String> {
    info!("Setting targets: {:?}", targets);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.targets_updated").to_string(),
        |settings| settings.target = Some(targets),
    )
}

/// Gets the list of available IDF versions
#[tauri::command]
pub async fn get_idf_versions(app_handle: AppHandle, include_unstable: bool) -> Vec<Value> {
    let Some(settings) = settings_or_report(&app_handle) else {
        return Vec::new();
    };

    let targets = settings.target.clone().unwrap_or_default();
    let selected_versions = settings.idf_versions.clone().unwrap_or_default();
    let targets_vec: Vec<String> = targets.to_vec();

    // Get full version information
    let releases = match idf_im_lib::idf_versions::get_idf_versions().await {
        Ok(r) => r,
        Err(e) => {
            send_message(&app_handle, e, "error".to_string());
            return Vec::new();
        }
    };
    let mut first = true;
    let mut available_versions: Vec<Value> = releases
        .versions
        .iter()
        .filter(|v| {
            // Filter out end_of_life and old versions
            if v.end_of_life || v.name == "latest" {
                return false;
            }
            // Filter out pre-release if not requested
            if !include_unstable && v.pre_release {
                return false;
            }
            // Filter by target if specified
            if !targets_vec.is_empty() && !targets_vec.contains(&"all".to_string()) {
                return v
                    .supported_targets
                    .iter()
                    .any(|t| targets_vec.contains(&t.to_lowercase()));
            }
            true
        })
        .map(|v| {
            // Determine if this is the latest stable version
            let is_latest = !v.pre_release && first;
            if is_latest {
                first = false;
            }

            json!({
              "name": v.name,
              "pre_release": v.pre_release,
              "old": v.old,
              "end_of_life": v.end_of_life,
              "has_targets": v.has_targets,
              "supported_targets": v.supported_targets,
              "selected": selected_versions.contains(&v.name),
              "latest": is_latest,
              "category": if v.pre_release { "pre_release" } else { "stable" }
            })
        })
        .collect();

    // Add master branch option
    available_versions.push(json!({
      "name": "master",
      "pre_release": false,
      "old": false,
      "end_of_life": false,
      "has_targets": true,
      "supported_targets": ["all"],
      "selected": selected_versions.contains(&"master".to_string()),
      "latest": false,
      "category": "development",
      "is_master": true
    }));

    available_versions
}

/// Sets the selected IDF versions
#[tauri::command]
pub fn set_versions(app_handle: AppHandle, versions: Vec<String>) -> Result<(), String> {
    info!("Setting IDF versions: {:?}", versions);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.idf_versions_updated").to_string(),
        |settings| settings.idf_versions = Some(versions),
    )
}

/// Gets latency entries for available IDF mirrors
#[tauri::command]
pub async fn get_idf_mirror_latency_entries(app_handle: AppHandle) -> Value {
    mirror_latency_entries(
        &app_handle,
        |settings| settings.idf_mirror.clone(),
        idf_im_lib::get_idf_mirrors_list(),
        app_state::set_idf_mirror_latency_entries,
    )
    .await
}

/// Returns only the available IDF mirror URLs quickly (no latency calculation)
#[tauri::command]
pub async fn get_idf_mirror_urls(app_handle: AppHandle) -> Value {
    mirror_urls(
        &app_handle,
        |settings| settings.idf_mirror.clone(),
        idf_im_lib::get_idf_mirrors_list(),
    )
}

/// Sets the selected IDF mirror
#[tauri::command]
pub async fn set_idf_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
    info!("Setting IDF mirror: {}", mirror);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.idf_mirror_updated").to_string(),
        |settings| settings.idf_mirror = Some(mirror),
    )
}

/// Gets latency entries for available tools mirrors
#[tauri::command]
pub async fn get_tools_mirror_latency_entries(app_handle: AppHandle) -> Value {
    mirror_latency_entries(
        &app_handle,
        |settings| settings.mirror.clone(),
        idf_im_lib::get_idf_tools_mirrors_list(),
        app_state::set_tools_mirror_latency_entries,
    )
    .await
}

/// Returns only the available tools mirror URLs quickly (no latency
/// calculation)
#[tauri::command]
pub async fn get_tools_mirror_urls(app_handle: AppHandle) -> Value {
    mirror_urls(
        &app_handle,
        |settings| settings.mirror.clone(),
        idf_im_lib::get_idf_tools_mirrors_list(),
    )
}

/// Sets the selected tools mirror
#[tauri::command]
pub async fn set_tools_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
    info!("Setting tools mirror: {}", mirror);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.tools_mirror_updated").to_string(),
        |settings| settings.mirror = Some(mirror),
    )
}

/// Gets latency entries for available PyPI mirrors
#[tauri::command]
pub async fn get_pypi_mirror_latency_entries(app_handle: AppHandle) -> Value {
    mirror_latency_entries(
        &app_handle,
        |settings| settings.pypi_mirror.clone(),
        idf_im_lib::get_pypi_mirrors_list(),
        app_state::set_pypi_mirror_latency_entries,
    )
    .await
}

/// Returns only the available PyPI mirror URLs quickly (no latency calculation)
#[tauri::command]
pub async fn get_pypi_mirror_urls(app_handle: AppHandle) -> Value {
    mirror_urls(
        &app_handle,
        |settings| settings.pypi_mirror.clone(),
        idf_im_lib::get_pypi_mirrors_list(),
    )
}

/// Sets the selected PyPI mirror
#[tauri::command]
pub async fn set_pypi_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
    info!("Setting pypi mirror: {}", mirror);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.pypi_mirror_updated").to_string(),
        |settings| settings.pypi_mirror = Some(mirror),
    )
}

/// Checks if a path is empty or doesn't exist
#[tauri::command]
pub async fn is_path_empty_or_nonexistent_command(
    app_handle: AppHandle,
    path: String,
    versions: Option<Vec<String>>,
) -> bool {
    let settings = match get_settings_non_blocking(&app_handle) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let versions = match versions {
        Some(v) => v,
        None => match &settings.idf_versions {
            Some(v) => v.clone(),
            None => {
                send_message(
                    &app_handle,
                    t!("gui.settings.no_idf_versions_selected").to_string(),
                    "error".to_string(),
                );
                // return false;
                [].to_vec()
            }
        },
    };

    is_path_empty_or_nonexistent(&path, &versions)
}

#[tauri::command]
pub async fn is_path_idf_directory(_app_handle: AppHandle, path: String) -> bool {
    is_valid_idf_directory(&path)
}

/// Resets the settings to their default values
#[tauri::command]
pub fn reset_settings_to_default(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state.settings.lock().map_err(|_| {
        "Failed to obtain lock on AppState. Please retry the last action later.".to_string()
    })?;

    *settings = Settings::default();
    log::info!("Settings reset to default values");

    Ok(())
}

/// Gets the tool download folder name
#[tauri::command]
pub fn get_tool_download_folder_name(app_handle: AppHandle) -> String {
    read_setting(&app_handle, String::new(), |settings| {
        settings.tool_download_folder_name.unwrap_or_default()
    })
}

/// Sets the tool download folder name
#[tauri::command]
pub fn set_tool_download_folder_name(app_handle: AppHandle, name: String) -> Result<(), String> {
    info!("Setting tool download folder name: {}", name);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.tool_download_folder_name_updated").to_string(),
        |settings| settings.tool_download_folder_name = Some(name),
    )
}

/// Gets the tool install folder name
#[tauri::command]
pub fn get_tool_install_folder_name(app_handle: AppHandle) -> String {
    read_setting(&app_handle, String::new(), |settings| {
        settings.tool_install_folder_name.unwrap_or_default()
    })
}

/// Sets the tool install folder name
#[tauri::command]
pub fn set_tool_install_folder_name(app_handle: AppHandle, name: String) -> Result<(), String> {
    info!("Setting tool install folder name: {}", name);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.tool_install_folder_name_updated").to_string(),
        |settings| settings.tool_install_folder_name = Some(name),
    )
}

/// Gets the cleanup flag (delete temporary files after install)
#[tauri::command]
pub fn get_cleanup(app_handle: AppHandle) -> bool {
    read_setting(&app_handle, false, |settings| {
        settings.cleanup.unwrap_or(false)
    })
}

/// Sets the cleanup flag (delete temporary files after install)
#[tauri::command]
pub fn set_cleanup(app_handle: AppHandle, cleanup: bool) -> Result<(), String> {
    info!("Setting cleanup: {}", cleanup);
    update_settings_and_notify(
        &app_handle,
        t!("gui.settings.cleanup_updated").to_string(),
        |settings| settings.cleanup = Some(cleanup),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepend_if_missing_keeps_known_mirror_order() {
        let mirrors = prepend_if_missing(vec!["a", "b"], "b");
        assert_eq!(mirrors, vec!["a", "b"]);
    }

    #[test]
    fn prepend_if_missing_puts_custom_mirror_first() {
        let mirrors = prepend_if_missing(vec!["a", "b"], "custom");
        assert_eq!(mirrors, vec!["custom", "a", "b"]);
    }

    #[test]
    fn prepend_if_missing_prepends_empty_selection() {
        let mirrors = prepend_if_missing(vec!["a"], "");
        assert_eq!(mirrors, vec!["", "a"]);
    }

    #[test]
    fn mirror_urls_json_skips_empty_selection() {
        assert_eq!(
            mirror_urls_json("", &["a", "b"]),
            json!({ "mirrors": ["a", "b"], "selected": "" })
        );
    }

    #[test]
    fn mirror_urls_json_prepends_custom_selection() {
        assert_eq!(
            mirror_urls_json("custom", &["a"]),
            json!({ "mirrors": ["custom", "a"], "selected": "custom" })
        );
    }

    #[test]
    fn mirror_urls_json_keeps_known_selection() {
        assert_eq!(
            mirror_urls_json("a", &["a", "b"]),
            json!({ "mirrors": ["a", "b"], "selected": "a" })
        );
    }
}
