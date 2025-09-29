use std::{path::PathBuf, sync::mpsc, thread};
use anyhow::Result;
use tauri::AppHandle;

use idf_im_lib::{ProgressMessage};
use idf_im_lib::settings::Settings;

use crate::gui::ui::{
    emit_installation_event, emit_log_message, InstallationProgress, InstallationStage, MessageLevel, ProgressBar,
};
use crate::gui::commands::idf_tools::setup_tools;

pub fn spawn_progress_monitor(
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
                }
                ProgressMessage::Update(value) => {
                    progress.update(value, Some(&format!("Downloading IDF {}...", version)));
                }
                ProgressMessage::SubmoduleUpdate((name, value)) => {
                    progress.update(
                        value,
                        Some(&format!("Downloading submodule {}... {}%", name, value)),
                    );
                }
                ProgressMessage::SubmoduleFinish(_name) => {
                    progress.update(100, None);
                }
            }
        }
    })
}

pub async fn download_idf(
    app_handle: &AppHandle,
    settings: &Settings,
    version: &str,
    idf_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();

    emit_installation_event(
        app_handle,
        InstallationProgress {
            stage: InstallationStage::Download,
            percentage: 0,
            message: format!("Starting ESP-IDF {} download...", version),
            detail: Some("Preparing to clone repository".to_string()),
            version: Some(version.to_string()),
        },
    );

    let app_handle_clone = app_handle.clone();
    let version_clone = version.to_string();

    let handle = thread::spawn(move || {
        let mut last_percentage = 0;
        let mut submodule_count = 0;
        let mut completed_submodules = 0;
        let mut total_estimated_submodules = 35;
        let mut main_repo_finished = false;
        let mut has_submodules = true;

        loop {
            match rx.recv() {
                Ok(ProgressMessage::Update(value)) => {
                    if value != last_percentage && (value - last_percentage) >= 10 {
                        last_percentage = value;
                        emit_installation_event(
                            &app_handle_clone,
                            InstallationProgress {
                                stage: InstallationStage::Download,
                                percentage: (value * 10 / 100) as u32,
                                message: format!("Cloning ESP-IDF {} repository", version_clone),
                                detail: Some(format!("Repository: {}%", value)),
                                version: Some(version_clone.clone()),
                            },
                        );
                    }
                }
                Ok(ProgressMessage::SubmoduleUpdate((name, value))) => {
                    has_submodules = true;
                    if submodule_count > total_estimated_submodules {
                        total_estimated_submodules = submodule_count + 10;
                    }
                    let submodule_base_progress = 10;
                    let submodule_range = 55;
                    let current_submodule_progress = (completed_submodules as f32
                        / total_estimated_submodules as f32)
                        * submodule_range as f32;
                    let individual_progress = (value as f32 / 100.0)
                        * (submodule_range as f32 / total_estimated_submodules as f32);
                    let total_progress = submodule_base_progress
                        + current_submodule_progress as u32
                        + individual_progress as u32;

                    emit_installation_event(
                        &app_handle_clone,
                        InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: total_progress.min(65),
                            message: format!("Downloading submodule: {}", name),
                            detail: Some(format!(
                                "Submodule {}/{} (est.) - {}%",
                                completed_submodules + 1,
                                total_estimated_submodules,
                                value
                            )),
                            version: Some(version_clone.clone()),
                        },
                    );
                }
                Ok(ProgressMessage::SubmoduleFinish(name)) => {
                    has_submodules = true;
                    completed_submodules += 1;
                    submodule_count = completed_submodules;

                    let effective_total = total_estimated_submodules.max(completed_submodules);
                    let submodule_progress = 10 + (completed_submodules * 55 / effective_total);

                    emit_installation_event(
                        &app_handle_clone,
                        InstallationProgress {
                            stage: InstallationStage::Download,
                            percentage: submodule_progress.min(65) as u32,
                            message: format!(
                                "Completed submodule: {}",
                                name.split('/').last().unwrap_or(&name).replace("_", " ")
                            ),
                            detail: Some(format!(
                                "Progress: {}/{} submodules",
                                completed_submodules, total_estimated_submodules
                            )),
                            version: Some(version_clone.clone()),
                        },
                    );

                    emit_log_message(
                        &app_handle_clone,
                        MessageLevel::Info,
                        format!(
                            "Submodule '{}' completed ({}/{})",
                            name, completed_submodules, total_estimated_submodules
                        ),
                    );
                }
                Ok(ProgressMessage::Finish) => {
                    main_repo_finished = true;
                    if !has_submodules {
                        emit_installation_event(
                            &app_handle_clone,
                            InstallationProgress {
                                stage: InstallationStage::Extract,
                                percentage: 65,
                                message: "ESP-IDF download completed (no submodules)".to_string(),
                                detail: Some("Repository cloned successfully".to_string()),
                                version: Some(version_clone.clone()),
                            },
                        );
                        break;
                    } else if completed_submodules > 0 {
                        emit_installation_event(
                            &app_handle_clone,
                            InstallationProgress {
                                stage: InstallationStage::Extract,
                                percentage: 65,
                                message: "ESP-IDF download completed".to_string(),
                                detail: Some(format!(
                                    "Repository and {} submodules ready",
                                    completed_submodules
                                )),
                                version: Some(version_clone.clone()),
                            },
                        );
                        break;
                    } else {
                        emit_installation_event(
                            &app_handle_clone,
                            InstallationProgress {
                                stage: InstallationStage::Download,
                                percentage: 10,
                                message: "Main repository cloned, preparing submodules..."
                                    .to_string(),
                                detail: Some("Waiting for submodule updates".to_string()),
                                version: Some(version_clone.clone()),
                            },
                        );
                    }
                }
                Err(_) => {
                    if main_repo_finished {
                        let final_percentage = if has_submodules { 65 } else { 65 };
                        emit_installation_event(
                            &app_handle_clone,
                            InstallationProgress {
                                stage: InstallationStage::Extract,
                                percentage: final_percentage,
                                message: "ESP-IDF download completed".to_string(),
                                detail: Some(if has_submodules {
                                    format!(
                                        "Repository and {} submodules processed",
                                        completed_submodules
                                    )
                                } else {
                                    "Repository cloned successfully".to_string()
                                }),
                                version: Some(version_clone.clone()),
                            },
                        );
                    }
                    break;
                }
            }
        }
    });

    emit_log_message(
        app_handle,
        MessageLevel::Info,
        format!(
            "Cloning ESP-IDF {} repository from {}",
            version,
            settings.idf_mirror.as_deref().unwrap_or("default mirror")
        ),
    );

    let result = idf_im_lib::get_esp_idf(
        idf_path.to_str().unwrap(),
        settings.repo_stub.as_deref(),
        version,
        settings.idf_mirror.as_deref(),
        settings.recurse_submodules.unwrap_or_default(),
        tx,
    );

    handle.join().unwrap();

    match result {
        Ok(_) => {
            emit_installation_event(
                app_handle,
                InstallationProgress {
                    stage: InstallationStage::Extract,
                    percentage: 70,
                    message: format!("ESP-IDF {} ready for tools installation", version),
                    detail: Some(format!("Location: {}", idf_path.display())),
                    version: Some(version.to_string()),
                },
            );

            emit_log_message(
                app_handle,
                MessageLevel::Success,
                format!(
                    "ESP-IDF {} downloaded successfully: {}",
                    version,
                    idf_path.display()
                ),
            );
            Ok(())
        }
        Err(e) => {
            emit_installation_event(
                app_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: format!("Failed to download ESP-IDF {}", version),
                    detail: Some(e.to_string()),
                    version: Some(version.to_string()),
                },
            );
            Err(e.into())
        }
    }
}

