use anyhow::{anyhow, Context, Error, Result};
#[cfg(target_os = "linux")]
use fork::{daemon, Fork};
use idf_im_lib::{
    add_path_to_path, download_file, ensure_path, expand_tilde, idf_tools::get_tools_export_paths,
    python_utils::run_idf_tools_py, settings::Settings, verify_file_checksum, DownloadProgress,
    ProgressMessage,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::metadata;
use std::fs::OpenOptions;
use std::process::Command;
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
    thread,
};
use tauri::{AppHandle, Manager}; // dep: fork = "0.1"

// Types and structs
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    wizard_data: Mutex<WizardData>,
    settings: Mutex<Settings>,
    installation_queue: Mutex<Vec<String>>,
    is_installing: Mutex<bool>,
}

// Then add a command to check installation status
#[tauri::command]
fn is_installing(app_handle: AppHandle) -> bool {
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

#[derive(Default, Serialize, Deserialize)]
struct WizardData {
    // Add fields relevant to your installation process
    step_completed: Vec<bool>,
}

// Event handling
fn send_message(app_handle: &AppHandle, message: String, message_type: String) {
    let _ = emit_to_fe(
        app_handle,
        "user-message",
        json!({ "type": message_type, "message": message }),
    );
}

fn send_tools_message(app_handle: &AppHandle, tool: String, action: String) {
    let _ = emit_to_fe(
        app_handle,
        "tools-message",
        json!({ "tool": tool, "action": action }),
    );
}

fn send_install_progress_message(app_handle: &AppHandle, version: String, state: String) {
    let _ = emit_to_fe(
        app_handle,
        "install-progress-message",
        json!({ "version": version, "state": state }),
    );
}

fn send_simple_setup_message(app_handle: &AppHandle, message_code: i32, message: String) {
    let _ = emit_to_fe(
        app_handle,
        "simple-setup-message",
        json!({ "code": message_code, "message": message }),
    );
}

fn emit_to_fe(app_handle: &AppHandle, event_name: &str, json_data: Value) {
    debug!("emit_to_fe: {} {:?}", event_name, json_data);
    let _ = app_handle.emit(event_name, json_data);
}

#[derive(Clone)]
struct ProgressBar {
    app_handle: AppHandle,
}

impl<'a> ProgressBar {
    fn new(app_handle: AppHandle, message: &str) -> Self {
        let progress = Self { app_handle };
        progress.create(message);
        progress
    }

    fn create(&self, message: &str) {
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

    fn update(&self, percentage: u64, message: Option<&str>) {
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

    fn finish(&self) {
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

// Settings management
fn get_locked_settings(app_handle: &AppHandle) -> Result<Settings, String> {
    let app_state = app_handle.state::<AppState>();
    app_state
        .settings
        .lock()
        .map(|guard| (*guard).clone())
        .map_err(|_| {
            "Failed to obtain lock on AppState. Please retry the last action later.".to_string()
        })
}

fn update_settings<F>(app_handle: &AppHandle, updater: F) -> Result<(), String>
where
    F: FnOnce(&mut Settings),
{
    let app_state = app_handle.state::<AppState>();
    let mut settings = app_state.settings.lock().map_err(|_| {
        "Failed to obtain lock on AppState. Please retry the last action later.".to_string()
    })?;
    updater(&mut settings);
    debug!("Settings after update: {:?}", settings);
    Ok(())
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    log::info!("Greet called with name: {}", name);
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_settings(app_handle: tauri::AppHandle) -> Settings {
    get_locked_settings(&app_handle).unwrap_or_default()
}

#[tauri::command]
fn get_prequisites() -> Vec<&'static str> {
    idf_im_lib::system_dependencies::get_prequisites()
}

#[tauri::command]
fn get_operating_system() -> String {
    std::env::consts::OS.to_string()
}
// ----
#[tauri::command]
fn install_prerequisites(app_handle: AppHandle) -> bool {
    let unsatisfied_prerequisites = idf_im_lib::system_dependencies::check_prerequisites()
        .unwrap()
        .into_iter()
        .map(|p| p.to_string())
        .collect();
    match idf_im_lib::system_dependencies::install_prerequisites(unsatisfied_prerequisites) {
        Ok(_) => true,
        Err(err) => {
            send_message(
                &app_handle,
                format!("Error installing prerequisites: {}", err),
                "error".to_string(),
            );
            log::error!("Error installing prerequisites: {}", err);
            false
        }
    }
}

#[tauri::command]
fn check_prequisites(app_handle: AppHandle) -> Vec<String> {
    match idf_im_lib::system_dependencies::check_prerequisites() {
        Ok(prerequisites) => {
            if prerequisites.is_empty() {
                vec![]
            } else {
                prerequisites.into_iter().map(|p| p.to_string()).collect()
            }
        }
        Err(err) => {
            send_message(
                &app_handle,
                format!("Error checking prerequisites: {}", err),
                "error".to_string(),
            );
            log::error!("Error checking prerequisites: {}", err);
            vec![]
        }
    }
}

#[tauri::command]
fn python_sanity_check(app_handle: AppHandle, python: Option<&str>) -> bool {
    let outpusts = idf_im_lib::python_utils::python_sanity_check(python);
    let mut all_ok = true;
    for output in outpusts {
        match output {
            Ok(_) => {}
            Err(err) => {
                all_ok = false;
                send_message(
                    &app_handle,
                    format!("Python sanity check failed: {}", err),
                    "warning".to_string(),
                );
                log::warn!("{:?}", err)
            }
        }
    }
    all_ok
}

#[tauri::command]
fn get_logs_folder(app_handle: AppHandle) -> PathBuf {
    match idf_im_lib::get_log_directory() {
        Some(folder) => folder,
        None => {
            send_message(
                &app_handle,
                format!("Error getting log folder"),
                "error".to_string(),
            );
            log::error!("Error getting log folder"); //TODO: emit message
            PathBuf::new()
        }
    }
}

#[tauri::command]
fn python_install(app_handle: AppHandle) -> bool {
    match idf_im_lib::system_dependencies::install_prerequisites(vec!["python".to_string()]) {
        Ok(_) => true,
        Err(err) => {
            send_message(
                &app_handle,
                format!("Error installing python: {}", err),
                "error".to_string(),
            );
            log::error!("Error installing python: {}", err); //TODO: emit message
            false
        }
    }
}
#[tauri::command]
async fn get_available_targets(app_handle: AppHandle) -> Vec<Value> {
    let app_state = app_handle.state::<AppState>();
    // Clone the settings to avoid holding the MutexGuard across await points
    let settings = {
        let guard = app_state
            .settings
            .lock()
            .map_err(|_| {
                send_message(
                    &app_handle,
                    "Failed to obtain lock on AppState. Please retry the last action later."
                        .to_string(),
                    "error".to_string(),
                )
            })
            .expect("Failed to lock settings");
        guard.clone()
    };
    let targets = settings.target.clone().unwrap_or_default();
    let mut available_targets = idf_im_lib::idf_versions::get_avalible_targets()
        .await
        .unwrap();
    // available_targets.insert(0, "all".to_string());
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

#[tauri::command]
fn set_targets(app_handle: AppHandle, targets: Vec<String>) -> Result<(), String> {
    info!("Setting targets: {:?}", targets);
    update_settings(&app_handle, |settings| {
        settings.target = Some(targets);
    })?;
    send_message(
        &app_handle,
        "Targets updated successfully".to_string(),
        "info".to_string(),
    );
    Ok(())
}

#[tauri::command]
async fn get_idf_versions(app_handle: AppHandle) -> Vec<Value> {
    let app_state = app_handle.state::<AppState>();
    // Clone the settings to avoid holding the MutexGuard across await points
    let settings = {
        let guard = app_state
            .settings
            .lock()
            .map_err(|_| {
                send_message(
                    &app_handle,
                    "Failed to obtain lock on AppState. Please retry the last action later."
                        .to_string(),
                    "error".to_string(),
                )
            })
            .expect("Failed to lock settings");
        guard.clone()
    };
    let targets = settings.target.clone().unwrap_or_default();
    let versions = settings.idf_versions.clone().unwrap_or_default();

    let targets_vec: Vec<String> = targets.iter().cloned().collect();
    let mut available_versions = if targets_vec.contains(&"all".to_string()) {
        idf_im_lib::idf_versions::get_idf_names().await
    } else {
        // todo: handle multiple targets
        idf_im_lib::idf_versions::get_idf_name_by_target(&targets[0].to_string().to_lowercase())
            .await
    };
    available_versions.push("master".to_string());
    available_versions
        .into_iter()
        .map(|v| {
            json!({
              "name": v,
              "selected": versions.contains(&v),
            })
        })
        .collect()
}

#[tauri::command]
fn set_versions(app_handle: AppHandle, versions: Vec<String>) -> Result<(), String> {
    info!("Setting IDF versions: {:?}", versions);
    update_settings(&app_handle, |settings| {
        settings.idf_versions = Some(versions);
    })?;

    send_message(
        &app_handle,
        "IDF versions updated successfully".to_string(),
        "info".to_string(),
    );
    Ok(())
}

#[tauri::command]
fn get_idf_mirror_list(app_handle: AppHandle) -> Value {
    let app_state = app_handle.state::<AppState>();
    // Clone the settings to avoid holding the MutexGuard across await points
    let settings = {
        let guard = app_state
            .settings
            .lock()
            .map_err(|_| {
                send_message(
                    &app_handle,
                    "Failed to obtain lock on AppState. Please retry the last action later."
                        .to_string(),
                    "error".to_string(),
                )
            })
            .expect("Failed to lock settings");
        guard.clone()
    };
    let mirror = settings.idf_mirror.clone().unwrap_or_default();
    let mut avalible_mirrors = idf_im_lib::get_idf_mirrors_list().to_vec();
    if !avalible_mirrors.contains(&mirror.as_str()) {
        let mut new_mirrors = vec![mirror.as_str()];
        new_mirrors.extend(avalible_mirrors);
        avalible_mirrors = new_mirrors;
    }
    json!({
      "mirrors":avalible_mirrors,
      "selected": mirror,
    })
}

#[tauri::command]
fn set_idf_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
    info!("Setting IDF mirror: {}", mirror);
    update_settings(&app_handle, |settings| {
        settings.idf_mirror = Some(mirror);
    })?;

    send_message(
        &app_handle,
        "IDF mirror updated successfully".to_string(),
        "info".to_string(),
    );
    Ok(())
}

#[tauri::command]
fn get_tools_mirror_list(app_handle: AppHandle) -> Value {
    let app_state = app_handle.state::<AppState>();
    // Clone the settings to avoid holding the MutexGuard across await points
    let settings = {
        let guard = app_state
            .settings
            .lock()
            .map_err(|_| {
                send_message(
                    &app_handle,
                    "Failed to obtain lock on AppState. Please retry the last action later."
                        .to_string(),
                    "error".to_string(),
                )
            })
            .expect("Failed to lock settings");
        guard.clone()
    };
    let mirror = settings.mirror.clone().unwrap_or_default();
    let mut avalible_mirrors = idf_im_lib::get_idf_tools_mirrors_list().to_vec();
    if !avalible_mirrors.contains(&mirror.as_str()) {
        let mut new_mirrors = vec![mirror.as_str()];
        new_mirrors.extend(avalible_mirrors);
        avalible_mirrors = new_mirrors;
    }
    json!({
      "mirrors":avalible_mirrors,
      "selected": mirror,
    })
}

#[tauri::command]
fn set_tools_mirror(app_handle: AppHandle, mirror: String) -> Result<(), String> {
    info!("Setting tools mirror: {}", mirror);
    update_settings(&app_handle, |settings| {
        settings.mirror = Some(mirror);
    })?;

    send_message(
        &app_handle,
        "Tools mirror updated successfully".to_string(),
        "info".to_string(),
    );
    Ok(())
}

#[tauri::command]
fn get_installation_path(app_handle: AppHandle) -> String {
    let app_state = app_handle.state::<AppState>();
    // Clone the settings to avoid holding the MutexGuard across await points
    let settings = {
        let guard = app_state
            .settings
            .lock()
            .map_err(|_| {
                send_message(
                    &app_handle,
                    "Failed to obtain lock on AppState. Please retry the last action later."
                        .to_string(),
                    "error".to_string(),
                )
            })
            .expect("Failed to lock settings");
        guard.clone()
    };
    let path = settings.path.clone().unwrap_or_default();
    path.to_str().unwrap().to_string()
}

#[tauri::command]
fn set_installation_path(app_handle: AppHandle, path: String) -> Result<(), String> {
    info!("Setting installation path: {}", path);
    update_settings(&app_handle, |settings| {
        settings.path = Some(PathBuf::from(path));
    })?;

    send_message(
        &app_handle,
        "Installation path updated successfully".to_string(),
        "info".to_string(),
    );
    Ok(())
}
#[tauri::command]
async fn is_path_empty_or_nonexistent(app_handle: AppHandle, path: String) -> bool {
    let path = Path::new(&path);

    // If path doesn't exist, return true
    if !path.exists() {
        return true;
    }

    // If path exists, check if it's a directory and if it's empty
    if path.is_dir() {
        match fs::read_dir(path) {
            Ok(mut entries) => {
                if entries.next().is_none() {
                    //it's empty
                    return true;
                }
                let settings = get_locked_settings(&app_handle).unwrap();

                let vers = match &settings.idf_versions {
                    Some(v) => v,
                    None => {
                        send_message(
                            &app_handle,
                            "No IDF versions selected. Please select at least one version to continue."
                                .to_string(),
                            "error".to_string(),
                        );
                        return false; // something is broken we don't have any versions selected
                    }
                };
                for v in vers {
                    let new_path = path.join(v);
                    if new_path.exists() {
                        return false;
                    }
                }
                return true;
            } // true if directory is empty
            Err(_) => false, // return false if we can't read the directory
        }
    } else {
        //path is file which is conflicting with the directory
        false // return false if it's not a directory
    }
}

#[tauri::command]
fn load_settings(app_handle: AppHandle, path: &str) {
    let mut file = File::open(path)
        .map_err(|_| {
            send_message(
                &app_handle,
                format!("Failed to open file: {}", path),
                "warning".to_string(),
            )
        })
        .expect("Failed to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|_| {
            send_message(
                &app_handle,
                format!("Failed to read file: {}", path),
                "warning".to_string(),
            )
        })
        .expect("Failed to read file");
    let settings: idf_im_lib::settings::Settings = toml::from_str(&contents)
        .map_err(|_| {
            send_message(
                &app_handle,
                format!("Failed to parse TOML: {}", path),
                "warning".to_string(),
            )
        })
        .expect("Failed to parse TOML");
    log::debug!("settings {:?}", settings);
    let app_state = app_handle.state::<AppState>();
    let mut state_settings = app_state
        .settings
        .lock()
        .map_err(|_| {
            send_message(
                &app_handle,
                "Failed to obtain lock on AppState. Please retry the last action later."
                    .to_string(),
                "error".to_string(),
            )
        })
        .expect("Failed to lock settings");
    *state_settings = settings;

    send_message(
        &app_handle,
        format!("Settings loaded from {}", path),
        "info".to_string(),
    );
}

fn prepare_installation_directories(
    app_handle: AppHandle,
    settings: &Settings,
    version: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut version_path = expand_tilde(settings.path.as_ref().unwrap().as_path());
    version_path.push(version);

    ensure_path(version_path.to_str().unwrap())?;
    send_message(
        &app_handle,
        format!(
            "IDF installation folder created at: {}",
            version_path.display()
        ),
        "info".to_string(),
    );

    Ok(version_path)
}

fn spawn_progress_monitor(
    app_handle: AppHandle,
    version: String,
    rx: mpsc::Receiver<ProgressMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let progress = ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", version));

        while let Ok(message) = rx.recv() {
            match message {
                ProgressMessage::Finish => {
                    progress.update(100, None);
                    progress.finish();
                    break;
                }
                ProgressMessage::Update(value) => {
                    progress.update(value, Some(&format!("Downloading IDF {}...", version)));
                }
            }
        }
    })
}

async fn download_idf(
    app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    idf_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    let progress = ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", version));

    let handle = spawn_progress_monitor(app_handle.clone(), version.to_string(), rx);

    log::info!("Starting IDF download for version: {}", version);
    log::info!("Installation path: {}", idf_path.display());

    match idf_im_lib::get_esp_idf_by_version_and_mirror(
        idf_path.to_str().unwrap(),
        version,
        settings.idf_mirror.as_deref(),
        tx,
        settings.recurse_submodules.unwrap_or_default(),
    ) {
        Ok(_) => {
            send_message(
                app_handle,
                format!(
                    "IDF {} installed successfully at: {}",
                    version,
                    idf_path.display()
                ),
                "info".to_string(),
            );
            progress.finish();
        }
        Err(e) => {
            send_message(
                app_handle,
                format!("Failed to install IDF {}. Reason: {}", version, e),
                "error".to_string(),
            );
            progress.finish();
            return Err(e.into());
        }
    }

    match handle.join() {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Progress monitor panicked: {:?}", e);
            send_message(
                app_handle,
                format!("Progress monitor panicked: {:?}", e),
                "error".to_string(),
            );
            Err("Progress monitor panicked".into())
        }
    }
}

