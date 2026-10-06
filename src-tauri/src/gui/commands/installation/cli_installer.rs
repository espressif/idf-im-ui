//! Runs `eim install` as a subprocess and turns its log output into GUI events
//! (used by the Windows `start_installation`).

use std::path::{Path, PathBuf};

use idf_im_lib::settings::Settings;

use crate::gui::ui::{InstallationProgress, InstallationStage};

use super::InstallationPlan;

#[cfg(target_os = "windows")]
use std::{
    io::{BufRead, BufReader},
    process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio},
};

#[cfg(target_os = "windows")]
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use crate::gui::{
    app_state::set_installation_status,
    ui::{emit_installation_event, emit_log_message, MessageLevel},
};

#[cfg(target_os = "windows")]
use super::emit_installation_plan;

pub(super) fn subprocess_config_path(temp_dir: &Path, pid: u32) -> PathBuf {
    temp_dir.join(format!("eim_config_{}.toml", pid))
}

/// Settings handed to the subprocess through the temporary config file.
pub(super) fn subprocess_settings(settings: &Settings, config_path: &Path) -> Settings {
    let mut settings_clone = settings.clone();
    settings_clone.config_file_save_path = Some(config_path.to_path_buf());
    settings_clone.non_interactive = Some(true);
    settings_clone.install_all_prerequisites = Some(true);
    settings_clone
}

/// Log line as shown to the user, or `None` for DEBUG/TRACE lines.
pub(super) fn clean_log_line(line: &str) -> Option<String> {
    if line.contains("DEBUG") || line.contains("TRACE") {
        return None;
    }
    let clean_message = if line.contains(" - ") {
        let parts: Vec<&str> = line.splitn(2, " - ").collect();
        if parts.len() > 1 {
            parts[1].to_string()
        } else {
            line.to_string()
        }
    } else {
        line.to_string()
    };
    Some(clean_message)
}

fn bracketed(line: &str) -> Option<&str> {
    let start = line.find('[')?;
    let end = line.find(']')?;
    Some(&line[start + 1..end])
}

#[derive(Debug, Clone)]
pub(super) enum CliEvent {
    Plan(InstallationPlan),
    Progress(InstallationProgress),
}

/// Estimates installation progress from the subprocess' stdout, one line at a time.
pub(super) struct CliProgressTracker {
    versions: Vec<String>,
    percentage: u32,
    current_version: Option<String>,
    tools_started: bool,
    completed: u32,
    total: u32,
}

impl CliProgressTracker {
    pub(super) fn new(versions: Vec<String>) -> Self {
        Self {
            versions,
            percentage: 5,
            current_version: None,
            tools_started: false,
            completed: 0,
            total: 0,
        }
    }

    /// Events to emit (in order) for `line`.
    pub(super) fn parse_line(&mut self, line: &str) -> Vec<CliEvent> {
        if line.contains("Selected idf version:") {
            if let Some(version_str) = bracketed(line) {
                let version = version_str.replace("\"", "").trim().to_string();
                return self.on_version_selected(version);
            }
        }
        self.phase_progress(line)
            .map(CliEvent::Progress)
            .into_iter()
            .collect()
    }

    fn progress(
        &self,
        stage: InstallationStage,
        percentage: u32,
        message: String,
        detail: String,
    ) -> InstallationProgress {
        InstallationProgress {
            stage,
            percentage,
            message,
            detail: Some(detail),
            version: self.current_version.clone(),
        }
    }

    fn on_version_selected(&mut self, version: String) -> Vec<CliEvent> {
        self.current_version = Some(version.clone());
        let mut events = Vec::new();
        if let Some(version_index) = self.versions.iter().position(|v| v == &version) {
            events.push(CliEvent::Plan(InstallationPlan {
                total_versions: self.versions.len(),
                versions: self.versions.clone(),
                current_version_index: Some(version_index),
            }));
        }
        self.percentage = 10;
        events.push(CliEvent::Progress(
            self.progress(
                InstallationStage::Download,
                10,
                rust_i18n::t!(
                    "gui.installation.starting_version",
                    version = version.clone()
                )
                .to_string(),
                rust_i18n::t!("gui.installation.preparing_download").to_string(),
            ),
        ));
        events
    }

