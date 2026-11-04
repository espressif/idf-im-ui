use anyhow::Error as AnyhowError;
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use sysinfo::System;
use uuid::Uuid;

const CONNECTION_STRING: Option<&str> = option_env!("APP_INSIGHTS_CONNECTION_STRING");

static HTTP_CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    CONNECTION_STRING?;
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()
});

static ENABLED: AtomicBool = AtomicBool::new(true);

/// Set once `track_install_outcome` has run, so a drop guard does not send a
/// second `install_finished` for the same process.
static OUTCOME_REPORTED: AtomicBool = AtomicBool::new(false);

static SYSTEM_INFO: LazyLock<String> = LazyLock::new(compute_system_info);

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
    /// The prerequisite check itself could not run (shell failed, cannot verify).
    PrerequisiteCheckFailed,
    DiskSpace,
    Permission,
    /// Archive is corrupt, truncated or has an unexpected size.
    ArchiveInvalid,
    /// The app was closed while an install was still running.
    AppClosed,
}

/// Who is responsible for a failure. Separates problems on the user's machine
/// from genuine installer defects, so the success rate reflects EIM itself.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Something about the user's machine: missing tools, broken python, no
    /// network, full disk, denied permissions.
    Environment,
    /// A defect in EIM: our extraction, copy, or tool setup.
    Installer,
    /// A user choice or action: bad config input, quitting mid-install.
    User,
    /// The cause could not be classified. Not counted against the installer.
    Unknown,
}

/// Where in the install the failure happened.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    Checking,
    Prerequisites,
    Download,
    Extract,
    Tools,
    Python,
    Configure,
}

impl ErrorKind {
    /// Fallback class for a kind, used when a call site does not state one.
    pub fn default_class(&self) -> FailureClass {
        match self {
            ErrorKind::Network
            | ErrorKind::DependencyMissing
            | ErrorKind::PrerequisiteCheckFailed
            | ErrorKind::DiskSpace
            | ErrorKind::Permission
            | ErrorKind::Python => FailureClass::Environment,
            // Call sites that know the user chose badly pass `FailureClass::User`
            // themselves. The default is the installer, which is what the
            // extract and config-save paths already report.
            ErrorKind::ArchiveInvalid
            | ErrorKind::Configuration
            | ErrorKind::Git
            | ErrorKind::Filesystem => FailureClass::Installer,
            ErrorKind::UserCancelled | ErrorKind::AppClosed => FailureClass::User,
            // Unclassified fallbacks must not count against the installer rate.
            ErrorKind::Unknown => FailureClass::Unknown,
        }
    }

    pub fn from_anyhow(err: &AnyhowError) -> Self {
        let msg = format!("{:#}", err);
        Self::from_message(&msg)
    }

    /// Keyword fallback for errors that reach telemetry without an explicit
    /// classification. Always feed this the raw error text, never a localized
    /// `rust_i18n::t!` string, or it will mostly yield `Unknown`.
    ///
    /// Order matters: prerequisite and disk/permission phrasing is checked
    /// before the broader `git` / `python` / `os error` matches, because
    /// "missing prerequisites: git" is a dependency problem, not a git one.
    ///
    /// Deliberately does not match a bare "missing". It would also catch
    /// "missing field" from a deserialization error and similar installer
    /// defects, and classifying those as `environment` would hide them from
    /// the installer success rate. Every prerequisite message EIM sends says
    /// "prerequisite", so matching the word itself is enough.
    pub fn from_message(msg: &str) -> Self {
        let msg = msg.to_lowercase();
        if msg.contains("prerequisite") || msg.contains("dependency") {
            ErrorKind::DependencyMissing
        } else if is_disk_space_message(&msg) {
            ErrorKind::DiskSpace
        } else if is_permission_message(&msg) {
            ErrorKind::Permission
        } else if msg.contains("python") || msg.contains("venv") || msg.contains("pip") {
            ErrorKind::Python
        } else if msg.contains("git") {
            ErrorKind::Git
        } else if is_network_os_error(&msg) || is_network_message(&msg) {
            ErrorKind::Network
        } else if msg.contains("cancel") || msg.contains("abort") {
            ErrorKind::UserCancelled
        } else if msg.contains("os error") || msg.contains("io error") || msg.contains("not found")
        {
            ErrorKind::Filesystem
        } else if msg.contains("config") || msg.contains("invalid") || msg.contains("parse") {
            ErrorKind::Configuration
        } else {
            ErrorKind::Unknown
        }
    }
}