// Tool installation types
#[derive(Debug)]
struct ToolSetup {
    download_dir: String,
    install_dir: String,
    tools_json_path: String,
}

impl ToolSetup {
    fn new(settings: &Settings, version_path: &PathBuf) -> Result<Self, String> {
        let p = version_path;
        let tools_json_path = p
            .join("esp-idf")
            .join(settings.tools_json_file.clone().unwrap_or_default());
        let download_dir = p.join(
            settings
                .tool_download_folder_name
                .clone()
                .unwrap_or_default(),
        );
        let install_dir = p.join(
            settings
                .tool_install_folder_name
                .clone()
                .unwrap_or_default(),
        );
        Ok(Self {
            download_dir: download_dir.to_str().unwrap().to_string(),
            install_dir: install_dir.to_str().unwrap().to_string(),
            tools_json_path: tools_json_path.to_str().unwrap().to_string(),
        })
    }

    fn create_directories(&self, app_handle: &AppHandle) -> Result<(), String> {
        // Create download directory
        ensure_path(&self.download_dir).map_err(|e| {
            send_message(
                app_handle,
                format!("Failed to create download directory: {}", e),
                "error".to_string(),
            );
            e.to_string()
        })?;

        // Create installation directory
        ensure_path(&self.install_dir).map_err(|e| {
            send_message(
                app_handle,
                format!("Failed to create installation directory: {}", e),
                "error".to_string(),
            );
            e.to_string()
        })?;

        // Add installation directory to PATH
        add_path_to_path(&self.install_dir);

        Ok(())
    }