    fn set_percentage(&mut self, progress: InstallationProgress) -> Option<InstallationProgress> {
        self.percentage = progress.percentage;
        Some(progress)
    }

    fn phase_progress(&mut self, line: &str) -> Option<InstallationProgress> {
        if line.contains("Checking for prerequisites") {
            let p = self.progress(
                InstallationStage::Prerequisites,
                8,
                rust_i18n::t!("gui.installation.checking_prerequisites").to_string(),
                rust_i18n::t!("gui.installation.verifying_requirements").to_string(),
            );
            self.set_percentage(p)
        } else if line.contains("Python sanity check") {
            let p = self.progress(
                InstallationStage::Prerequisites,
                12,
                rust_i18n::t!("gui.installation.verifying_python").to_string(),
                rust_i18n::t!("gui.installation.checking_python").to_string(),
            );
            self.set_percentage(p)
        } else if line.contains("Cloning ESP-IDF") || line.contains("git clone") {
            let p = self.progress(
                InstallationStage::Download,
                15,
                rust_i18n::t!("gui.installation.downloading_repository").to_string(),
                rust_i18n::t!("gui.installation.cloning_main").to_string(),
            );
            self.set_percentage(p)
        } else if line.contains("Updating submodule") || line.contains("submodule update") {
            let p = self.submodule_progress();
            self.set_percentage(p)
        } else if line.contains("Downloading tools:") {
            self.on_tools_list(line)
        } else if line.contains("Downloading tool:") && self.tools_started {
            self.on_tool_download(line)
        } else if line.contains("extracted tool:") || line.contains("Decompression completed") {
            let p = self.on_tool_extracted();
            self.set_percentage(p)
        } else if line.contains("Python environment") || line.contains("Installing python") {
            let p = self.progress(
                InstallationStage::Python,
                90,
                rust_i18n::t!("gui.installation.python_environment").to_string(),
                rust_i18n::t!("gui.installation.configuring_python").to_string(),
            );
            self.set_percentage(p)
        } else if line.contains("Successfully installed IDF")
            || line.contains("Installation complete")
        {
            let p = self.progress(
                InstallationStage::Complete,
                100,
                rust_i18n::t!("gui.installation.completed_successfully").to_string(),
                rust_i18n::t!("gui.installation.finished").to_string(),
            );
            self.set_percentage(p)
        } else {
            None
        }
    }

    /// Submodules are the long part of the download (15-65%).
    fn submodule_progress(&self) -> InstallationProgress {
        self.progress(
            InstallationStage::Download,
            std::cmp::min(65, self.percentage + 2),
            rust_i18n::t!("gui.installation.downloading_submodules").to_string(),
            rust_i18n::t!("gui.installation.processing_submodules").to_string(),
        )
    }

    fn tools_percentage(&self) -> u32 {
        65 + (self.completed * 20 / self.total.max(1))
    }

    fn on_tools_list(&mut self, line: &str) -> Option<InstallationProgress> {
        let tools_str = bracketed(line)?;
        let tools: Vec<&str> = tools_str.split(',').collect();
        self.total = tools.len() as u32;
        let p = self.progress(
            InstallationStage::Tools,
            65,
            rust_i18n::t!("gui.installation.installing_tools", count = self.total).to_string(),
            rust_i18n::t!("gui.installation.preparing_tools").to_string(),
        );
        self.tools_started = true;
        self.set_percentage(p)
    }

