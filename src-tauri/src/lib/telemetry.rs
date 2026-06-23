use anyhow::Error as AnyhowError;
use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use sysinfo::System;
use uuid::Uuid;

const CONNECTION_STRING: Option<&str> = option_env!("APP_INSIGHTS_CONNECTION_STRING");

static HTTP_CLIENT: Lazy<Option<reqwest::Client>> = Lazy::new(|| {
    if CONNECTION_STRING.is_none() {
        return None;
    }
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()
});

static ENABLED: AtomicBool = AtomicBool::new(true);

static SYSTEM_INFO: Lazy<String> = Lazy::new(compute_system_info);

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    HTTP_CLIENT.is_some() && ENABLED.load(Ordering::Relaxed)
}

pub const EIM_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Interface {
    Gui,
    Cli,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMode {
    Wizard,
    Simple,
    Offline,
    Fix,
    Cli,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Unknown,
    Network,
    Filesystem,
    DependencyMissing,
    UserCancelled,
    Git,
    Python,
    Configuration,
}

impl ErrorKind {
    pub fn from_anyhow(err: &AnyhowError) -> Self {
        let msg = format!("{:#}", err);
        Self::from_message(&msg)
    }

    pub fn from_message(msg: &str) -> Self {
        let msg = msg.to_lowercase();
        if msg.contains("cancel") || msg.contains("abort") {
            ErrorKind::UserCancelled
        } else if msg.contains("permission")
            || msg.contains("os error")
            || msg.contains("io error")
            || msg.contains("not found")
            || msg.contains("disk")
            || msg.contains("space")
        {
            ErrorKind::Filesystem
        } else if msg.contains("timeout")
            || msg.contains("dns")
            || msg.contains("connection")
            || msg.contains("network")
            || msg.contains("tls")
            || msg.contains("http")
        {
            ErrorKind::Network
        } else if msg.contains("git") {
            ErrorKind::Git
        } else if msg.contains("python") || msg.contains("venv") || msg.contains("pip") {
            ErrorKind::Python
        } else if msg.contains("missing")
            || msg.contains("prerequisite")
            || msg.contains("dependency")
        {
            ErrorKind::DependencyMissing
        } else if msg.contains("config") || msg.contains("invalid") || msg.contains("parse") {
            ErrorKind::Configuration
        } else {
            ErrorKind::Unknown
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallationContext {
    pub session_id: String,
    pub started_at: Instant,
    pub interface: Interface,
    pub mode: InstallMode,
    pub versions: Vec<String>,
    pub installation_ids: Vec<String>,
}

impl InstallationContext {
    pub fn duration_seconds(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64()
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeExtras {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_interactive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_existing_idf: Option<bool>,
}

pub fn new_session(
    interface: Interface,
    mode: InstallMode,
    versions: Vec<String>,
    installation_ids: Vec<String>,
) -> InstallationContext {
    InstallationContext {
        session_id: format!("eim-{}", Uuid::new_v4().simple()),
        started_at: Instant::now(),
        interface,
        mode,
        versions,
        installation_ids,
    }
}

pub fn track_install_started(ctx: &InstallationContext) {
    if !is_enabled() {
        return;
    }
    dispatch(EventProps {
        event_name: "install_started",
        interface: ctx.interface,
        mode: ctx.mode,
        outcome: None,
        session_id: ctx.session_id.clone(),
        installation_ids: ctx.installation_ids.clone(),
        versions: ctx.versions.clone(),
        duration_seconds: None,
        error_kind: None,
        error_message: None,
        error_hash: None,
        extras: OutcomeExtras::default(),
        subcommand: None,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn track_install_outcome(
    ctx: &InstallationContext,
    outcome: InstallOutcome,
    error_kind: Option<ErrorKind>,
    error: Option<&AnyhowError>,
    extras: OutcomeExtras,
) {
    if !is_enabled() {
        return;
    }
    let (error_message, error_hash) = match (outcome, error) {
        (InstallOutcome::Failure, Some(err)) => {
            let raw = format!("{:#}", err);
            let scrubbed = scrub_pii(&raw);
            let hash = hash_short(&raw);
            (Some(scrubbed), Some(hash))
        }
        _ => (None, None),
    };
    dispatch(EventProps {
        event_name: "install_finished",
        interface: ctx.interface,
        mode: ctx.mode,
        outcome: Some(outcome),
        session_id: ctx.session_id.clone(),
        installation_ids: ctx.installation_ids.clone(),
        versions: ctx.versions.clone(),
        duration_seconds: Some(ctx.duration_seconds()),
        error_kind,
        error_message,
        error_hash,
        extras,
        subcommand: None,
    });
}

pub fn track_cli_invoked(subcommand: &str) {
    if !is_enabled() {
        return;
    }
    dispatch(EventProps {
        event_name: "cli_invoked",
        interface: Interface::Cli,
        mode: InstallMode::Cli,
        outcome: None,
        session_id: String::new(),
        installation_ids: Vec::new(),
        versions: Vec::new(),
        duration_seconds: None,
        error_kind: None,
        error_message: None,
        error_hash: None,
        extras: OutcomeExtras::default(),
        subcommand: Some(subcommand.to_string()),
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EventProps {
    #[serde(rename = "eventName")]
    event_name: &'static str,
    interface: Interface,
    mode: InstallMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<InstallOutcome>,
    #[serde(skip_serializing_if = "String::is_empty")]
    session_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    installation_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    versions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_kind: Option<ErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_hash: Option<String>,
    #[serde(flatten)]
    extras: OutcomeExtras,
    #[serde(skip_serializing_if = "Option::is_none")]
    subcommand: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    name: &'static str,
    time: String,
    i_key: String,
    tags: serde_json::Value,
    data: serde_json::Value,
}

fn dispatch(props: EventProps) {
    let Some(client) = HTTP_CLIENT.as_ref() else {
        return;
    };
    let (i_key, ingest_base) = parse_connection_string();
    let url = format!("{}/v2/track", ingest_base);

    let envelope = Envelope {
        name: "Microsoft.ApplicationInsights.Event",
        time: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        i_key,
        tags: serde_json::json!({ "ai.cloud.role": "desktop-app" }),
        data: serde_json::json!({
            "baseType": "EventData",
            "baseData": {
                "name": "eim_event",
                "properties": props,
            }
        }),
    };

    let serial = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        Err(e) => {
            log::trace!("Failed to serialize telemetry payload: {}", e);
            return;
        }
    };

    let client = client.clone();
    tokio::spawn(async move {
        let _ = client
            .post(&url)
            .header("Content-Type", "application/x-json-stream")
            .body(serial)
            .send()
            .await;
    });
}

fn parse_connection_string() -> (String, String) {
    let mut i_key = String::new();
    let mut base = "https://dc.services.visualstudio.com".to_string();
    for kv in CONNECTION_STRING.unwrap().split(';') {
        let mut parts = kv.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("InstrumentationKey"), Some(v)) => i_key = v.into(),
            (Some("IngestionEndpoint"), Some(v)) => base = v.trim_end_matches('/').into(),
            _ => {}
        }
    }
    (i_key, base)
}

pub fn get_system_info() -> String {
    SYSTEM_INFO.clone()
}

fn compute_system_info() -> String {
    let os_name = if std::env::consts::OS == "linux" {
        get_linux_os_name()
    } else {
        System::name()
            .filter(|s| !s.is_empty() && s != "Unknown")
            .unwrap_or_else(|| std::env::consts::OS.to_string())
    };

    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
    let arch = System::cpu_arch();

    format!(
        "OS: {} {} | Architecture: {} | Kernel: {}",
        os_name, os_version, arch, kernel_version
    )
}

pub fn get_linux_os_name() -> String {
    if std::env::consts::OS == "linux" {
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(name) = line.strip_prefix("ID=") {
                    let distro = name.trim_matches('"').to_lowercase();
                    return format!("linux-{}", distro);
                }
            }
        }
        return "linux".to_string();
    }
    "Unknown".to_string()
}

fn scrub_pii(input: &str) -> String {
    static PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        [
            r#"/(?:Users|home|root|tmp|var|etc|opt)/[^\s"'<>]+"#,
            r#"[A-Za-z]:\\[^\s"'<>]+"#,
            r#"\\\\[^\s"'<>]+"#,
            r#"~[^\s"'<>]+"#,
            r#"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"#,
        ]
        .into_iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    });

    let mut out = input.to_string();
    for re in PATTERNS.iter() {
        out = re.replace_all(&out, "<redacted>").into_owned();
    }
    if out.len() > 1024 {
        out.truncate(1024);
        out.push('…');
    }
    out
}

fn hash_short(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{:x}", digest);
    hex.chars().take(16).collect()
}