    fn validate_tools_json(&self) -> Result<(), String> {
        if fs::metadata(&self.tools_json_path).is_err() {
            return Err(format!(
                "tools.json file not found at: {}",
                self.tools_json_path
            ));
        }
        Ok(())
    }
}

async fn setup_tools(
    app_handle: &AppHandle,
    settings: &Settings,
    idf_path: &PathBuf,
) -> Result<Vec<String>> {
    info!("Setting up tools...");

    let version_path = idf_path
        .parent()
        .context("Failed to get parent directory of IDF path")?;

    // Initialize tool setup
    let tool_setup = ToolSetup::new(settings, &PathBuf::from(version_path))
        .map_err(|e| anyhow!("Failed to initialize tool setup: {}", e))?;

    // Create necessary directories
    tool_setup
        .create_directories(app_handle)
        .map_err(|e| anyhow!("Failed to create tool directories: {}", e))?;

    // Validate tools.json exists
    tool_setup
        .validate_tools_json()
        .map_err(|e| anyhow!("Failed to validate tools.json: {}", e))?;

    // Parse tools.json and get list of tools to download
    let tools = idf_im_lib::idf_tools::read_and_parse_tools_file(&tool_setup.tools_json_path)
        .map_err(|e| {
            send_message(
                app_handle,
                format!("Failed to parse tools.json: {}", e),
                "error".to_string(),
            );
            anyhow!("Failed to parse tools.json: {}", e)
        })?;

    let tools_to_download = idf_im_lib::idf_tools::get_list_of_tools_to_download(
        tools.clone(),
        settings.target.clone().unwrap_or_default(),
        settings.mirror.as_deref(),
    );

    for (tool_name, download) in tools_to_download {
        process_tool_download(&app_handle, &tool_setup, &tool_name, &download).await?;
    }

    let env_vrs =
        idf_im_lib::setup_environment_variables(&PathBuf::from(&tool_setup.install_dir), idf_path)
            .map_err(|e| {
                send_message(
                    app_handle,
                    format!("Failed to setup environment variables: {}", e),
                    "error".to_string(),
                );
                anyhow!("Failed to setup environment variables: {}", e)
            })?;

    // get_and_validate_idf_tools_path

    let mut idf_tools_path = idf_path.clone();
    idf_tools_path.push(settings.idf_tools_path.clone().unwrap_or_default());
    if fs::metadata(&idf_tools_path).is_err() {
        // TODO: let the user navigate to find the file manually
        error!("IDF tools path does not exist");
        return Err(anyhow!("Failed to setup environment variables:"));
    }

    // run_idf_tools_py TODO: replace the python call

    run_idf_tools_py(idf_tools_path.to_str().unwrap(), &env_vrs).map_err(|e| {
        send_message(
            app_handle,
            format!("Failed to run IDF tools setup: {}", e),
            "error".to_string(),
        );
        anyhow!("Failed to run IDF tools setup: {}", e)
    })?;

    send_message(
        app_handle,
        "IDF tools setup completed successfully".to_string(),
        "info".to_string(),
    );

    let export_paths: Vec<String> = get_tools_export_paths(
        tools,
        settings.target.clone().unwrap(),
        PathBuf::from(tool_setup.install_dir)
            .join("tools")
            .to_str()
            .unwrap(),
    )
    .into_iter()
    .map(|p| {
        if std::env::consts::OS == "windows" {
            idf_im_lib::replace_unescaped_spaces_win(&p)
        } else {
            p
        }
    })
    .collect();

    send_message(
        app_handle,
        "Tools setup completed successfully".to_string(),
        "info".to_string(),
    );

    Ok(export_paths)
}

