use idf_im_lib::system_dependencies::{self, PrerequisitesCheckResult};
use log::warn;
use tauri::AppHandle;

use crate::gui::ui::{emit_log_message, MessageLevel};

use super::progress::emit_error;

/// The error to report when the prerequisites check could not be trusted.
pub(super) fn prerequisite_check_error(result: &PrerequisitesCheckResult) -> Option<String> {
    if result.shell_failed {
        Some(rust_i18n::t!("gui.system_dependencies.shell_verification_failed").to_string())
    } else if !result.can_verify {
        Some(
            rust_i18n::t!(
                "gui.system_dependencies.verification_error",
                error = "unknown"
            )
            .to_string(),
        )
    } else {
        None
    }
}

/// Runs the system prerequisites check and returns the missing prerequisites.
/// When the check itself fails, emits an error event titled `check_failed_message`.
pub(super) fn missing_prerequisites(
    app_handle: &AppHandle,
    check_failed_message: &str,
) -> Result<Vec<String>, String> {
    let fail = |error_msg: String| {
        emit_error(
            app_handle,
            check_failed_message.to_string(),
            Some(error_msg.clone()),
            None,
        );
        error_msg
    };

    let result = system_dependencies::check_prerequisites_with_result().map_err(|err| {
        fail(
            rust_i18n::t!(
                "gui.system_dependencies.verification_error",
                error = err.clone()
            )
            .to_string(),
        )
    })?;

    if let Some(error_msg) = prerequisite_check_error(&result) {
        return Err(fail(error_msg));
    }

    Ok(result.missing.into_iter().map(|p| p.to_string()).collect())
}

/// Python sanity check: the user sees each failing check's name + hint; the raw
/// output is only logged.
fn check_python_sanity(app_handle: &AppHandle) -> Result<(), String> {
    let mut python_sane = true;
    for result in &idf_im_lib::python_utils::python_sanity_check(None, true) {
        if !result.passed {
            python_sane = false;
            let name = rust_i18n::t!(result.check.display_key()).to_string();
            let hint =
                rust_i18n::t!(result.check.hint_key_for_os(std::env::consts::OS)).to_string();
            warn!("[FAIL] {}: {}", name, result.message);
            emit_log_message(
                app_handle,
                MessageLevel::Warning,
                format!("{} — {}", name, hint),
            );
        }
    }
    if !python_sane {
        let msg = rust_i18n::t!("gui.offline.python_check_failed").to_string();
        emit_error(
            app_handle,
            msg.clone(),
            Some(rust_i18n::t!("gui.offline.python_not_configured").to_string()),
            None,
        );
        return Err(msg);
    }
    Ok(())
}

/// Non-Windows prerequisite + Python check for the offline flows.
///
/// On Windows the offline pipeline installs git/python FROM the archive, so the
/// archive must be downloaded first. On macOS/Linux it can only *verify* them,
/// and doing that after a multi-GB download is a poor experience — so the simple
/// offline setup runs this up-front and fails fast. Emits events only on failure.
pub(super) fn precheck_posix_prerequisites(app_handle: &AppHandle) -> Result<(), String> {
    let missing = missing_prerequisites(
        app_handle,
        &rust_i18n::t!("gui.offline.prerequisites_check_failed"),
    )?;
    if !missing.is_empty() {
        let msg = rust_i18n::t!(
            "gui.offline.missing_prerequisites",
            items = missing.join(", ")
        )
        .to_string();
        emit_error(
            app_handle,
            rust_i18n::t!("gui.offline.prerequisites_missing").to_string(),
            Some(msg.clone()),
            None,
        );
        return Err(msg);
    }

    check_python_sanity(app_handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(shell_failed: bool, can_verify: bool) -> PrerequisitesCheckResult {
        PrerequisitesCheckResult {
            missing: vec!["git"],
            can_verify,
            shell_failed,
        }
    }

    #[test]
    fn prerequisite_check_error_none_when_verifiable() {
        assert_eq!(prerequisite_check_error(&result(false, true)), None);
    }

    #[test]
    fn prerequisite_check_error_prefers_shell_failure() {
        let expected =
            rust_i18n::t!("gui.system_dependencies.shell_verification_failed").to_string();
        assert_eq!(
            prerequisite_check_error(&result(true, true)),
            Some(expected.clone())
        );
        assert_eq!(
            prerequisite_check_error(&result(true, false)),
            Some(expected)
        );
    }

    #[test]
    fn prerequisite_check_error_reports_unverifiable_system() {
        let expected = rust_i18n::t!(
            "gui.system_dependencies.verification_error",
            error = "unknown"
        )
        .to_string();
        assert_eq!(
            prerequisite_check_error(&result(false, false)),
            Some(expected)
        );
    }
}
