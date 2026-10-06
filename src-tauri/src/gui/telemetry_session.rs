//! Install-outcome telemetry for the GUI, owned by the Rust commands.
//!
//! Outcomes used to be reported by the frontend invoking `track_event_command`
//! from Vue listeners. That made every outcome depend on the component still
//! being mounted and on the invoke arguments deserializing, and one bad
//! argument silently dropped the event. The reported GUI success rate for
//! v0.19.0 was about 1%, against a real rate above 50%.
//!
//! Now each install command wraps its own `Result`, the same way the CLI does
//! in `crate::cli`, so an attempt is recorded whatever the UI does.

use idf_im_lib::settings::Settings;
use idf_im_lib::telemetry::{
    self, ErrorKind, FailureClass, InstallMode, InstallOutcome, InstallationContext, Interface,
    OutcomeExtras,
};
use tauri::{AppHandle, Manager};

use crate::gui::app_state::{get_settings_non_blocking, AppState};
use crate::gui::commands::utils_commands::get_app_settings;

/// Whether the user has allowed usage statistics.
///
/// Mirrors the gate the frontend used to apply before invoking
/// `track_event_command`: a missing setting means the question has not been
/// answered yet and is treated as a no.
pub fn usage_statistics_allowed(app_handle: &AppHandle) -> bool {
    match get_app_settings(app_handle.clone()).get("usage_statistics") {
        Some(serde_json::Value::Bool(allowed)) => *allowed,
        Some(_) | None => false,
    }
}

/// Opens an install session and sends `install_started`.
///
/// Returns `None`, which makes the matching `finish` a no-op, when either
/// telemetry is off or a session is already open. The second case is what
/// keeps nested commands honest: simple setup calls the wizard command, and
/// simple offline calls the offline pipeline, so without this the inner
/// command would report a second install under the wrong mode. The outermost
/// caller wins, which is the mode the user actually picked.
///
/// Versions and installation ids come from the current settings, so the event
/// carries them even though the frontend no longer passes anything.
pub fn begin(app_handle: &AppHandle, mode: InstallMode) -> Option<InstallationContext> {
    if !usage_statistics_allowed(app_handle) {
        log::debug!("Usage statistics not allowed, not tracking this install.");
        return None;
    }

    let state = app_handle.state::<AppState>();
    let mut slot = match state.telemetry_session.lock() {
        Ok(slot) => slot,
        Err(_) => {
            log::warn!("Failed to lock the telemetry session; this install will have no outcome.");
            return None;
        }
    };
    if slot.is_some() {
        log::debug!("An install session is already open; not opening a nested one.");
        return None;
    }

    let settings = get_settings_non_blocking(app_handle).ok();
    let versions = settings
        .as_ref()
        .and_then(|s| s.idf_versions.clone())
        .unwrap_or_default();
    let installation_ids = settings
        .as_ref()
        .and_then(|s| s.pending_installation_ids.as_ref())
        .map(|ids| ids.values().cloned().collect())
        .unwrap_or_default();

    let ctx = telemetry::new_session(Interface::Gui, mode, versions, installation_ids);
    telemetry::track_install_started(&ctx);

    // Also kept in app state so the exit handler can close a session that is
    // still open when the user quits mid-install.
    *slot = Some(ctx.clone());
    Some(ctx)
}