async fn process_tool_download(
    app_handle: &AppHandle,
    tool_setup: &ToolSetup,
    tool_name: &str,
    download: &idf_im_lib::idf_tools::Download,
) -> Result<()> {
    let (progress_tx, progress_rx) = mpsc::channel();
    let progress = ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", tool_name));

    let filename = Path::new(&download.url)
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| anyhow!("Invalid download URL"))?;

    let full_path = Path::new(&tool_setup.download_dir).join(filename);
    let full_path_str = match full_path.to_str() {
        Some(s) => s,
        None => return Err(anyhow!("Invalid file path")),
    };

    send_tools_message(app_handle, filename.to_string(), "start".to_string());

    // Verify existing file checksum
    if let Ok(true) = verify_file_checksum(&download.sha256, full_path_str) {
        info!("Checksum verified for existing file: {}", full_path_str);
        send_tools_message(app_handle, filename.to_string(), "match".to_string());
        return Ok(());
    }

    // Setup progress monitoring
    let progress_handle = setup_progress_monitoring(
        app_handle.clone(),
        progress_rx,
        progress,
        tool_name.to_string(),
    );

    // Download file
    info!("Downloading {} to: {}", tool_name, full_path_str);
    match download_file(&download.url, &tool_setup.download_dir, progress_tx).await {
        Ok(_) => {
            send_tools_message(app_handle, filename.to_string(), "downloaded".to_string());
            send_message(
                app_handle,
                format!("Tool {} downloaded successfully", tool_name),
                "info".to_string(),
            );
        }
        Err(e) => return Err(anyhow!("Download failed: {}", e)),
    };

    // Verify downloaded file
    verify_download(&app_handle, &download.sha256, full_path_str, filename)?;

    // Extract tool
    extract_tool(
        &app_handle,
        filename,
        full_path_str,
        &Path::new(&tool_setup.install_dir),
    )?;

    progress_handle
        .join()
        .map_err(|_| anyhow!("Progress monitoring thread panicked"))?;

    Ok(())
}