/// `os error 5` must not match `os error 50` or `os error 53`.
fn contains_os_error(msg: &str, code: u32) -> bool {
    let needle = format!("os error {code}");
    let mut rest = msg;
    while let Some(index) = rest.find(&needle) {
        let after = &rest[index + needle.len()..];
        match after.chars().next() {
            Some(c) if c.is_ascii_digit() => {}
            _ => return true,
        }
        rest = &rest[index + needle.len()..];
    }
    false
}

fn is_disk_space_message(msg: &str) -> bool {
    msg.contains("no space left")
        || msg.contains("no space")
        || msg.contains("disk full")
        || msg.contains("enospc")
        || contains_os_error(msg, 28)
        || contains_os_error(msg, 112)
        || msg.contains("write to disk")
        || (msg.contains("disk") && (msg.contains("full") || msg.contains("space")))
}

fn is_permission_message(msg: &str) -> bool {
    msg.contains("permission denied")
        || msg.contains("permission error")
        || msg.contains("access is denied")
        || msg.contains("access denied")
        || msg.contains("eacces")
        || msg.contains("not permitted")
        || contains_os_error(msg, 13)
        || contains_os_error(msg, 5)
}

/// Windows network-path failures. These contain "os error N" but are not
/// permission errors and are not defects in EIM.
fn is_network_os_error(msg: &str) -> bool {
    contains_os_error(msg, 53)
        || contains_os_error(msg, 67)
        || contains_os_error(msg, 1231)
        || contains_os_error(msg, 1232)
}

fn is_network_message(msg: &str) -> bool {
    msg.contains("timeout")
        || msg.contains("dns")
        || msg.contains("connection")
        || msg.contains("network")
        || msg.contains("tls")
        || msg.contains("http")
}

/// Maps an `io::Error` onto the kinds that describe machine problems, so a
/// full disk or a denied path is not reported as an installer defect.
pub fn kind_from_io_error(err: &std::io::Error) -> Option<ErrorKind> {
    match err.kind() {
        std::io::ErrorKind::PermissionDenied => Some(ErrorKind::Permission),
        std::io::ErrorKind::StorageFull => Some(ErrorKind::DiskSpace),
        _ => match err.raw_os_error() {
            // ENOSPC on unix, ERROR_DISK_FULL on windows.
            Some(28) | Some(112) => Some(ErrorKind::DiskSpace),
            _ => None,
        },
    }
}

/// Classifies a filesystem failure in our own pipeline. A full disk or a
/// denied path is the user's machine, anything else is an EIM defect.
pub fn classify_fs_failure(err: &std::io::Error) -> (ErrorKind, FailureClass) {
    match kind_from_io_error(err) {
        Some(kind) => (kind, FailureClass::Environment),
        None => (ErrorKind::Filesystem, FailureClass::Installer),
    }
}

/// Same as `classify_fs_failure` for errors that have already been flattened
/// into a string and so no longer carry an `io::ErrorKind`.
pub fn classify_fs_failure_message(msg: &str) -> (ErrorKind, FailureClass) {
    match ErrorKind::from_message(msg) {
        kind @ (ErrorKind::DiskSpace | ErrorKind::Permission) => (kind, FailureClass::Environment),
        _ => (ErrorKind::Filesystem, FailureClass::Installer),
    }
}