    fn on_tool_download(&mut self, line: &str) -> Option<InstallationProgress> {
        let tool_start = line.find("tool:")?;
        let tool_name = line[tool_start + 5..].trim();
        let p = self.progress(
            InstallationStage::Tools,
            self.tools_percentage(),
            rust_i18n::t!("gui.installation.downloading_tool", name = tool_name).to_string(),
            rust_i18n::t!("gui.installation.tool_number", number = self.completed + 1).to_string(),
        );
        self.set_percentage(p)
    }

    fn on_tool_extracted(&mut self) -> InstallationProgress {
        self.completed += 1;
        self.progress(
            InstallationStage::Tools,
            self.tools_percentage().min(85),
            rust_i18n::t!("gui.installation.installed_tool", number = self.completed).to_string(),
            rust_i18n::t!("gui.installation.tool_completed").to_string(),
        )
    }
}

#[cfg(target_os = "windows")]
fn installer_spawn_error(app_handle: &AppHandle, e: std::io::Error) -> String {
    emit_installation_event(
        app_handle,
        InstallationProgress {
            stage: InstallationStage::Error,
            percentage: 0,
            message: rust_i18n::t!("gui.installation.installer_process_failed").to_string(),
            detail: Some(e.to_string()),
            version: None,
        },
    );
    format!("Failed to start installer: {}", e)
}

/// Starts the installer with piped stdout and stderr.
#[cfg(target_os = "windows")]
pub(super) fn spawn_cli_installer(
    app_handle: &AppHandle,
    current_exe: PathBuf,
    config_path: &Path,
) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new(current_exe)
        .arg("install")
        .arg("-n")
        .arg("true") // Non-interactive mode
        .arg("-a")
        .arg("true") // Install prerequisites
        .arg("-c")
        .arg(config_path) // Path to config file
        .stdout(Stdio::piped()) // Capture stdout
        .stderr(Stdio::piped()) // Capture stderr
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| installer_spawn_error(app_handle, e))
}