fn verify_download(
    app_handle: &AppHandle,
    sha256: &str,
    full_path_str: &str,
    filename: &str,
) -> Result<()> {
    match verify_file_checksum(sha256, full_path_str) {
        Ok(true) => {
            info!(
                "Checksum verified for newly downloaded file: {}",
                full_path_str
            );
            send_tools_message(
                app_handle,
                filename.to_string(),
                "download_verified".to_string(),
            );
            Ok(())
        }
        _ => {
            debug!(
                "Checksum verification of downloaded file failed: {}",
                full_path_str
            );
            send_tools_message(
                app_handle,
                filename.to_string(),
                "download_verification_failed".to_string(),
            );
            Err(anyhow!("Checksum verification failed"))
        }
    }
}

fn extract_tool(
    app_handle: &AppHandle,
    tool_name: &str,
    full_path_str: &str,
    install_dir: &Path,
) -> Result<()> {
    match idf_im_lib::decompress_archive(full_path_str, install_dir.to_str().unwrap()) {
        Ok(_) => {
            send_tools_message(app_handle, tool_name.to_string(), "extracted".to_string());
            send_message(
                app_handle,
                format!("Tool {} extracted successfully", tool_name),
                "info".to_string(),
            );
        }
        Err(e) => {
            send_tools_message(app_handle, tool_name.to_string(), "error".to_string());
            send_message(
                app_handle,
                format!("Failed to extract tool {}: {}", tool_name, e),
                "error".to_string(),
            );
            return Err(e.into());
        }
    }
    Ok(())
}