/// A failure recorded at the place it happened, so the outcome event can report
/// the real cause instead of guessing from a translated message.
///
/// The original text is hashed and discarded. It is never stored here and
/// never sent.
#[derive(Debug, Clone)]
pub struct ClassifiedFailure {
    pub kind: ErrorKind,
    pub class: FailureClass,
    pub stage: Option<FailureStage>,
    /// SHA-256 prefix of the local error text. The text itself is not kept.
    pub error_hash: String,
    /// Tool names for `DependencyMissing`, for example `git`, `cmake`.
    pub missing: Vec<String>,
}

/// Holds the most recent classified failure until the outcome event is sent.
/// A single global slot is enough because installs are serialized: the GUI
/// guards them with `is_installing` and the CLI runs one install per process.
static PENDING_FAILURE: LazyLock<std::sync::Mutex<Option<ClassifiedFailure>>> =
    LazyLock::new(|| std::sync::Mutex::new(None));

/// Records the cause of a failure at the point it is detected.
pub fn note_failure(
    kind: ErrorKind,
    class: FailureClass,
    stage: Option<FailureStage>,
    raw: impl Into<String>,
    missing: Vec<String>,
) {
    let raw = raw.into();
    let failure = ClassifiedFailure {
        kind,
        class,
        stage,
        error_hash: hash_short(&raw),
        missing: normalize_missing_names(missing),
    };
    drop(raw);
    log::debug!(
        "Telemetry failure noted: kind={:?} class={:?} stage={:?}",
        failure.kind,
        failure.class,
        failure.stage
    );
    if let Ok(mut slot) = PENDING_FAILURE.lock() {
        *slot = Some(failure);
    }
}

/// Same as `note_failure` but derives the class from the kind.
pub fn note_failure_kind(kind: ErrorKind, stage: Option<FailureStage>, raw: impl Into<String>) {
    note_failure(kind, kind.default_class(), stage, raw, Vec::new());
}

/// Lowercases, sorts and deduplicates prerequisite tool names so the same
/// machine state always produces the same `missingPrerequisites` value.
pub fn normalize_missing_names(missing: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut names: Vec<String> = missing
        .into_iter()
        .map(|m| m.as_ref().to_lowercase())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Records missing system tools for telemetry.
///
/// Shared by the CLI and GUI so the dashboard can show which prerequisite is
/// most often absent instead of a generic failure. Names are normalized before
/// they are stored.
pub fn note_missing_prerequisites(
    reason: &str,
    missing: impl IntoIterator<Item = impl AsRef<str>>,
) {
    let names = normalize_missing_names(missing);
    note_failure(
        ErrorKind::DependencyMissing,
        FailureClass::Environment,
        Some(FailureStage::Prerequisites),
        format!("{}: {}", reason, names.join(", ")),
        names,
    );
}

/// Records a failure only if none was recorded yet.
///
/// Outer layers see the inner error flattened into a localized string, so they
/// can only classify it vaguely. Inner error sites know the real cause. This
/// lets a wrapper provide a fallback without overwriting what the inner step
/// already reported.
pub fn note_failure_if_absent(
    kind: ErrorKind,
    class: FailureClass,
    stage: Option<FailureStage>,
    raw: impl Into<String>,
) {
    if let Ok(mut slot) = PENDING_FAILURE.lock() {
        if slot.is_none() {
            let raw = raw.into();
            *slot = Some(ClassifiedFailure {
                kind,
                class,
                stage,
                error_hash: hash_short(&raw),
                missing: Vec::new(),
            });
        }
    }
}

/// Whether an error site has already classified the current failure.
#[cfg(test)]
fn has_pending_failure() -> bool {
    PENDING_FAILURE
        .lock()
        .map(|slot| slot.is_some())
        .unwrap_or(false)
}

/// Drops a failure noted for an earlier step.
///
/// Call this at the start of each version so a cause from the previous
/// version cannot be reported for the next one.
pub fn clear_pending_failure() {
    if let Ok(mut slot) = PENDING_FAILURE.lock() {
        *slot = None;
    }
}

fn take_pending_failure() -> Option<ClassifiedFailure> {
    PENDING_FAILURE.lock().ok().and_then(|mut slot| slot.take())
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
    // Always clear, even when telemetry is off, so a stale failure from an
    // earlier attempt can never be attributed to this one.
    clear_pending_failure();
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
        error_hash: None,
        failure_class: None,
        failure_stage: None,
        missing_prerequisites: Vec::new(),
        extras: OutcomeExtras::default(),
        subcommand: None,
    });
}