pub async fn install_single_version(
    app_handle: AppHandle,
    settings: &Settings,
    version: String,
) -> Result<(), Box<dyn std::error::Error>> {
    use log::{info, error};
    use crate::gui::ui::send_message;

    info!("Installing IDF version: {}", version);

    let paths = settings.get_version_paths(&version).map_err(|err| {
        error!("Failed to get version paths: {}", err);
        err.to_string()
    })?;

    if paths.using_existing_idf {
        info!("Using existing IDF directory: {}", paths.idf_path.display());
        send_message(
            &app_handle,
            format!("Using existing IDF directory: {}", paths.idf_path.display()),
            "info".to_string(),
        );

        log::debug!("Using IDF version: {}", paths.actual_version);
    } else {
        download_idf(&app_handle, settings, &version, &paths.idf_path).await?;
    }

    let export_vars = setup_tools(
        &app_handle,
        settings,
        &paths.idf_path,
        &paths.actual_version,
    )
    .await?;

    idf_im_lib::single_version_post_install(
        &paths.activation_script_path.to_str().unwrap(),
        paths.idf_path.to_str().unwrap(),
        &paths.actual_version,
        paths.tool_install_directory.to_str().unwrap(),
        export_vars,
        paths.python_venv_path.to_str(),
        None,
    );

    Ok(())
}