#[cfg(target_os = "windows")]
fn forward_stdout(handle: &AppHandle, stdout: ChildStdout, versions: Vec<String>) {
    let mut tracker = CliProgressTracker::new(versions);
    let stdout_reader = BufReader::new(stdout);
    for line in stdout_reader.lines() {
        if let Ok(line) = line {
            for event in tracker.parse_line(&line) {
                match event {
                    CliEvent::Plan(plan) => emit_installation_plan(handle, plan),
                    CliEvent::Progress(progress) => emit_installation_event(handle, progress),
                }
            }

            if let Some(clean_message) = clean_log_line(&line) {
                emit_log_message(handle, MessageLevel::Info, clean_message);
                log::info!("Install process stdout: {}", line);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn forward_stderr(handle: &AppHandle, stderr: ChildStderr) {
    let stderr_reader = BufReader::new(stderr);
    for line in stderr_reader.lines() {
        if let Ok(line) = line {
            emit_log_message(handle, MessageLevel::Error, line.clone());
            log::error!("Install process stderr: {}", line);
        }
    }
}

#[cfg(target_os = "windows")]
fn report_cli_result(
    monitor_handle: &AppHandle,
    status: ExitStatus,
    versions: &[String],
    current_version: Option<String>,
) {
    let success = status.success();
    log::info!("Installation completed with success={}", success);

    if success {
        emit_installation_event(
            monitor_handle,
            InstallationProgress {
                stage: InstallationStage::Complete,
                percentage: 100,
                message: rust_i18n::t!("gui.installation.all_completed").to_string(),
                detail: Some(
                    rust_i18n::t!(
                        "gui.installation.all_versions",
                        versions = versions.join(", ")
                    )
                    .to_string(),
                ),
                version: None,
            },
        );

        emit_log_message(
            monitor_handle,
            MessageLevel::Success,
            rust_i18n::t!("gui.installation.success_message").to_string(),
        );
    } else {
        let error_msg = rust_i18n::t!(
            "gui.installation.failed_exit_code",
            code = status.code().unwrap_or(-1)
        )
        .to_string();

        emit_installation_event(
            monitor_handle,
            InstallationProgress {
                stage: InstallationStage::Error,
                percentage: 0,
                message: rust_i18n::t!("gui.installation.process_failed_detail").to_string(),
                detail: Some(error_msg.clone()),
                version: current_version,
            },
        );

        emit_log_message(monitor_handle, MessageLevel::Error, error_msg);
    }
}

/// Forwards the installer's output to the frontend, waits for it to exit,
/// reports the result and removes the temporary config file.
#[cfg(target_os = "windows")]
pub(super) fn monitor_cli_installer(
    monitor_handle: AppHandle,
    mut child: Child,
    versions: Vec<String>,
    cfg_path: PathBuf,
) {
    let current_version: Option<String> = None;

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    let stdout_monitor = {
        let handle = monitor_handle.clone();
        let versions = versions.clone();
        std::thread::spawn(move || forward_stdout(&handle, stdout, versions))
    };

    let stderr_monitor = {
        let handle = monitor_handle.clone();
        std::thread::spawn(move || forward_stderr(&handle, stderr))
    };

    let status = match child.wait() {
        Ok(status) => {
            log::info!("Install process completed with status: {:?}", status);
            status
        }
        Err(e) => {
            log::error!("Failed to wait for install process: {}", e);

            emit_installation_event(
                &monitor_handle,
                InstallationProgress {
                    stage: InstallationStage::Error,
                    percentage: 0,
                    message: rust_i18n::t!("gui.installation.process_failed").to_string(),
                    detail: Some(e.to_string()),
                    version: current_version.clone(),
                },
            );

            std::thread::sleep(std::time::Duration::from_secs(2));
            return;
        }
    };

    let _ = stdout_monitor.join();
    let _ = stderr_monitor.join();

    if let Err(e) = set_installation_status(&monitor_handle, false) {
        log::error!("Failed to update installation status: {}", e);
    }

    report_cli_result(&monitor_handle, status, &versions, current_version);

    let _ = std::fs::remove_file(&cfg_path);

    log::info!("Installation monitor thread completed");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress_events(events: Vec<CliEvent>) -> Vec<InstallationProgress> {
        events
            .into_iter()
            .filter_map(|e| match e {
                CliEvent::Progress(p) => Some(p),
                CliEvent::Plan(_) => None,
            })
            .collect()
    }

    fn single_progress(tracker: &mut CliProgressTracker, line: &str) -> InstallationProgress {
        let mut events = progress_events(tracker.parse_line(line));
        assert_eq!(events.len(), 1, "expected one event for {line:?}");
        events.remove(0)
    }

    #[test]
    fn subprocess_config_path_uses_pid() {
        let dir = Path::new("/tmp/x");
        assert_eq!(
            subprocess_config_path(dir, 42),
            dir.join("eim_config_42.toml")
        );
    }

    #[test]
    fn subprocess_settings_forces_non_interactive_with_prerequisites() {
        let settings = Settings::default();
        let config = PathBuf::from("/tmp/eim_config_1.toml");
        let s = subprocess_settings(&settings, &config);
        assert_eq!(s.config_file_save_path, Some(config));
        assert_eq!(s.non_interactive, Some(true));
        assert_eq!(s.install_all_prerequisites, Some(true));
        assert_eq!(s.idf_versions, settings.idf_versions);
    }

    #[test]
    fn clean_log_line_skips_debug_and_trace() {
        assert_eq!(clean_log_line("2024 DEBUG - foo"), None);
        assert_eq!(clean_log_line("TRACE something"), None);
    }

    #[test]
    fn clean_log_line_strips_prefix_before_first_separator() {
        assert_eq!(
            clean_log_line("12:00 INFO - Cloning - done").as_deref(),
            Some("Cloning - done")
        );
        assert_eq!(clean_log_line("plain line").as_deref(), Some("plain line"));
    }

    #[test]
    fn selected_version_emits_plan_then_download_event() {
        let mut tracker = CliProgressTracker::new(vec!["v5.1".into(), "v5.2".into()]);
        let events = tracker.parse_line("INFO - Selected idf version: [\"v5.2\"]");
        assert_eq!(events.len(), 2);
        match &events[0] {
            CliEvent::Plan(plan) => {
                assert_eq!(plan.total_versions, 2);
                assert_eq!(plan.current_version_index, Some(1));
            }
            other => panic!("expected plan, got {other:?}"),
        }
        match &events[1] {
            CliEvent::Progress(p) => {
                assert!(matches!(p.stage, InstallationStage::Download));
                assert_eq!(p.percentage, 10);
                assert_eq!(p.version.as_deref(), Some("v5.2"));
            }
            other => panic!("expected progress, got {other:?}"),
        }
    }

    #[test]
    fn selected_unknown_version_skips_plan() {
        let mut tracker = CliProgressTracker::new(vec!["v5.1".into()]);
        let events = tracker.parse_line("Selected idf version: [master]");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], CliEvent::Progress(_)));
    }

    #[test]
    fn unrelated_line_emits_nothing() {
        let mut tracker = CliProgressTracker::new(vec![]);
        assert!(tracker.parse_line("hello").is_empty());
    }

    #[test]
    fn fixed_phases_report_their_percentages() {
        let mut tracker = CliProgressTracker::new(vec![]);
        let cases = [
            ("Checking for prerequisites", 8),
            ("Python sanity check", 12),
            ("git clone x", 15),
            ("Python environment", 90),
            ("Installation complete", 100),
        ];
        for (line, pct) in cases {
            assert_eq!(single_progress(&mut tracker, line).percentage, pct);
        }
    }

    #[test]
    fn submodule_progress_grows_by_two_and_caps_at_65() {
        let mut tracker = CliProgressTracker::new(vec![]);
        assert_eq!(
            single_progress(&mut tracker, "Updating submodule").percentage,
            7
        );
        single_progress(&mut tracker, "Cloning ESP-IDF");
        assert_eq!(
            single_progress(&mut tracker, "submodule update").percentage,
            17
        );
        tracker.percentage = 64;
        assert_eq!(
            single_progress(&mut tracker, "submodule update").percentage,
            65
        );
    }

    #[test]
    fn tool_download_requires_tools_list_first() {
        let mut tracker = CliProgressTracker::new(vec![]);
        assert!(tracker.parse_line("Downloading tool: cmake").is_empty());

        let list = single_progress(&mut tracker, "Downloading tools: [a, b, c, d]");
        assert!(matches!(list.stage, InstallationStage::Tools));
        assert_eq!(list.percentage, 65);
        assert_eq!(tracker.total, 4);

        assert_eq!(
            single_progress(&mut tracker, "Downloading tool: cmake").percentage,
            65
        );
        assert_eq!(
            single_progress(&mut tracker, "extracted tool: a").percentage,
            70
        );
        assert_eq!(
            single_progress(&mut tracker, "Downloading tool: ninja").percentage,
            70
        );
    }

    #[test]
    fn tools_list_without_brackets_emits_nothing() {
        let mut tracker = CliProgressTracker::new(vec![]);
        assert!(tracker.parse_line("Downloading tools: none").is_empty());
        assert!(!tracker.tools_started);
    }

    #[test]
    fn extracted_tools_cap_at_85() {
        let mut tracker = CliProgressTracker::new(vec![]);
        // No tools list seen: total is treated as 1.
        assert_eq!(
            single_progress(&mut tracker, "Decompression completed").percentage,
            85
        );
        assert_eq!(
            single_progress(&mut tracker, "Decompression completed").percentage,
            85
        );
        assert_eq!(tracker.completed, 2);
    }
}
