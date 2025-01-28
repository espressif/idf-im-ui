use idf_im_lib::{
    self, add_path_to_path, download_file, ensure_path, expand_tilde,
    idf_tools::get_tools_export_paths, python_utils::run_idf_tools_py, settings::Settings,
    verify_file_checksum, DownloadProgress, ProgressMessage,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
    thread,
};
use tauri::{AppHandle, Manager};
use std::process::Command;
use std::fs::metadata;
#[cfg(target_os = "linux")]
use fork::{daemon, Fork}; // dep: fork = "0.1"



// Types and structs
#[derive(Default, Serialize, Deserialize)]
pub struct AppState {
    wizard_data: Mutex<WizardData>,
    settings: Mutex<Settings>,
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
}}


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

    handle.join().unwrap();
    Ok(())
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

struct ToolDownloader<'a> {
    app_handle: &'a AppHandle,
    download_dir: String,
}

impl<'a> ToolDownloader<'a> {
    fn new(app_handle: &'a AppHandle, download_dir: String) -> Self {
        Self {
            app_handle,
            download_dir,
        }
    }

    fn verify_existing_tool(&self, path: &str, expected_sha256: &str) -> Result<bool, String> {
        match verify_file_checksum(expected_sha256, path) {
            Ok(true) => {
                info!("Checksum verified for existing file: {}", path);
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }
}

async fn setup_tools(
    app_handle: &AppHandle,
    settings: &Settings,
    idf_path: &PathBuf,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    info!("Setting up tools...");

    let version_path = PathBuf::from(idf_path.parent().unwrap());

    // Initialize tool setup
    let tool_setup = ToolSetup::new(settings, &version_path)?;

    // Create necessary directories
    tool_setup.create_directories(app_handle)?;

    // Validate tools.json exists
    tool_setup.validate_tools_json()?;

    // Parse tools.json and get list of tools to download
    let tools = idf_im_lib::idf_tools::read_and_parse_tools_file(&tool_setup.tools_json_path)
        .map_err(|e| {
            send_message(
                app_handle,
                format!("Failed to parse tools.json: {}", e),
                "error".to_string(),
            );
            e
        })?;

    let tools_to_download = idf_im_lib::idf_tools::get_list_of_tools_to_download(
        tools.clone(),
        settings.target.clone().unwrap_or_default(),
        settings.mirror.as_deref(),
    );

    // TODO: send tools message showing and finishing progress alltogether -> like visual chekbox

    // download tools
    for (tool_name, download) in tools_to_download {
        let (progress_tx, progress_rx) = mpsc::channel();

        let progress =
            ProgressBar::new(app_handle.clone(), &format!("Installing IDF {}", tool_name));

        let filename = Path::new(&download.url)
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| "Invalid download URL".to_string())?;

        let full_path = Path::new(&tool_setup.download_dir).join(filename);
        let full_path_str = full_path.to_str().unwrap();

        send_tools_message(app_handle, filename.to_string(), "start".to_string());

        match verify_file_checksum(&download.sha256, full_path_str) {
            Ok(true) => {
                info!("Checksum verified for existing file: {}", full_path_str);
                send_tools_message(app_handle, filename.to_string(), "match".to_string());
            }
            _ => {
                debug!(
                    "Checksum verification failed or file does not exists: {}",
                    full_path_str
                );
            }
        }

        let progress_handle = {
            let tool_name = tool_name.clone();
            let app_handle = app_handle.clone();
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
        };
        info!("Downloading {} to: {}", tool_name, full_path_str);
        let download_result =
            download_file(&download.url, &tool_setup.download_dir, progress_tx).await;

        match download_result {
            Ok(_) => {
                send_tools_message(app_handle, filename.to_string(), "downloaded".to_string());

                send_message(
                    app_handle,
                    format!("Tool {} downloaded successfully", tool_name),
                    "info".to_string(),
                );
            }
            Err(e) => {
                send_tools_message(app_handle, filename.to_string(), "error".to_string());

                send_message(
                    app_handle,
                    format!("Failed to download {}: {}", tool_name, e),
                    "error".to_string(),
                );
                return Err(e.into());
            }
        }

        // extract tool:
        let out = idf_im_lib::decompress_archive(full_path_str, &tool_setup.install_dir);
        match out {
            Ok(_) => {
                send_tools_message(app_handle, filename.to_string(), "extracted".to_string());

                send_message(
                    app_handle,
                    format!("Tool {} extracted successfully", tool_name),
                    "info".to_string(),
                );
            }
            Err(e) => {
                send_tools_message(app_handle, filename.to_string(), "error".to_string());

                send_message(
                    app_handle,
                    format!("Failed to extract tool {}: {}", tool_name, e),
                    "error".to_string(),
                );
                return Err(e.into());
            }
        }

        progress_handle.join().unwrap();
    }

    let env_vrs = match idf_im_lib::setup_environment_variables(
        &PathBuf::from(&tool_setup.install_dir),
        idf_path,
    ) {
        Ok(vrs) => vrs,
        Err(e) => {
            send_message(
                app_handle,
                format!("Failed to setup environment variables: {}", e),
                "error".to_string(),
            );
            return Err(e.into());
        }
    };

    // get_and_validate_idf_tools_path

    let mut idf_tools_path = idf_path.clone();
    idf_tools_path.push(settings.idf_tools_path.clone().unwrap_or_default());
    if fs::metadata(&idf_tools_path).is_err() {
        // TODO: let the user navigate to find the file manually
        error!("IDF tools path does not exist");
        return Err("IDF tools path does not exist".into());
    }

    // run_idf_tools_py TODO: replace the python call

    match run_idf_tools_py(idf_tools_path.to_str().unwrap(), &env_vrs) {
        Ok(_) => {
            send_message(
                app_handle,
                "IDF tools setup completed successfully".to_string(),
                "info".to_string(),
            );
        }
        Err(e) => {
            send_message(
                app_handle,
                format!("Failed to run IDF tools setup: {}", e),
                "error".to_string(),
            );
            return Err(e.into());
        }
    }

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

    Ok(())
}

#[tauri::command]
async fn start_installation(app_handle: AppHandle) -> Result<(), String> {
    info!("Starting installation");
    let settings = get_locked_settings(&app_handle)?;

    if let Some(versions) = &settings.idf_versions {
        for version in versions {
            send_install_progress_message(&app_handle, version.clone(), "started".to_string());
            if let Err(e) =
                install_single_version(app_handle.clone(), &settings, version.clone()).await
            {
                send_install_progress_message(&app_handle, version.clone(), "failed".to_string());

                error!("Failed to install version {}: {}", version, e);
                return Err(format!("Installation failed: {}", e));
            } else {
                send_install_progress_message(&app_handle, version.clone(), "finished".to_string());
            }
        }
    } else {
        send_message(
            &app_handle,
            "No IDF versions were selected".to_string(),
            "warning".to_string(),
        );
    }
    let ide_json_path = settings.esp_idf_json_path.clone().unwrap_or_default();
    let _ = ensure_path(&ide_json_path);
    let filepath = PathBuf::from(ide_json_path).join("esp_ide.json");
    match settings.save_esp_ide_json(filepath.to_str().unwrap()) {
        Ok(_) => {
            send_message(
                &app_handle,
                format!("IDE JSON file saved to: {}", filepath.to_str().unwrap()),
                "info".to_string(),
            );
        }
        Err(e) => {
            send_message(
                &app_handle,
                format!("Failed to save IDE JSON file: {}", e),
                "error".to_string(),
            );
        }
    }

    Ok(())
}

#[tauri::command]
fn quit_app(app_handle: tauri::AppHandle) {
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
    send_simple_setup_message(&app_handle, 10, "Installing IDF".to_string());
    let res = start_installation(app_handle.clone()).await;
    match res {
        Ok(_) => {
            send_simple_setup_message(&app_handle, 11, "Installation completed".to_string());
        }
        Err(e) => {
            send_simple_setup_message(&app_handle, 12, "Failed to install IDF".to_string());
        }
    }
}

#[tauri::command]
fn show_in_folder(path: String) {
  #[cfg(target_os = "windows")]
  {
    match Command::new("explorer")
        .args(["/select,", &path]) // The comma after select is not a typo
        .spawn() {
          Ok(_) => {},
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
        if Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .is_err()
        {
            // Fallback to dbus-send if xdg-open fails
            let uri = format!("file://{}", path);
            match Command::new("dbus-send")
                .args(["--session", "--dest=org.freedesktop.FileManager1", "--type=method_call",
                      "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems",
                      format!("array:string:\"{}\"", uri).as_str(), "string:\"\""])
                .spawn() {
                    Ok(_) => {},
                    Err(e) => {
                        error!("Failed to open file with dbus-send: {}", e);
                    }
                }
        }
    }

  #[cfg(target_os = "macos")]
  {
    match Command::new("open")
        .args(["-R", &path])
        .spawn() {
            Ok(_) => {},
            Err(e) => {
                error!("Failed to open file with open: {}", e);
            }
        }
        
  }
}

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    // this actually can not keep pace with the console, so maybe we should disable it for production build
                    tauri_plugin_log::TargetKind::Webview,
                ))
                .level(log::LevelFilter::Error)
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
            show_in_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