fn setup_progress_monitoring(
    app_handle: AppHandle,
    progress_rx: mpsc::Receiver<DownloadProgress>,
    progress: ProgressBar,
    tool_name: String,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(progress_msg) = progress_rx.recv() {
            match progress_msg {
                DownloadProgress::Progress(current, total) => {
                    let percentage = (current * 100 / total) as u64;
                    progress.update(
                        percentage,
                        Some(&format!("Downloading {}... {}%", tool_name, percentage)),
                    );
                }
                DownloadProgress::Complete => {
                    progress.finish();
                    break;
                }
                DownloadProgress::Error(err) => {
                    send_message(
                        &app_handle,
                        format!("Error downloading {}: {}", tool_name, err),
                        "error".to_string(),
                    );
                    break;
                }
            }
        }
    })
}

async fn install_single_version(
    app_handle: AppHandle,
    settings: &Settings,
    version: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Installing IDF version: {}", version);

    let version_path = prepare_installation_directories(app_handle.clone(), settings, &version)?;
    let idf_path = version_path.clone().join("esp-idf");
    download_idf(&app_handle, settings, &version, &idf_path).await?;
    let export_vars = setup_tools(&app_handle, settings, &idf_path).await?;
    let tools_install_path = version_path.clone().join(
        settings
            .tool_install_folder_name
            .clone()
            .unwrap_or_default(),
    );
    idf_im_lib::single_version_post_install(
        &version_path.to_str().unwrap(),
        idf_path.to_str().unwrap(),
        &version,
        tools_install_path.to_str().unwrap(),
        export_vars,
    );
    info!("single version installation completed");

    Ok(())
}

// #[tauri::command]
// async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
//     info!("Starting installation");
//     let settings = get_locked_settings(&app_handle)?;

//     if let Some(versions) = &settings.idf_versions {
//         for version in versions {
//             send_install_progress_message(&app_handle, version.clone(), "started".to_string());
//             if let Err(e) =
//                 install_single_version(app_handle.clone(), &settings, version.clone()).await
//             {
//                 send_install_progress_message(&app_handle, version.clone(), "failed".to_string());

//                 error!("Failed to install version {}: {}", version, e);
//                 return Err(format!("Installation failed: {}", e));
//             } else {
//                 send_install_progress_message(&app_handle, version.clone(), "finished".to_string());
//             }
//         }
//     } else {
//         send_message(
//             &app_handle,
//             "No IDF versions were selected".to_string(),
//             "warning".to_string(),
//         );
//         return Err("No IDF versions were selected".to_string());
//     }
//     let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
//     let _ = ensure_path(&ide_json_path);
//     let filepath = PathBuf::from(ide_json_path).join("esp_ide.json");
//     match settings.save_esp_ide_json(filepath.to_str().unwrap()) {
//         Ok(_) => {
//             info!("IDE JSON file saved to: {}", filepath.to_str().unwrap());
//             send_message(
//                 &app_handle,
//                 format!("IDE JSON file saved to: {}", filepath.to_str().unwrap()),
//                 "info".to_string(),
//             );
//         }
//         Err(e) => {
//             error!("Failed to save IDE JSON file: {}", e);
//             send_message(
//                 &app_handle,
//                 format!("Failed to save IDE JSON file: {}", e),
//                 "error".to_string(),
//             );
//         }
//     }

//     Ok(())
// }
#[tauri::command]
async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    // Set installation flag and queue up versions
    {
        let mut is_installing = app_state
            .is_installing
            .lock()
            .map_err(|_| "Lock error".to_string())?;
        if *is_installing {
            return Err("Installation already in progress".to_string());
        }
        *is_installing = true;

        let settings = get_locked_settings(&app_handle)?;
        if let Some(versions) = &settings.idf_versions {
            let mut queue = app_state
                .installation_queue
                .lock()
                .map_err(|_| "Lock error".to_string())?;
            *queue = versions.clone();
        } else {
            return Err("No IDF versions were selected".to_string());
        }
    }

    // Spawn a background task to process installations
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        process_installation_queue(app_handle_clone).await;
    });

    Ok(())
}