/// Sends `install_finished`.
///
/// On failure the cause comes from the classification recorded by
/// `note_failure` at the error site. `error_kind` and `error` are only used
/// when nothing was recorded, which keeps the CLI call sites working.
#[allow(clippy::too_many_arguments)]
pub fn track_install_outcome(
    ctx: &InstallationContext,
    outcome: InstallOutcome,
    error_kind: Option<ErrorKind>,
    error: Option<&AnyhowError>,
    extras: OutcomeExtras,
) {
    OUTCOME_REPORTED.store(true, Ordering::Relaxed);
    let classified = match outcome {
        InstallOutcome::Failure => take_pending_failure(),
        // A success leaves nothing to attribute; drop anything recorded by a
        // recoverable step so it cannot leak into the next install.
        InstallOutcome::Success => {
            clear_pending_failure();
            None
        }
    };
    if !is_enabled() {
        return;
    }

    // Used only to classify and hash when no site recorded a cause. Dropped
    // before the event is built, so the text never leaves the process.
    let fallback_text = error.map(|err| format!("{:#}", err));
    let error_hash = match outcome {
        InstallOutcome::Failure => classified
            .as_ref()
            .map(|failure| failure.error_hash.clone())
            .or_else(|| fallback_text.as_deref().map(hash_short)),
        InstallOutcome::Success => None,
    };

    let kind = match (outcome, &classified) {
        (InstallOutcome::Failure, Some(failure)) => Some(failure.kind),
        (InstallOutcome::Failure, None) => error_kind.or_else(|| {
            fallback_text
                .as_deref()
                .map(ErrorKind::from_message)
                .or(Some(ErrorKind::Unknown))
        }),
        (InstallOutcome::Success, _) => None,
    };
    let failure_class = match (outcome, &classified) {
        (InstallOutcome::Failure, Some(failure)) => Some(failure.class),
        (InstallOutcome::Failure, None) => kind.map(|k| k.default_class()),
        (InstallOutcome::Success, _) => None,
    };
    let failure_stage = classified.as_ref().and_then(|f| f.stage);
    let missing_prerequisites = classified
        .as_ref()
        .map(|f| f.missing.clone())
        .unwrap_or_default();

    dispatch(EventProps {
        event_name: "install_finished",
        interface: ctx.interface,
        mode: ctx.mode,
        outcome: Some(outcome),
        session_id: ctx.session_id.clone(),
        installation_ids: ctx.installation_ids.clone(),
        versions: ctx.versions.clone(),
        duration_seconds: Some(ctx.duration_seconds()),
        error_kind: kind,
        error_hash,
        failure_class,
        failure_stage,
        missing_prerequisites,
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
        error_hash: None,
        failure_class: None,
        failure_stage: None,
        missing_prerequisites: Vec::new(),
        extras: OutcomeExtras::default(),
        subcommand: Some(subcommand.to_string()),
    });
}

/// Session handed over by a parent process via `EIM_TELEMETRY_*`.
///
/// Used when the child exits before it can build a full install context.
pub fn context_from_parent_env() -> Option<InstallationContext> {
    let session_id = std::env::var("EIM_TELEMETRY_SESSION").ok()?;
    if session_id.is_empty() {
        return None;
    }
    let interface = match std::env::var("EIM_TELEMETRY_INTERFACE").as_deref() {
        Ok("gui") => Interface::Gui,
        _ => Interface::Cli,
    };
    let mode = match std::env::var("EIM_TELEMETRY_MODE").as_deref() {
        Ok("wizard") => InstallMode::Wizard,
        Ok("simple") => InstallMode::Simple,
        Ok("offline") => InstallMode::Offline,
        Ok("fix") => InstallMode::Fix,
        _ => InstallMode::Cli,
    };
    let mut ctx = new_session(interface, mode, Vec::new(), Vec::new());
    ctx.session_id = session_id;
    Some(ctx)
}