/// The session currently open, if any.
///
/// Windows needs this: simple setup opens the session and then delegates to
/// the wizard command, which has to pass the same session id down to the
/// `eim install` subprocess.
pub fn current_session(app_handle: &AppHandle) -> Option<InstallationContext> {
    app_handle
        .state::<AppState>()
        .telemetry_session
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

/// Takes the open session out of app state if it is still the given one.
///
/// The slot doubles as "this session still needs an outcome from us", so
/// claiming it is what makes a caller responsible for reporting. Returns false
/// when someone else already reported or took over.
fn claim(app_handle: &AppHandle, session_id: &str) -> bool {
    match app_handle.state::<AppState>().telemetry_session.lock() {
        Ok(mut slot) => {
            let matches = slot
                .as_ref()
                .is_some_and(|open| open.session_id == session_id);
            if matches {
                *slot = None;
            }
            matches
        }
        Err(_) => false,
    }
}

/// Gives up ownership of the open session without reporting an outcome.
///
/// Used on Windows, where the wizard installs by spawning `eim install` and
/// returns as soon as the subprocess starts. The child carries the same
/// session id and sends the real outcome, so reporting here would both
/// pre-empt it with a premature success and double-count the install.
pub fn hand_off_to_subprocess(app_handle: &AppHandle) {
    if let Ok(mut slot) = app_handle.state::<AppState>().telemetry_session.lock() {
        if slot.take().is_some() {
            log::debug!("Install subprocess now owns the telemetry session outcome.");
        }
    }
}

/// Sends `install_finished` for a session opened by `begin`.
///
/// Does nothing when the session has already been reported or handed off to a
/// subprocess, so an outer command can call this unconditionally.
///
/// A failure takes its cause from whichever error site called
/// `telemetry::note_failure`. The `Err` string here is localized, so it only
/// serves as a fallback for paths that do not classify themselves.
pub fn finish<T>(
    app_handle: &AppHandle,
    ctx: Option<InstallationContext>,
    result: &Result<T, String>,
) {
    let Some(mut ctx) = ctx else {
        return;
    };
    if !claim(app_handle, &ctx.session_id) {
        return;
    }

    let settings = get_settings_non_blocking(app_handle).ok();

    // Simple and offline installs pick their version after `begin` has run, so
    // backfill it here or their outcome events would carry no version at all.
    if ctx.versions.is_empty() {
        if let Some(versions) = settings.as_ref().and_then(|s| s.idf_versions.clone()) {
            ctx.versions = versions;
        }
    }

    let extras = settings
        .as_ref()
        .map(extras_from_settings)
        .unwrap_or_default();

    match result {
        Ok(_) => {
            telemetry::track_install_outcome(&ctx, InstallOutcome::Success, None, None, extras)
        }
        Err(err) => {
            let fallback = anyhow::anyhow!(err.clone());
            telemetry::track_install_outcome(
                &ctx,
                InstallOutcome::Failure,
                None,
                Some(&fallback),
                extras,
            );
        }
    }
}

/// Closes a session that is still open, for when the app exits mid-install.
///
/// Reported as a user-class failure: the install never finished, but nothing
/// went wrong with the installer, so it must not count against the success
/// rate. Returns whether there was a session to close.
pub fn abandon_open_session(app_handle: &AppHandle) -> bool {
    let ctx = app_handle
        .state::<AppState>()
        .telemetry_session
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());

    let Some(ctx) = ctx else {
        return false;
    };

    log::info!("App closing with an install still running; reporting it as abandoned.");
    telemetry::note_failure(
        ErrorKind::AppClosed,
        FailureClass::User,
        None,
        "the app was closed while the install was still running",
        Vec::new(),
    );
    telemetry::track_install_outcome(
        &ctx,
        InstallOutcome::Failure,
        None,
        None,
        OutcomeExtras::default(),
    );
    true
}

/// Install counts for the outcome event, so a failure can be read against how
/// much was being installed.
fn extras_from_settings(settings: &Settings) -> OutcomeExtras {
    let feature_count = settings.idf_features.as_ref().map(|v| v.len()).or_else(|| {
        settings
            .idf_features_per_version
            .as_ref()
            .map(|m| m.values().map(|v| v.len()).sum())
    });
    let tool_count = settings.idf_tools.as_ref().map(|v| v.len()).or_else(|| {
        settings
            .idf_tools_per_version
            .as_ref()
            .map(|m| m.values().map(|v| v.len()).sum())
    });
    OutcomeExtras {
        feature_count,
        tool_count,
        target_count: settings.target.as_ref().map(|v| v.len()),
        non_interactive: settings.non_interactive,
        used_existing_idf: None,
    }
}