async fn process_installation_queue(app_handle: AppHandle) {
    let app_state = app_handle.state::<AppState>();
    let settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(_) => {
            send_message(
                &app_handle,
                "Failed to get settings".to_string(),
                "error".to_string(),
            );
            return;
        }
    };

    loop {
        // Get the next version to install
        let version = {
            let mut queue = app_state.installation_queue.lock().unwrap();
            if queue.is_empty() {
                break;
            }
            queue.remove(0)
        };

        // Start installation for this version
        send_install_progress_message(&app_handle, version.clone(), "started".to_string());

        match install_single_version(app_handle.clone(), &settings, version.clone()).await {
            Ok(_) => {
                send_install_progress_message(&app_handle, version, "finished".to_string());
            }
            Err(e) => {
                send_install_progress_message(&app_handle, version.clone(), "failed".to_string());
                error!("Failed to install version {}: {}", version, e);
                send_message(
                    &app_handle,
                    format!("Failed to install version {}: {}", version, e),
                    "error".to_string(),
                );
            }
        }

        // Allow other tasks to run
        tokio::task::yield_now().await;
    }

    // Complete installation process
    let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
    let _ = ensure_path(&ide_json_path);
    let filepath = PathBuf::from(ide_json_path).join("esp_ide.json");
    match settings.save_esp_ide_json(filepath.to_str().unwrap()) {
        Ok(_) => {
            info!("IDE JSON file saved to: {}", filepath.to_str().unwrap());
            send_message(
                &app_handle,
                format!("IDE JSON file saved to: {}", filepath.to_str().unwrap()),
                "info".to_string(),
            );
        }
        Err(e) => {
            error!("Failed to save IDE JSON file: {}", e);
            send_message(
                &app_handle,
                format!("Failed to save IDE JSON file: {}", e),
                "error".to_string(),
            );
        }
    }

    // Reset installation flag
    let mut is_installing = app_state.is_installing.lock().unwrap();
    *is_installing = false;
}

#[tauri::command]
async fn quit_app(app_handle: tauri::AppHandle) {
    info!("Attempting to quit app - pre-exit log");

    #[cfg(target_os = "windows")]
    {
        // Log before attempting to exit
        if let Some(log_dir) = idf_im_lib::get_log_directory() {
            use std::io::Write;
            let log_path = log_dir.join("windows_exit_attempt.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = writeln!(
                    file,
                    "Windows exit triggered at {:?}",
                    std::time::SystemTime::now()
                );
            }
        }

        let app_handle_clone = app_handle.clone();
        app_handle.exit(0);

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            info!("Forcing Windows process exit");
            let _ = app_handle_clone.emit("app_exiting", ());
            std::process::exit(0);
        });

        return;
    }

    app_handle.exit(0);
}

#[tauri::command]
fn save_config(app_handle: tauri::AppHandle, path: String) {
    let mut settings = match get_locked_settings(&app_handle) {
        Ok(s) => s,
        Err(_) => {
            return send_message(
                &app_handle,
                "Instalation config can not be saved. Please try again later.".to_string(),
                "error".to_string(),
            )
        }
    };

    settings.config_file_save_path = Some(PathBuf::from(path));
    let _ = settings.save();
}

#[tauri::command]
async fn start_simple_setup(app_handle: tauri::AppHandle) {
    let mut settings = get_locked_settings(&app_handle).unwrap();
    let state_settings = app_handle.state::<AppState>();
    send_simple_setup_message(&app_handle, 1, "started".to_string());
    // prerequisities check
    let mut prerequisities = check_prequisites(app_handle.clone());
    let os = get_operating_system().to_lowercase();
    if prerequisities.len() > 0 && os == "windows" {
        send_simple_setup_message(&app_handle, 2, "installing prerequisites".to_string());
        prerequisities = check_prequisites(app_handle.clone());
        if !install_prerequisites(app_handle.clone()) {
            send_simple_setup_message(&app_handle, 3, prerequisities.join(", "));
            return;
        }
        prerequisities = check_prequisites(app_handle.clone());
    }
    if prerequisities.len() > 0 {
        send_simple_setup_message(&app_handle, 4, prerequisities.join(", "));
        return;
    }
    // python check
    let mut python_found = python_sanity_check(app_handle.clone(), None);
    if !python_found && os == "windows" {
        send_simple_setup_message(&app_handle, 5, "Installing Python".to_string());
        if !python_install(app_handle.clone()) {
            send_simple_setup_message(&app_handle, 6, "Failed to install Python".to_string());
            return;
        }
    }
    python_found = python_sanity_check(app_handle.clone(), None);
    if !python_found {
        send_simple_setup_message(
            &app_handle,
            7,
            "Python not found. Please install it manually".to_string(),
        );
        return;
    }
    // version check get_idf_versions
    if settings.idf_versions.is_none() {
        send_simple_setup_message(&app_handle, 8, "Getting IDF versions".to_string());
        let versions = get_idf_versions(app_handle.clone()).await;
        let version = versions[0]["name"]
            .clone()
            .to_string()
            .trim_matches('"')
            .to_string();
        if set_versions(app_handle.clone(), vec![version]).is_err() {
            send_simple_setup_message(&app_handle, 9, "Failed to set IDF versions".to_string());
            return;
        }
    }
    settings = get_locked_settings(&app_handle).unwrap();
    // install
    info!("Starting installation");
    send_simple_setup_message(&app_handle, 10, "Installing IDF".to_string());
    let res = start_installation(app_handle.clone()).await;
    match res {
        Ok(_) => {
            info!("Installation completed");
            send_simple_setup_message(&app_handle, 11, "Installation completed".to_string());
        }
        Err(e) => {
            send_simple_setup_message(&app_handle, 12, "Failed to install IDF".to_string());
            error!("Failed to install IDF: {}", e);
        }
    }
}

