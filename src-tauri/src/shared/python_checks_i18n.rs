use idf_im_lib::python_utils::SanityCheck;
use rust_i18n::t;

/// Returns a translated display name for a Python sanity check variant.
///
/// # Arguments
/// * `check` - The sanity check type to get the display name for
///
/// # Returns
/// A localized string representing the check name
///
/// # Example
///
/// let name = check_display_name(SanityCheck::PythonVersion);
/// Returns "Python Version" in English, "Python 版本" in Chinese, etc.
/// 
pub fn check_display_name(check: SanityCheck) -> String {
    match check {
        SanityCheck::PythonVersion => t!("python.sanitycheck.check.version"),
        SanityCheck::Pip           => t!("python.sanitycheck.check.pip"),
        SanityCheck::Venv          => t!("python.sanitycheck.check.venv"),
        SanityCheck::StdLib        => t!("python.sanitycheck.check.stdlib"),
        SanityCheck::Ctypes        => t!("python.sanitycheck.check.ctypes"),
        SanityCheck::Ssl           => t!("python.sanitycheck.check.ssl"),
    }
    .to_string()
}

/// Returns an OS-aware translated resolution hint for a failed sanity check.
///
/// Different operating systems may have different resolution steps for the
/// same Python issue (e.g., installing packages via apt on Linux vs Homebrew on macOS).
///
/// # Arguments
/// * `check` - The failed sanity check to get a hint for
///
/// # Returns
/// A localized, OS-specific hint string explaining how to fix the issue
///
/// # Example
///
/// let hint = check_hint(SanityCheck::Venv);
/// On Linux: "Run: sudo apt install python3-venv..."
/// On macOS: "Reinstall Python from python.org..."
/// On Windows: "Reinstall Python from python.org..."
///
pub fn check_hint(check: SanityCheck) -> String {
    let os = std::env::consts::OS;
    match (check, os) {
        (SanityCheck::PythonVersion, _)  => t!("python.sanitycheck.hint.version"),
        (SanityCheck::Pip, _)            => t!("python.sanitycheck.hint.pip"),
        (SanityCheck::StdLib, _)         => t!("python.sanitycheck.hint.stdlib"),
        (SanityCheck::Venv, "macos")     => t!("python.sanitycheck.hint.venv.macos"),
        (SanityCheck::Venv, "windows")   => t!("python.sanitycheck.hint.venv.windows"),
        (SanityCheck::Venv, _)           => t!("python.sanitycheck.hint.venv.linux"),
        (SanityCheck::Ctypes, "macos")   => t!("python.sanitycheck.hint.ctypes.macos"),
        (SanityCheck::Ctypes, "windows") => t!("python.sanitycheck.hint.ctypes.windows"),
        (SanityCheck::Ctypes, _)         => t!("python.sanitycheck.hint.ctypes.linux"),
        (SanityCheck::Ssl, "macos")      => t!("python.sanitycheck.hint.ssl.macos"),
        (SanityCheck::Ssl, "windows")    => t!("python.sanitycheck.hint.ssl.windows"),
        (SanityCheck::Ssl, _)            => t!("python.sanitycheck.hint.ssl.linux"),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_display_name_all_variants() {
        // Ensure all variants return non-empty strings
        assert!(!check_display_name(SanityCheck::PythonVersion).is_empty());
        assert!(!check_display_name(SanityCheck::Pip).is_empty());
        assert!(!check_display_name(SanityCheck::Venv).is_empty());
        assert!(!check_display_name(SanityCheck::StdLib).is_empty());
        assert!(!check_display_name(SanityCheck::Ctypes).is_empty());
        assert!(!check_display_name(SanityCheck::Ssl).is_empty());
    }

    #[test]
    fn test_check_hint_returns_content() {
        // Basic smoke test to ensure hints return something
        assert!(!check_hint(SanityCheck::PythonVersion).is_empty());
        assert!(!check_hint(SanityCheck::Ssl).is_empty());
    }
}