/// Sends one `install_finished` when an inherited child exits without one.
///
/// The reason is a fixed string. It is hashed for clustering and then dropped.
/// No system-function output is included.
pub fn note_unreported_child_exit(ctx: &InstallationContext) {
    if OUTCOME_REPORTED.swap(true, Ordering::Relaxed) {
        return;
    }
    note_failure(
        ErrorKind::Unknown,
        FailureClass::Unknown,
        None,
        "child installer exited before reporting an outcome",
        Vec::new(),
    );
    track_install_outcome(
        ctx,
        InstallOutcome::Failure,
        Some(ErrorKind::Unknown),
        None,
        OutcomeExtras::default(),
    );
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
    error_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_class: Option<FailureClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_stage: Option<FailureStage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missing_prerequisites: Vec<String>,
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

    // Attach the static app-version/system-info fields here, once, so every event
    // carries them regardless of which track_* function built it.
    let mut properties = match serde_json::to_value(&props) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => return,
    };
    properties.insert(
        "appVersion".to_string(),
        serde_json::Value::String(EIM_VERSION.to_string()),
    );
    properties.insert(
        "systemInfo".to_string(),
        serde_json::Value::String(get_system_info()),
    );

    let envelope = Envelope {
        name: "Microsoft.ApplicationInsights.Event",
        time: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        i_key,
        tags: serde_json::json!({ "ai.cloud.role": "desktop-app" }),
        data: serde_json::json!({
            "baseType": "EventData",
            "baseData": {
                "name": "eim_event",
                "properties": properties,
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

    if tokio::runtime::Handle::try_current().is_err() {
        // The Tauri run loop is outside any Tokio runtime, so spawn would panic.
        // Keep the body for `flush`, which sends it with `block_on` from that thread.
        if let Ok(mut queued) = QUEUED_DISPATCHES.lock() {
            queued.push(QueuedDispatch { url, body: serial });
        }
        return;
    }
    let client = client.clone();
    let handle = tokio::spawn(async move {
        let _ = client
            .post(&url)
            .header("Content-Type", "application/x-json-stream")
            .body(serial)
            .send()
            .await;
    });
    if let Ok(mut pending) = PENDING_DISPATCHES.lock() {
        pending.retain(|handle| !handle.is_finished());
        pending.push(handle);
    }
}

struct QueuedDispatch {
    url: String,
    body: String,
}

static QUEUED_DISPATCHES: LazyLock<std::sync::Mutex<Vec<QueuedDispatch>>> =
    LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

static PENDING_DISPATCHES: LazyLock<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>> =
    LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// Whether any telemetry HTTP requests are still in flight or waiting to be sent.
pub fn has_pending_dispatches() -> bool {
    let queued = QUEUED_DISPATCHES
        .lock()
        .map(|queued| !queued.is_empty())
        .unwrap_or(false);
    if queued {
        return true;
    }
    let Ok(mut pending) = PENDING_DISPATCHES.lock() else {
        return false;
    };
    pending.retain(|handle| !handle.is_finished());
    !pending.is_empty()
}

pub async fn flush(timeout: Duration) {
    let queued = match QUEUED_DISPATCHES.lock() {
        Ok(mut queued) => std::mem::take(&mut *queued),
        Err(_) => Vec::new(),
    };
    let handles: Vec<_> = match PENDING_DISPATCHES.lock() {
        Ok(mut pending) => std::mem::take(&mut *pending),
        Err(_) => Vec::new(),
    };
    if queued.is_empty() && handles.is_empty() {
        return;
    }
    let client = HTTP_CLIENT.as_ref().cloned();
    let _ = tokio::time::timeout(timeout, async {
        if let Some(client) = client {
            for item in queued {
                let _ = client
                    .post(&item.url)
                    .header("Content-Type", "application/x-json-stream")
                    .body(item.body)
                    .send()
                    .await;
            }
        }
        for handle in handles {
            let _ = handle.await;
        }
    })
    .await;
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

fn hash_short(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let mut hex = crate::to_hex(&hasher.finalize());
    hex.truncate(16);
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The failure slot is process-wide, so these tests must not run
    /// concurrently with each other.
    static SLOT_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn missing_prerequisites_is_a_dependency_problem_not_a_git_one() {
        // Regression: "git" used to be matched first, so a user missing git
        // and cmake was reported as a git failure.
        assert_eq!(
            ErrorKind::from_message("Missing prerequisites: git, cmake"),
            ErrorKind::DependencyMissing
        );
        assert_eq!(
            ErrorKind::from_message("please install the following dependency: python3"),
            ErrorKind::DependencyMissing
        );
    }

    #[test]
    fn a_bare_missing_is_not_a_dependency_problem() {
        // "missing" on its own shows up in deserialization and lookup errors,
        // which are installer defects. Classifying them as environment would
        // quietly remove them from the installer success rate.
        assert_ne!(
            ErrorKind::from_message("invalid config: missing field `version`"),
            ErrorKind::DependencyMissing
        );
        assert_ne!(
            ErrorKind::from_message("archive entry is missing"),
            ErrorKind::DependencyMissing
        );
    }

    #[test]
    fn plain_git_and_python_errors_still_classify() {
        assert_eq!(
            ErrorKind::from_message("git clone failed: remote hung up"),
            ErrorKind::Git
        );
        assert_eq!(
            ErrorKind::from_message("failed to create venv"),
            ErrorKind::Python
        );
    }

    #[test]
    fn out_of_space_and_denied_paths_are_not_generic_filesystem_errors() {
        assert_eq!(
            ErrorKind::from_message("failed to write: No space left on device (os error 28)"),
            ErrorKind::DiskSpace
        );
        assert_eq!(
            ErrorKind::from_message("copy failed: os error 112"),
            ErrorKind::DiskSpace
        );
        assert_eq!(
            ErrorKind::from_message("failed to write to disk"),
            ErrorKind::DiskSpace
        );
        assert_eq!(
            ErrorKind::from_message("open failed: Permission denied (os error 13)"),
            ErrorKind::Permission
        );
        assert_eq!(
            ErrorKind::from_message("permission error"),
            ErrorKind::Permission
        );
    }

    #[test]
    fn missing_prerequisite_names_are_normalized_before_recording() {
        let _guard = SLOT_GUARD.lock().unwrap();
        clear_pending_failure();

        note_missing_prerequisites("missing prerequisites", ["CMake", "git", "cmake"]);
        let failure = take_pending_failure().expect("noted");
        assert_eq!(failure.missing, vec!["cmake", "git"]);
        assert_eq!(failure.kind, ErrorKind::DependencyMissing);
        assert_eq!(failure.class, FailureClass::Environment);
    }

    #[test]
    fn machine_problems_are_not_blamed_on_the_installer() {
        for kind in [
            ErrorKind::Network,
            ErrorKind::DependencyMissing,
            ErrorKind::PrerequisiteCheckFailed,
            ErrorKind::DiskSpace,
            ErrorKind::Permission,
            ErrorKind::Python,
        ] {
            assert_eq!(
                kind.default_class(),
                FailureClass::Environment,
                "{:?} should not count against the installer",
                kind
            );
        }
        assert_eq!(ErrorKind::AppClosed.default_class(), FailureClass::User);
        assert_eq!(ErrorKind::Unknown.default_class(), FailureClass::Unknown);
        assert_eq!(
            ErrorKind::ArchiveInvalid.default_class(),
            FailureClass::Installer
        );
        assert_eq!(
            ErrorKind::Configuration.default_class(),
            FailureClass::Installer
        );
    }

    #[test]
    fn os_error_codes_do_not_match_a_longer_number() {
        assert_eq!(ErrorKind::from_message("os error 53"), ErrorKind::Network);
        assert_eq!(
            ErrorKind::from_message("os error 53").default_class(),
            FailureClass::Environment
        );
        assert_ne!(
            ErrorKind::from_message("os error 50"),
            ErrorKind::Permission
        );
        assert_eq!(
            ErrorKind::from_message("Access is denied. (os error 5)"),
            ErrorKind::Permission
        );
        assert_eq!(
            ErrorKind::from_message("os error 5)"),
            ErrorKind::Permission
        );
        assert_ne!(
            ErrorKind::from_message("os error 280"),
            ErrorKind::DiskSpace
        );
        assert_eq!(ErrorKind::from_message("os error 28"), ErrorKind::DiskSpace);
        assert_ne!(
            ErrorKind::from_message("invalid permissions configuration"),
            ErrorKind::Permission
        );
    }

    #[test]
    fn filesystem_helpers_live_with_telemetry() {
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        assert_eq!(
            classify_fs_failure(&denied),
            (ErrorKind::Permission, FailureClass::Environment)
        );
        let other = std::io::Error::new(std::io::ErrorKind::InvalidData, "corrupt");
        assert_eq!(
            classify_fs_failure(&other),
            (ErrorKind::Filesystem, FailureClass::Installer)
        );
        assert_eq!(
            classify_fs_failure_message("no space left on device"),
            (ErrorKind::DiskSpace, FailureClass::Environment)
        );
    }

    #[test]
    fn a_later_note_replaces_an_earlier_one() {
        let _guard = SLOT_GUARD.lock().unwrap();
        clear_pending_failure();

        note_failure(
            ErrorKind::Python,
            FailureClass::Environment,
            Some(FailureStage::Python),
            "python failed",
            Vec::new(),
        );
        note_failure(
            ErrorKind::Network,
            FailureClass::Environment,
            Some(FailureStage::Download),
            "connection reset",
            Vec::new(),
        );

        let failure = take_pending_failure().expect("the later note should be kept");
        assert_eq!(failure.kind, ErrorKind::Network);
        assert_eq!(failure.class, FailureClass::Environment);
        assert_eq!(failure.stage, Some(FailureStage::Download));
    }

    #[test]
    fn noted_failures_keep_a_hash_and_not_the_text() {
        let _guard = SLOT_GUARD.lock().unwrap();
        clear_pending_failure();
        let raw = "failed to write /Users/secret/idf: os error 5";
        note_failure(
            ErrorKind::Permission,
            FailureClass::Environment,
            Some(FailureStage::Extract),
            raw,
            Vec::new(),
        );
        let failure = take_pending_failure().expect("noted");
        assert_eq!(failure.error_hash, hash_short(raw));
        assert_eq!(failure.error_hash.len(), 16);
        assert!(!failure.error_hash.contains("secret"));
        assert!(!failure.error_hash.contains("os error"));
    }

    #[test]
    fn io_errors_map_only_when_the_machine_is_at_fault() {
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        assert_eq!(kind_from_io_error(&denied), Some(ErrorKind::Permission));

        let enospc = std::io::Error::from_raw_os_error(28);
        assert_eq!(kind_from_io_error(&enospc), Some(ErrorKind::DiskSpace));

        let other = std::io::Error::new(std::io::ErrorKind::InvalidData, "corrupt");
        assert_eq!(kind_from_io_error(&other), None);
    }

    #[test]
    fn a_noted_failure_is_kept_and_an_absent_one_does_not_overwrite_it() {
        let _guard = SLOT_GUARD.lock().unwrap();
        clear_pending_failure();

        note_failure(
            ErrorKind::DependencyMissing,
            FailureClass::Environment,
            Some(FailureStage::Prerequisites),
            "missing prerequisites: cmake, git",
            vec!["cmake".to_string(), "git".to_string()],
        );
        // An outer wrapper re-reporting the same failure must not clobber it.
        note_failure_if_absent(
            ErrorKind::Unknown,
            FailureClass::Installer,
            Some(FailureStage::Tools),
            "install failed",
        );

        let failure = take_pending_failure().expect("the noted failure should still be there");
        assert_eq!(failure.kind, ErrorKind::DependencyMissing);
        assert_eq!(failure.class, FailureClass::Environment);
        assert_eq!(failure.stage, Some(FailureStage::Prerequisites));
        assert_eq!(failure.missing, vec!["cmake", "git"]);
        assert!(!has_pending_failure(), "taking should consume the slot");
    }

    #[test]
    fn a_fallback_is_used_when_nothing_classified_the_failure() {
        let _guard = SLOT_GUARD.lock().unwrap();
        clear_pending_failure();

        note_failure_if_absent(
            ErrorKind::Unknown,
            FailureClass::Installer,
            Some(FailureStage::Extract),
            "something broke",
        );

        let failure = take_pending_failure().expect("the fallback should be recorded");
        assert_eq!(failure.kind, ErrorKind::Unknown);
        assert_eq!(failure.class, FailureClass::Installer);
    }

    #[test]
    fn starting_an_install_discards_a_failure_from_an_earlier_attempt() {
        let _guard = SLOT_GUARD.lock().unwrap();
        note_failure_kind(
            ErrorKind::Network,
            Some(FailureStage::Download),
            "connection reset",
        );
        assert!(has_pending_failure());

        let ctx = new_session(Interface::Gui, InstallMode::Simple, Vec::new(), Vec::new());
        track_install_started(&ctx);

        assert!(
            !has_pending_failure(),
            "a retry must not inherit the previous attempt's cause"
        );
    }

    #[test]
    fn outcome_dimensions_are_serialized_in_the_names_the_dashboard_queries() {
        let props = EventProps {
            event_name: "install_finished",
            interface: Interface::Gui,
            mode: InstallMode::Simple,
            outcome: Some(InstallOutcome::Failure),
            session_id: "eim-test".to_string(),
            installation_ids: Vec::new(),
            versions: vec!["v5.4".to_string()],
            duration_seconds: Some(12.5),
            error_kind: Some(ErrorKind::DependencyMissing),
            error_hash: Some("abc123".to_string()),
            failure_class: Some(FailureClass::Environment),
            failure_stage: Some(FailureStage::Prerequisites),
            missing_prerequisites: vec!["cmake".to_string()],
            extras: OutcomeExtras::default(),
            subcommand: None,
        };

        let json = serde_json::to_value(&props).expect("props should serialize");
        assert_eq!(json["failureClass"], "environment");
        assert_eq!(json["failureStage"], "prerequisites");
        assert_eq!(json["missingPrerequisites"][0], "cmake");
        assert_eq!(json["errorKind"], "dependency_missing");
        assert_eq!(json["outcome"], "failure");
        assert_eq!(json["errorHash"], "abc123");
        assert!(json.get("errorMessage").is_none());
    }

    #[test]
    fn unset_outcome_dimensions_are_left_out_entirely() {
        // Successes must not carry empty failure fields, so the dashboard can
        // tell "no failure" from "failure we could not classify".
        let props = EventProps {
            event_name: "install_finished",
            interface: Interface::Cli,
            mode: InstallMode::Cli,
            outcome: Some(InstallOutcome::Success),
            session_id: "eim-test".to_string(),
            installation_ids: Vec::new(),
            versions: Vec::new(),
            duration_seconds: Some(1.0),
            error_kind: None,
            error_hash: None,
            failure_class: None,
            failure_stage: None,
            missing_prerequisites: Vec::new(),
            extras: OutcomeExtras::default(),
            subcommand: None,
        };

        let json = serde_json::to_value(&props).expect("props should serialize");
        assert!(json.get("failureClass").is_none());
        assert!(json.get("failureStage").is_none());
        assert!(json.get("missingPrerequisites").is_none());
        assert!(json.get("errorKind").is_none());
        assert!(json.get("errorMessage").is_none());
    }
}