#[tauri::command]
async fn show_in_folder(app_handle: tauri::AppHandle, path: String) {
    info!("Opening folder: {}", path);
    send_message(
        &app_handle,
        format!("Opening folder: {}", path),
        "info".to_string(),
    );
    #[cfg(target_os = "windows")]
    {
        match Command::new("explorer")
            .args(["/select,", &path]) // The comma after select is not a typo
            .spawn()
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to open folder with explorer: {}", e);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let path = if path.contains(",") {
            // see https://gitlab.freedesktop.org/dbus/dbus/-/issues/76
            match metadata(&path).unwrap().is_dir() {
                true => path,
                false => {
                    let mut path2 = PathBuf::from(path);
                    path2.pop();
                    path2.into_os_string().into_string().unwrap()
                }
            }
        } else {
            path
        };

        // Try using xdg-open first
        if Command::new("xdg-open").arg(&path).spawn().is_err() {
            // Fallback to dbus-send if xdg-open fails
            let uri = format!("file://{}", path);
            match Command::new("dbus-send")
                .args([
                    "--session",
                    "--dest=org.freedesktop.FileManager1",
                    "--type=method_call",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                    format!("array:string:\"{}\"", uri).as_str(),
                    "string:\"\"",
                ])
                .spawn()
            {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to open file with dbus-send: {}", e);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        match Command::new("open").args(["-R", &path]).spawn() {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to open file with open: {}", e);
            }
        }
    }
}

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_dir = match idf_im_lib::get_log_directory() {
        Some(dir) => dir,
        None => {
            eprintln!("Failed to get log directory, using current directory");
            PathBuf::from(".")
        }
    };

    let log_path = log_dir.join("eim_early_gui.log");
    let log_result = || -> std::io::Result<()> {
        use std::io::Write;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        writeln!(file, "Starting GUI at ??")?;

        Ok(())
    }();

    if let Err(e) = log_result {
        eprintln!("Failed to log startup: {}", e);
    }

    // Mac-specific PATH settings
    #[cfg(target_os = "macos")]
    {
        env::set_var("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/local/bin:/opt/local/sbin");
    }

    // Run the Tauri app with proper error handling
    if let Err(e) = std::panic::catch_unwind(|| {
        run_tauri_app();
    }) {
        error!("Critical Windows GUI error: {:?}", e);
        // Attempt to log to file before exiting
        if let Some(log_dir) = idf_im_lib::get_log_directory() {
            let crash_log = log_dir.join("windows_gui_crash.log");
            let _ = std::fs::write(crash_log, format!("Critical error: {:?}", e));
        }
    }
}

fn run_tauri_app() {
    let log_dir = idf_im_lib::get_log_directory().unwrap_or_else(|| {
        error!("Failed to get log directory.");
        PathBuf::from(".")
    });

    let result = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: log_dir,
                        file_name: Some("eim_gui_log".to_string()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .level_for("idf_im_lib", log::LevelFilter::Info)
                .level_for("eim_lib", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_state = AppState::default();
            app.manage(app_state);

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_settings,
            check_prequisites,
            install_prerequisites,
            get_prequisites,
            get_operating_system,
            python_sanity_check,
            python_install,
            get_available_targets,
            set_targets,
            get_idf_versions,
            set_versions,
            get_idf_mirror_list,
            set_idf_mirror,
            get_tools_mirror_list,
            set_tools_mirror,
            load_settings,
            get_installation_path,
            set_installation_path,
            start_installation,
            start_simple_setup,
            quit_app,
            save_config,
            get_logs_folder,
            show_in_folder,
            is_path_empty_or_nonexistent,
        ])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => {
            app.run(|app_handle, event| match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    info!("Exit requested");
                    app_handle.cleanup_before_exit();
                }
                _ => {
                    debug!("App event: {:?}", event);
                }
            });
        }
        Err(e) => {
            error!("Failed to build Tauri app: {}", e);
            eprintln!("Error building Tauri app: {}", e);
        }
    }
}
