use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use crate::{ensure_path, render_template};

/// Creates an executable shell script with the given content and file path.
///
/// # Parameters
///
/// * `file_path`: A string representing the path where the shell script should be created.
/// * `content`: A string representing the content of the shell script.
///
/// # Return
///
/// * `Result<(), String>`: On success, returns `Ok(())`. On error, returns `Err(String)` containing the error message.
fn create_executable_shell_script(file_path: &str, content: &str) -> Result<(), String> {
    if std::env::consts::OS == "windows" {
        unimplemented!("create_executable_shell_script not implemented for Windows")
    } else {
        // Create and write to the file
        let mut file = File::create(file_path).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Set the file as executable (mode 0o755)
            let permissions = PermissionsExt::from_mode(0o755);
            fs::set_permissions(std::path::Path::new(file_path), permissions)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Formats each key-value pair with `format_pair` and joins the results with `separator`.
fn format_env_pairs(
    pairs: &[(String, String)],
    format_pair: impl Fn(&str, &str) -> String,
    separator: &str,
) -> String {
    pairs
        .iter()
        .map(|(key, value)| format_pair(key, value))
        .collect::<Vec<_>>()
        .join(separator)
}

/// Formats a vector of key-value pairs into a bash-compatible format for environment variables.
///
/// # Parameters
///
/// * `pairs` - A reference to a vector of tuples, where each tuple contains a key (String) and a value (String).
///
/// # Return
///
/// * A String representing the formatted environment variable pairs in bash-compatible format.
///   Each pair is enclosed in double quotes and separated by a newline.
///
fn format_bash_env_pairs(pairs: &[(String, String)]) -> String {
    format!(
        "get_env_var_pairs() {{
cat << 'EOF'
{}
EOF
}}",
        format_env_pairs(pairs, |key, value| format!("{}:{}", key, value), "\n")
    )
}

/// Formats a vector of key-value pairs into a fish shell-compatible format for environment variables.
fn format_fish_env_pairs(pairs: &[(String, String)]) -> String {
    // Fish uses semicolon-separated list for easy parsing
    format_env_pairs(pairs, |key, value| format!("{}:{}", key, value), ";")
}

/// Formats a vector of key-value pairs into a PowerShell-compatible format for environment variables.
///
/// # Parameters
///
/// * `pairs`: A reference to a vector of tuples, where each tuple contains a key-value pair.
///
/// # Return
///
/// * A string representing the formatted environment variables in PowerShell-compatible format.
///
fn format_powershell_env_pairs(pairs: &[(String, String)]) -> String {
    let formatted_pairs = format_env_pairs(
        pairs,
        |key, value| format!("    \"{}\" = \"{}\"", key, value),
        "\n",
    );
    format!("$env_var_pairs = @{{\n{}\n}}", formatted_pairs)
}

/// Formats a vector of key-value pairs into a batch file-compatible format for environment variables.
fn format_batch_env_pairs(pairs: &[(String, String)]) -> String {
    format_env_pairs(pairs, |key, value| format!("set {}={}", key, value), "\n")
}

/// Formats env var pairs for printing in batch file -e mode
fn format_batch_env_pairs_print(pairs: &[(String, String)]) -> String {
    format_env_pairs(pairs, |key, value| format!("echo {}={}", key, value), "\n")
}

/// Escapes every space in `input` with `escape`, leaving spaces that are already escaped alone.
fn escape_unescaped_spaces(input: &str, escape: char) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == escape && chars.peek() == Some(&' ') {
            result.push(ch);
            result.push(chars.next().unwrap());
        } else if ch == ' ' {
            result.push(escape);
            result.push(' ');
        } else {
            result.push(ch);
        }
    }

    result
}

pub fn replace_unescaped_spaces_posix(input: &str) -> String {
    escape_unescaped_spaces(input, '\\')
}

pub fn replace_unescaped_spaces_win(input: &str) -> String {
    escape_unescaped_spaces(input, '`')
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScriptKind {
    Activate,
    Deactivate,
}

/// The activation and deactivation templates of one shell.
struct ScriptTemplates {
    activate: &'static str,
    deactivate: &'static str,
}

impl ScriptTemplates {
    fn get(&self, kind: ScriptKind) -> &'static str {
        match kind {
            ScriptKind::Activate => self.activate,
            ScriptKind::Deactivate => self.deactivate,
        }
    }
}

const BASH_TEMPLATES: ScriptTemplates = ScriptTemplates {
    activate: include_str!("../../bash_scripts/activate_idf_template.sh"),
    deactivate: include_str!("../../bash_scripts/deactivate_idf_template.sh"),
};

const FISH_TEMPLATES: ScriptTemplates = ScriptTemplates {
    activate: include_str!("../../bash_scripts/activate_idf_template.fish"),
    deactivate: include_str!("../../bash_scripts/deactivate_idf_template.fish"),
};

const POWERSHELL_TEMPLATES: ScriptTemplates = ScriptTemplates {
    activate: include_str!("../../powershell_scripts/idf_tools_profile_template.ps1"),
    deactivate: include_str!("../../powershell_scripts/idf_tools_profile_deactivate_template.ps1"),
};

const BATCH_TEMPLATES: ScriptTemplates = ScriptTemplates {
    activate: include_str!("../../powershell_scripts/idf_tools_profile_template.bat"),
    deactivate: include_str!("../../powershell_scripts/idf_tools_profile_deactivate_template.bat"),
};

#[derive(Clone, Copy, Debug)]
enum UnixShell {
    Bash,
    Fish,
}

impl UnixShell {
    fn file_name(self, kind: ScriptKind, idf_version: &str) -> String {
        let prefix = match kind {
            ScriptKind::Activate => "activate",
            ScriptKind::Deactivate => "deactivate",
        };
        let extension = match self {
            UnixShell::Bash => "sh",
            UnixShell::Fish => "fish",
        };
        format!("{}_idf_{}.{}", prefix, idf_version, extension)
    }

    fn templates(self) -> &'static ScriptTemplates {
        match self {
            UnixShell::Bash => &BASH_TEMPLATES,
            UnixShell::Fish => &FISH_TEMPLATES,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum WindowsShell {
    PowerShell,
    Batch,
}

impl WindowsShell {
    fn file_name(self, kind: ScriptKind, idf_version: &str) -> String {
        match (self, kind) {
            (WindowsShell::PowerShell, ScriptKind::Activate) => {
                format!("Microsoft.{}.PowerShell_profile.ps1", idf_version)
            }
            (WindowsShell::PowerShell, ScriptKind::Deactivate) => {
                format!("Microsoft.{}.PowerShell_deactivate.ps1", idf_version)
            }
            (WindowsShell::Batch, ScriptKind::Activate) => {
                format!("Microsoft.{}_profile.bat", idf_version)
            }
            (WindowsShell::Batch, ScriptKind::Deactivate) => {
                format!("Microsoft.{}_deactivate.bat", idf_version)
            }
        }
    }

    fn templates(self) -> &'static ScriptTemplates {
        match self {
            WindowsShell::PowerShell => &POWERSHELL_TEMPLATES,
            WindowsShell::Batch => &BATCH_TEMPLATES,
        }
    }
}

/// The installation details every activation/deactivation script is rendered from.
struct ScriptInputs<'a> {
    idf_path: &'a str,
    idf_tools_path: &'a str,
    idf_python_env_path: Option<&'a str>,
    idf_version: &'a str,
    export_paths: Vec<String>,
    env_var_pairs: Vec<(String, String)>,
    python_bin_path: &'a str,
}

/// Builds template variables for Unix shell scripts (bash/fish)
fn build_unix_shell_variables(
    inputs: &ScriptInputs,
    activate_script_path: String,
    deactivate_script_path: Option<String>,
) -> Vec<(&'static str, String)> {
    let env_var_pairs_str = format_bash_env_pairs(&inputs.env_var_pairs);
    let fish_env_var_pairs_str = format_fish_env_pairs(&inputs.env_var_pairs);
    let idf_python_env_path_val = match inputs.idf_python_env_path {
        Some(path) => path.to_string(),
        None => format!("{}/python", inputs.idf_tools_path),
    };

    let addition_to_path = inputs.export_paths.join(":");
    let current_system_path = env::var("PATH").unwrap_or_default();
    let deactivate_script_path = deactivate_script_path.unwrap_or_else(|| {
        format!(
            "{}/deactivate_idf_{}.sh",
            inputs.idf_tools_path, inputs.idf_version
        )
    });

    vec![
        ("env_var_pairs", env_var_pairs_str),
        ("fish_env_var_pairs", fish_env_var_pairs_str),
        ("idf_path", inputs.idf_path.to_string()),
        (
            "idf_path_escaped",
            replace_unescaped_spaces_posix(inputs.idf_path),
        ),
        ("idf_tools_path", inputs.idf_tools_path.to_string()),
        (
            "idf_tools_path_escaped",
            replace_unescaped_spaces_posix(inputs.idf_tools_path),
        ),
        ("idf_version", inputs.idf_version.to_string()),
        ("addition_to_path", addition_to_path),
        ("current_system_path", current_system_path),
        ("python_bin_path", inputs.python_bin_path.to_string()),
        ("idf_python_env_path", idf_python_env_path_val.clone()),
        (
            "idf_python_env_path_escaped",
            replace_unescaped_spaces_posix(&idf_python_env_path_val),
        ),
        ("activate_script_path", activate_script_path.clone()),
        (
            "activate_script_path_escaped",
            replace_unescaped_spaces_posix(&activate_script_path),
        ),
        ("deactivate_script_path", deactivate_script_path.clone()),
        (
            "deactivate_script_path_escaped",
            replace_unescaped_spaces_posix(&deactivate_script_path),
        ),
    ]
}

/// Builds the template variables for PowerShell and batch profiles
fn build_windows_profile_variables(
    shell: WindowsShell,
    inputs: &ScriptInputs,
    activate_script_path: String,
    deactivate_script_path: Option<String>,
) -> Vec<(&'static str, String)> {
    let deactivate_script_path = deactivate_script_path.unwrap_or_else(|| {
        format!(
            "{}\\{}",
            inputs.idf_tools_path,
            shell.file_name(ScriptKind::Deactivate, inputs.idf_version)
        )
    });

    let mut variables = vec![
        ("idf_path", replace_unescaped_spaces_win(inputs.idf_path)),
        ("idf_version", inputs.idf_version.to_string()),
    ];
    match shell {
        WindowsShell::PowerShell => variables.push((
            "env_var_pairs",
            format_powershell_env_pairs(&inputs.env_var_pairs),
        )),
        WindowsShell::Batch => variables.extend([
            (
                "env_var_pairs",
                format_batch_env_pairs(&inputs.env_var_pairs),
            ),
            (
                "env_var_pairs_print",
                format_batch_env_pairs_print(&inputs.env_var_pairs),
            ),
        ]),
    }
    variables.extend([
        (
            "idf_tools_path",
            replace_unescaped_spaces_win(inputs.idf_tools_path),
        ),
        (
            "idf_python_env_path",
            replace_unescaped_spaces_win(
                inputs
                    .idf_python_env_path
                    .unwrap_or(&format!("{}\\python", inputs.idf_tools_path)),
            ),
        ),
        ("add_paths_extras", inputs.export_paths.join(";")),
        ("current_system_path", env::var("PATH").unwrap_or_default()),
        (
            "python_bin_path",
            replace_unescaped_spaces_win(inputs.python_bin_path),
        ),
        ("activate_script_path", activate_script_path),
        ("deactivate_script_path", deactivate_script_path),
    ]);
    variables
}

/// Renders a Unix shell script into `file_path` and marks it executable.
fn write_unix_script(
    (shell, kind): (UnixShell, ScriptKind),
    file_path: &str,
    inputs: ScriptInputs,
) -> Result<(), String> {
    ensure_path(file_path).map_err(|e| e.to_string())?;
    let dir = PathBuf::from(file_path);
    let script_path = |kind| {
        dir.join(shell.file_name(kind, inputs.idf_version))
            .to_string_lossy()
            .into_owned()
    };
    let deactivate_script_path =
        (kind == ScriptKind::Deactivate).then(|| script_path(ScriptKind::Deactivate));
    let variables = build_unix_shell_variables(
        &inputs,
        script_path(ScriptKind::Activate),
        deactivate_script_path,
    );
    let rendered = render_template(shell.templates().get(kind), &variables);

    create_executable_shell_script(&script_path(kind), &rendered)
}

/// Renders a PowerShell or batch profile into `profile_path` and returns the written file's path.
fn write_windows_profile(
    (shell, kind): (WindowsShell, ScriptKind),
    profile_path: &str,
    inputs: ScriptInputs,
) -> Result<String, std::io::Error> {
    let script_path = |kind| {
        format!(
            "{}\\{}",
            profile_path,
            shell.file_name(kind, inputs.idf_version)
        )
    };
    let deactivate_script_path =
        (kind == ScriptKind::Deactivate).then(|| script_path(ScriptKind::Deactivate));
    let variables = build_windows_profile_variables(
        shell,
        &inputs,
        script_path(ScriptKind::Activate),
        deactivate_script_path,
    );
    render_and_write_profile(
        profile_path,
        shell.templates().get(kind),
        variables,
        &shell.file_name(kind, inputs.idf_version),
    )
}

/// Renders a profile template and writes it to a file
fn render_and_write_profile(
    profile_path: &str,
    template_content: &str,
    variables: Vec<(&str, String)>,
    filename: &str,
) -> Result<String, std::io::Error> {
    ensure_path(profile_path).expect("Unable to create directory");

    let mut rendered = render_template(template_content, &variables);

    if std::env::consts::OS == "windows" {
        rendered = rendered.replace("\r\n", "\n").replace("\n", "\r\n");
    }

    let mut filepath = PathBuf::from(profile_path);
    filepath.push(filename);
    fs::write(&filepath, rendered).expect("Unable to write file");
    Ok(filepath.display().to_string())
}

/// Declares a script generator taking the argument list shared by every
/// activation/deactivation script, delegating to `$writer` for `$script`.
macro_rules! script_generator {
    ($(#[$attr:meta])* $vis:vis fn $name:ident -> $ret:ty = $writer:ident($script:expr)) => {
        $(#[$attr])*
        #[allow(clippy::too_many_arguments)]
        $vis fn $name(
            file_path: &str,
            idf_path: &str,
            idf_tools_path: &str,
            idf_python_env_path: Option<&str>,
            idf_version: &str,
            export_paths: Vec<String>,
            env_var_pairs: Vec<(String, String)>,
            python_bin_path: &str,
        ) -> $ret {
            $writer(
                $script,
                file_path,
                ScriptInputs {
                    idf_path,
                    idf_tools_path,
                    idf_python_env_path,
                    idf_version,
                    export_paths,
                    env_var_pairs,
                    python_bin_path,
                },
            )
        }
    };
}

script_generator! {
    /// Creates a bash shell activation script for the ESP-IDF toolchain.
    ///
    /// # Parameters
    ///
    /// * `file_path`: A string representing the path where the activation script should be created.
    /// * `idf_path`: A string representing the path to the ESP-IDF installation.
    /// * `idf_tools_path`: A string representing the path to the ESP-IDF tools installation.
    /// * `idf_version`: A string representing the version of the ESP-IDF toolchain.
    /// * `export_paths`: A vector of strings representing additional paths to be added to the shell's PATH environment variable.
    ///
    /// # Return
    ///
    /// * `Result<(), String>`: On success, returns `Ok(())`. On error, returns `Err(String)` containing the error message.
    pub fn create_activation_shell_script -> Result<(), String> =
        write_unix_script((UnixShell::Bash, ScriptKind::Activate))
}

script_generator! {
    /// Creates a fish shell activation script for the ESP-IDF toolchain.
    pub fn create_fish_script -> Result<(), String> =
        write_unix_script((UnixShell::Fish, ScriptKind::Activate))
}

script_generator! {
    /// Creates a bash shell deactivation script for the ESP-IDF toolchain.
    ///
    /// The deactivation script is the inverse of `create_activation_shell_script`:
    /// it unsets the same env vars, strips the IDF PATH prefixes, drops the IDF
    /// shell functions/completions, and runs the Python venv's `deactivate`.
    pub fn create_deactivation_shell_script -> Result<(), String> =
        write_unix_script((UnixShell::Bash, ScriptKind::Deactivate))
}

script_generator! {
    /// Creates a fish shell deactivation script for the ESP-IDF toolchain.
    pub fn create_deactivation_fish_script -> Result<(), String> =
        write_unix_script((UnixShell::Fish, ScriptKind::Deactivate))
}

script_generator! {
    /// Creates a PowerShell profile script for the ESP-IDF tools.
    ///
    /// # Parameters
    ///
    /// * `file_path` - A string representing the path where the PowerShell profile script should be created.
    /// * `idf_path` - A string representing the path to the ESP-IDF repository.
    /// * `idf_tools_path` - A string representing the path to the ESP-IDF tools directory.
    ///
    /// # Returns
    ///
    /// * `Result<String, std::io::Error>` - On success, returns the path to the created PowerShell profile script.
    ///   On error, returns an `std::io::Error` indicating the cause of the error.
    pub(crate) fn create_powershell_profile -> Result<String, std::io::Error> =
        write_windows_profile((WindowsShell::PowerShell, ScriptKind::Activate))
}

script_generator! {
    /// Creates a PowerShell deactivation script for the ESP-IDF tools.
    ///
    /// Mirrors the matching `Microsoft.<ver>.PowerShell_profile.ps1`:
    /// unsets the same env vars, strips toolchain PATH prefixes, removes
    /// the IDF functions/aliases, and deactivates the Python venv.
    pub(crate) fn create_powershell_deactivate_profile -> Result<String, std::io::Error> =
        write_windows_profile((WindowsShell::PowerShell, ScriptKind::Deactivate))
}

script_generator! {
    /// Creates a batch file profile script for the ESP-IDF tools.
    ///
    /// # Parameters
    ///
    /// * `file_path` - A string representing the path where the batch profile script should be created.
    /// * `idf_path` - A string representing the path to the ESP-IDF repository.
    /// * `idf_tools_path` - A string representing the path to the ESP-IDF tools directory.
    ///
    /// # Returns
    ///
    /// * `Result<String, std::io::Error>` - On success, returns the path to the created batch profile script.
    ///   On error, returns an `std::io::Error` indicating the cause of the error.
    pub(crate) fn create_batch_profile -> Result<String, std::io::Error> =
        write_windows_profile((WindowsShell::Batch, ScriptKind::Activate))
}

script_generator! {
    /// Creates a CMD batch deactivation script for the ESP-IDF tools.
    ///
    /// Mirrors the matching `Microsoft.<ver>_profile.bat`: unsets the same
    /// env vars, strips toolchain PATH prefixes, removes the IDF doskey
    /// aliases, and deactivates the Python venv.
    pub(crate) fn create_batch_deactivate_profile -> Result<String, std::io::Error> =
        write_windows_profile((WindowsShell::Batch, ScriptKind::Deactivate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_pairs() -> Vec<(String, String)> {
        vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "x y".to_string()),
        ]
    }

    #[test]
    fn test_format_bash_env_pairs() {
        assert_eq!(
            format_bash_env_pairs(&sample_pairs()),
            "get_env_var_pairs() {\ncat << 'EOF'\nA:1\nB:x y\nEOF\n}"
        );
    }

    #[test]
    fn test_format_fish_env_pairs() {
        assert_eq!(format_fish_env_pairs(&sample_pairs()), "A:1;B:x y");
        assert_eq!(format_fish_env_pairs(&[]), "");
    }

    #[test]
    fn test_format_powershell_env_pairs() {
        assert_eq!(
            format_powershell_env_pairs(&sample_pairs()),
            "$env_var_pairs = @{\n    \"A\" = \"1\"\n    \"B\" = \"x y\"\n}"
        );
        assert_eq!(format_powershell_env_pairs(&[]), "$env_var_pairs = @{\n\n}");
    }

    #[test]
    fn test_format_batch_env_pairs() {
        assert_eq!(
            format_batch_env_pairs(&sample_pairs()),
            "set A=1\nset B=x y"
        );
        assert_eq!(
            format_batch_env_pairs_print(&sample_pairs()),
            "echo A=1\necho B=x y"
        );
    }

    #[test]
    fn test_replace_unescaped_spaces_posix() {
        assert_eq!(replace_unescaped_spaces_posix("/a b/c"), "/a\\ b/c");
        assert_eq!(replace_unescaped_spaces_posix("/a\\ b c"), "/a\\ b\\ c");
        assert_eq!(replace_unescaped_spaces_posix("a  b"), "a\\ \\ b");
        assert_eq!(replace_unescaped_spaces_posix("a`b c"), "a`b\\ c");
        assert_eq!(replace_unescaped_spaces_posix(""), "");
    }

    #[test]
    fn test_replace_unescaped_spaces_win() {
        assert_eq!(replace_unescaped_spaces_win("C:\\a b"), "C:\\a` b");
        assert_eq!(replace_unescaped_spaces_win("a` b c"), "a` b` c");
        assert_eq!(replace_unescaped_spaces_win("a\\ b"), "a\\` b");
        assert_eq!(replace_unescaped_spaces_win("trailing`"), "trailing`");
    }

    #[test]
    fn test_unix_shell_file_names() {
        assert_eq!(
            UnixShell::Bash.file_name(ScriptKind::Activate, "v5.3"),
            "activate_idf_v5.3.sh"
        );
        assert_eq!(
            UnixShell::Bash.file_name(ScriptKind::Deactivate, "v5.3"),
            "deactivate_idf_v5.3.sh"
        );
        assert_eq!(
            UnixShell::Fish.file_name(ScriptKind::Activate, "v5.3"),
            "activate_idf_v5.3.fish"
        );
        assert_eq!(
            UnixShell::Fish.file_name(ScriptKind::Deactivate, "v5.3"),
            "deactivate_idf_v5.3.fish"
        );
    }

    #[test]
    fn test_windows_shell_file_names() {
        assert_eq!(
            WindowsShell::PowerShell.file_name(ScriptKind::Activate, "v5.3"),
            "Microsoft.v5.3.PowerShell_profile.ps1"
        );
        assert_eq!(
            WindowsShell::PowerShell.file_name(ScriptKind::Deactivate, "v5.3"),
            "Microsoft.v5.3.PowerShell_deactivate.ps1"
        );
        assert_eq!(
            WindowsShell::Batch.file_name(ScriptKind::Activate, "v5.3"),
            "Microsoft.v5.3_profile.bat"
        );
        assert_eq!(
            WindowsShell::Batch.file_name(ScriptKind::Deactivate, "v5.3"),
            "Microsoft.v5.3_deactivate.bat"
        );
    }

    fn sample_inputs() -> ScriptInputs<'static> {
        ScriptInputs {
            idf_path: "/idf",
            idf_tools_path: "/my tools",
            idf_python_env_path: None,
            idf_version: "v5.3",
            export_paths: vec!["/a".to_string(), "/b".to_string()],
            env_var_pairs: sample_pairs(),
            python_bin_path: "/py",
        }
    }

    fn variable<'a>(vars: &'a [(&str, String)], key: &str) -> &'a str {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("missing variable {key}"))
    }

    #[test]
    fn test_build_unix_shell_variables_defaults() {
        let vars = build_unix_shell_variables(&sample_inputs(), "/s/activate".to_string(), None);
        assert_eq!(
            variable(&vars, "deactivate_script_path"),
            "/my tools/deactivate_idf_v5.3.sh"
        );
        assert_eq!(
            variable(&vars, "deactivate_script_path_escaped"),
            "/my\\ tools/deactivate_idf_v5.3.sh"
        );
        assert_eq!(variable(&vars, "idf_python_env_path"), "/my tools/python");
        assert_eq!(variable(&vars, "addition_to_path"), "/a:/b");
        assert_eq!(variable(&vars, "activate_script_path"), "/s/activate");
    }

    #[test]
    fn test_build_windows_profile_variables_per_shell() {
        let ps = build_windows_profile_variables(
            WindowsShell::PowerShell,
            &sample_inputs(),
            "P\\act".to_string(),
            None,
        );
        assert!(!ps.iter().any(|(k, _)| *k == "env_var_pairs_print"));
        assert_eq!(
            variable(&ps, "deactivate_script_path"),
            "/my tools\\Microsoft.v5.3.PowerShell_deactivate.ps1"
        );
        assert_eq!(variable(&ps, "idf_python_env_path"), "/my` tools\\python");
        assert_eq!(variable(&ps, "add_paths_extras"), "/a;/b");

        let bat = build_windows_profile_variables(
            WindowsShell::Batch,
            &sample_inputs(),
            "P\\act".to_string(),
            Some("P\\deact".to_string()),
        );
        let keys: Vec<&str> = bat.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            &keys[..4],
            &[
                "idf_path",
                "idf_version",
                "env_var_pairs",
                "env_var_pairs_print"
            ]
        );
        assert_eq!(variable(&bat, "deactivate_script_path"), "P\\deact");
    }

    /// Renders the POSIX activation + deactivation scripts and checks the
    /// deactivate script's source for the markers a successful deactivation
    /// requires (no leftover placeholders, the matching activate path in the
    /// help text, and the PATH-stripping logic).
    #[test]
    fn test_posix_activation_and_deactivation_scripts_render() {
        let tmp = TempDir::new().unwrap();
        let script_dir = tmp.path().to_str().unwrap().to_string();

        let export_paths = vec![
            "/opt/esp/tools/xtensa-esp-elf".to_string(),
            "/opt/esp/tools/riscv32-esp-elf".to_string(),
        ];
        let env_vars = vec![
            ("IDF_TOOLS_PATH".to_string(), "/opt/esp/tools".to_string()),
            (
                "IDF_COMPONENT_LOCAL_STORAGE_URL".to_string(),
                "file:///opt/esp/components".to_string(),
            ),
        ];

        create_activation_shell_script(
            &script_dir,
            "/opt/esp/esp-idf",
            "/opt/esp/tools",
            Some("/opt/esp/tools/python"),
            "v5.3.2",
            export_paths.clone(),
            env_vars.clone(),
            "/opt/esp/tools/python/bin/python",
        )
        .expect("create activation shell script");

        create_deactivation_shell_script(
            &script_dir,
            "/opt/esp/esp-idf",
            "/opt/esp/tools",
            Some("/opt/esp/tools/python"),
            "v5.3.2",
            export_paths.clone(),
            env_vars.clone(),
            "/opt/esp/tools/python/bin/python",
        )
        .expect("create deactivation shell script");

        let activate_path = PathBuf::from(&script_dir).join("activate_idf_v5.3.2.sh");
        let deactivate_path = PathBuf::from(&script_dir).join("deactivate_idf_v5.3.2.sh");

        let activate = fs::read_to_string(&activate_path).expect("read activate");
        let deactivate = fs::read_to_string(&deactivate_path).expect("read deactivate");

        #[cfg(not(target_os = "windows"))]
        {
            let output = std::process::Command::new("sh")
                .args([
                    "-c",
                    "unset IDF_TOOLS_PATH; set -C; . \"$1\" >>/dev/null 2>&1; [ \"$IDF_TOOLS_PATH\" = \"/opt/esp/tools\" ]",
                    "sh",
                ])
                .arg(&activate_path)
                .output()
                .expect("run activation script with noclobber");
            assert!(
                output.status.success(),
                "activation with noclobber failed (status: {}): stdout: {} stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // No leftover placeholders
        assert!(
            !activate.contains("{{"),
            "activate has placeholders: {}",
            activate
        );
        assert!(
            !deactivate.contains("{{"),
            "deactivate has placeholders: {}",
            deactivate
        );

        // The deactivate script must reference the activate path so the
        // help text points the user at the script being undone.
        assert!(
            deactivate.contains("activate_idf_v5.3.2.sh"),
            "deactivate script does not reference activate script:\n{}",
            deactivate
        );

        // The deactivate script must export the addition_to_path so the
        // PATH stripping works. The activation sets the same value as
        // `addition_to_path`; we check it's present in both.
        assert!(activate.contains("/opt/esp/tools/xtensa-esp-elf"));
        assert!(deactivate.contains("/opt/esp/tools/xtensa-esp-elf"));

        // The deactivate script must unset the fixed IDF env vars and the
        // tool vars. We check by looking for the variable name in a
        // for-loop unsetting block. The exact spelling is flexible:
        // both `unset ESP_IDF_VERSION` (in a literal list) and the
        // `for _v in ESP_IDF_VERSION ...; do ...; unset $_v; done`
        // pattern (used in the template) qualify.
        for var in [
            "ESP_IDF_VERSION",
            "IDF_PATH",
            "IDF_TOOLS_PATH",
            "IDF_PYTHON_ENV_PATH",
            "IDF_COMPONENT_LOCAL_STORAGE_URL",
            "ESP_ROM_ELF_DIR",
            "OPENOCD_SCRIPTS",
        ] {
            let found = deactivate.contains(&format!("unset {var}"))
                || deactivate.contains(&"unset $_v".to_string()) && deactivate.contains(var);
            assert!(
                found,
                "deactivate script does not unset {var}:\n{deactivate}"
            );
        }

        // Tool vars written by the activate script as a heredoc must be
        // unset by the deactivate script too. We accept either the
        // safe quoted form (`unset "$_key"`) or the older unquoted
        // form (`unset $_key`) so this check stays valid across the
        // regression.
        assert!(
            deactivate.contains("unset \"$_key\"")
                || deactivate.contains("unset $_key")
                || deactivate.contains("unset $key"),
            "deactivate script does not unset tool vars:\n{deactivate}"
        );

        // The deactivate script should refuse to run if executed (not sourced).
        assert!(deactivate.contains("This script should be sourced"));

        // Regression: the tool-vars unset block must use a heredoc
        // (`<< EOF`) instead of mktemp+printf. The heredoc keeps the
        // while loop in the parent shell, so `unset` actually affects
        // the caller's environment. The mktemp approach can also fail
        // when /tmp is full or read-only.
        assert!(
            deactivate.contains("<< EOF"),
            "POSIX deactivate must use a heredoc to feed ENV_VAR_PAIRS. Got:\n{deactivate}"
        );
        // Look for the actual mktemp command (`$(mktemp)`), not the
        // word in a comment.
        assert!(
            !deactivate.contains("$(mktemp)"),
            "POSIX deactivate must not call mktemp. Got:\n{deactivate}"
        );
        assert!(
            !deactivate.contains("`mktemp`"),
            "POSIX deactivate must not call mktemp. Got:\n{deactivate}"
        );

        // Regression: the tool-vars unset must use the safe `unset "$_key"`
        // form, not `eval "unset $_key"`. The eval form is unsafe if
        // a tool var name ever contains a metacharacter.
        // (The fixed-var block above uses `eval "unset $_v"` on a
        // hardcoded list, which is safe — only the dynamic `$_key`
        // path needed to switch to the safe form.)
        assert!(
            deactivate.contains("unset \"$_key\""),
            "POSIX deactivate must use `unset \"$_key\"`. Got:\n{deactivate}"
        );
        // Look for the actual `eval "unset $_key"` invocation. The
        // script may legitimately mention the old pattern in a comment
        // explaining why it was changed.
        assert!(
            !deactivate.contains("eval \"unset $_key\""),
            "POSIX deactivate must not use `eval \"unset $_key\"`. Got:\n{deactivate}"
        );
    }

    /// Same as above for fish, on POSIX only.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_fish_activation_and_deactivation_scripts_render() {
        let tmp = TempDir::new().unwrap();
        let script_dir = tmp.path().to_str().unwrap().to_string();

        let export_paths = vec!["/opt/esp/tools/xtensa-esp-elf".to_string()];
        let env_vars = vec![("IDF_TOOLS_PATH".to_string(), "/opt/esp/tools".to_string())];

        create_fish_script(
            &script_dir,
            "/opt/esp/esp-idf",
            "/opt/esp/tools",
            Some("/opt/esp/tools/python"),
            "v5.3.2",
            export_paths.clone(),
            env_vars.clone(),
            "/opt/esp/tools/python/bin/python",
        )
        .expect("create fish activate");

        create_deactivation_fish_script(
            &script_dir,
            "/opt/esp/esp-idf",
            "/opt/esp/tools",
            Some("/opt/esp/tools/python"),
            "v5.3.2",
            export_paths,
            env_vars,
            "/opt/esp/tools/python/bin/python",
        )
        .expect("create fish deactivate");

        let activate =
            fs::read_to_string(PathBuf::from(&script_dir).join("activate_idf_v5.3.2.fish"))
                .expect("read fish activate");
        let deactivate =
            fs::read_to_string(PathBuf::from(&script_dir).join("deactivate_idf_v5.3.2.fish"))
                .expect("read fish deactivate");

        assert!(!activate.contains("{{"));
        assert!(!deactivate.contains("{{"));
        assert!(deactivate.contains("activate_idf_v5.3.2.fish"));
        assert!(deactivate.contains("/opt/esp/tools/xtensa-esp-elf"));
        assert!(deactivate.contains("ESP_IDF_VERSION"));

        // Regression: the -h branch must `return 0`, not `exit 0`.
        // In fish, `exit` inside a sourced script terminates the
        // entire shell session, which would unexpectedly close the
        // user's terminal.
        assert!(
            deactivate.contains("return 0"),
            "Fish deactivate -h branch must use `return 0`. Got:\n{deactivate}"
        );
        assert!(
            !deactivate.contains("exit 0"),
            "Fish deactivate must not use `exit 0` (it would close the user's terminal). Got:\n{deactivate}"
        );
    }

    /// Smoke-tests the Windows profile renderers. We can call the
    /// internal helpers directly on any OS; the templates are platform-
    /// agnostic until render_and_write_profile rewrites line endings.
    #[test]
    fn test_windows_profile_renderers_smoke() {
        let tmp = TempDir::new().unwrap();
        let script_dir = tmp.path().to_str().unwrap().to_string();

        let export_paths = vec!["C:/esp/tools/xtensa-esp-elf/bin".to_string()];
        let env_vars = vec![("IDF_TOOLS_PATH".to_string(), "C:/esp/tools".to_string())];

        // create_powershell_profile / create_batch_profile are private,
        // but we can call them within the same module's tests.
        let ps = create_powershell_profile(
            &script_dir,
            "C:/esp/esp-idf",
            "C:/esp/tools",
            Some("C:/esp/tools/python"),
            "v5.3.2",
            export_paths.clone(),
            env_vars.clone(),
            "C:/esp/tools/python/python.exe",
        )
        .expect("create PS profile");
        let ps_deact = create_powershell_deactivate_profile(
            &script_dir,
            "C:/esp/esp-idf",
            "C:/esp/tools",
            Some("C:/esp/tools/python"),
            "v5.3.2",
            export_paths.clone(),
            env_vars.clone(),
            "C:/esp/tools/python/python.exe",
        )
        .expect("create PS deactivate");
        let bat = create_batch_profile(
            &script_dir,
            "C:/esp/esp-idf",
            "C:/esp/tools",
            Some("C:/esp/tools/python"),
            "v5.3.2",
            export_paths.clone(),
            env_vars.clone(),
            "C:/esp/tools/python/python.exe",
        )
        .expect("create batch profile");
        let bat_deact = create_batch_deactivate_profile(
            &script_dir,
            "C:/esp/esp-idf",
            "C:/esp/tools",
            Some("C:/esp/tools/python"),
            "v5.3.2",
            export_paths,
            env_vars,
            "C:/esp/tools/python/python.exe",
        )
        .expect("create batch deactivate");

        for (path, label) in [
            (&ps, "powershell activate"),
            (&ps_deact, "powershell deactivate"),
            (&bat, "batch activate"),
            (&bat_deact, "batch deactivate"),
        ] {
            let content = fs::read_to_string(path).expect(label);
            assert!(!content.contains("{{"), "{label} has placeholders");
        }

        let ps_deact_content = fs::read_to_string(&ps_deact).expect("read PS deact");
        assert!(
            ps_deact_content.contains("Microsoft.v5.3.2.PowerShell_deactivate.ps1"),
            "PS deactivate did not contain its own filename. Got:\n{ps_deact_content}"
        );
        // The help text should describe what the script does.
        assert!(ps_deact_content.contains("Removes the ESP-IDF environment"));

        let bat_deact_content = fs::read_to_string(&bat_deact).expect("read BAT deact");
        assert!(
            bat_deact_content.contains("Microsoft.v5.3.2_deactivate.bat"),
            "BAT deactivate did not contain its own filename. Got:\n{bat_deact_content}"
        );
        assert!(bat_deact_content.contains("Removed IDF toolchain entries from PATH"));

        // Regression: the BAT deactivation script must use `delims== `
        // (both = and space) with tokens=1,2 so that `set VAR=val` is
        // parsed as K=`set`, L=`VAR`. Using `delims==` alone would
        // leave K holding `set VAR` and silently fail to unset anything.
        assert!(
            bat_deact_content.contains("delims== "),
            "BAT deactivate must use `delims== ` (both = and space). Got:\n{bat_deact_content}"
        );
        assert!(
            bat_deact_content.contains("tokens=1,2"),
            "BAT deactivate must use `tokens=1,2`. Got:\n{bat_deact_content}"
        );
        // And it must NOT use the broken `tokens=1,*` form.
        assert!(
            !bat_deact_content.contains("tokens=1,*"),
            "BAT deactivate still contains the broken `tokens=1,*` form. Got:\n{bat_deact_content}"
        );

        // Regression: the BAT deactivate must not call `doskey /reinstall`
        // (it would clobber every doskey macro in the user's session,
        // not just the IDF ones).
        assert!(
            !bat_deact_content.contains("doskey /reinstall"),
            "BAT deactivate must not call `doskey /reinstall`. Got:\n{bat_deact_content}"
        );

        // Regression: the BAT deactivate must not leave a stray
        // `endlocal` at the very end (it could prematurely end a
        // setlocal in a calling script) and must clean up
        // ACTIVATE_SCRIPT at the `:end` label.
        let last_meaningful = bat_deact_content
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("REM "))
            .unwrap_or("");
        assert!(
            !last_meaningful.trim().eq_ignore_ascii_case("endlocal"),
            "BAT deactivate must not have a trailing `endlocal`. Got:\n{bat_deact_content}"
        );
        assert!(
            bat_deact_content.contains("set \"ACTIVATE_SCRIPT=\""),
            "BAT deactivate must clear ACTIVATE_SCRIPT at the :end label. Got:\n{bat_deact_content}"
        );
    }

    const GOLDEN_VERSION: &str = "v5.3";

    fn golden_export_paths() -> Vec<String> {
        vec![
            "/opt/esp/tools/xtensa bin".to_string(),
            "/opt/esp/tools/riscv/bin".to_string(),
        ]
    }

    fn golden_env_vars() -> Vec<(String, String)> {
        vec![
            ("IDF_TOOLS_PATH".to_string(), "/opt/esp/tools".to_string()),
            (
                "OPENOCD_SCRIPTS".to_string(),
                "/opt/esp/openocd scripts".to_string(),
            ),
        ]
    }

    /// Returns a temp dir plus a script directory inside it whose name contains
    /// a space, so the escaping of generated paths is exercised.
    fn golden_script_dir() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().to_str().unwrap().to_string();
        assert!(!base.contains(' '), "temp dir must not contain spaces");
        (tmp, format!("{}/golden scripts", base))
    }

    fn expected_unix_vars(
        python_env: &str,
        python_env_escaped: &str,
        activate: String,
        deactivate: String,
    ) -> Vec<(&'static str, String)> {
        let escape = |p: &str| p.replace(' ', "\\ ");
        vec![
            (
                "env_var_pairs",
                "get_env_var_pairs() {\ncat << 'EOF'\nIDF_TOOLS_PATH:/opt/esp/tools\nOPENOCD_SCRIPTS:/opt/esp/openocd scripts\nEOF\n}".to_string(),
            ),
            (
                "fish_env_var_pairs",
                "IDF_TOOLS_PATH:/opt/esp/tools;OPENOCD_SCRIPTS:/opt/esp/openocd scripts".to_string(),
            ),
            ("idf_path", "/opt/esp idf/v5.3".to_string()),
            ("idf_path_escaped", "/opt/esp\\ idf/v5.3".to_string()),
            ("idf_tools_path", "/opt/esp tools".to_string()),
            ("idf_tools_path_escaped", "/opt/esp\\ tools".to_string()),
            ("idf_version", GOLDEN_VERSION.to_string()),
            (
                "addition_to_path",
                "/opt/esp/tools/xtensa bin:/opt/esp/tools/riscv/bin".to_string(),
            ),
            ("current_system_path", env::var("PATH").unwrap_or_default()),
            ("python_bin_path", "/opt/esp tools/python/bin/python".to_string()),
            ("idf_python_env_path", python_env.to_string()),
            ("idf_python_env_path_escaped", python_env_escaped.to_string()),
            ("activate_script_path", activate.clone()),
            ("activate_script_path_escaped", escape(&activate)),
            ("deactivate_script_path", deactivate.clone()),
            ("deactivate_script_path_escaped", escape(&deactivate)),
        ]
    }

    type UnixGenerator = fn(
        &str,
        &str,
        &str,
        Option<&str>,
        &str,
        Vec<String>,
        Vec<(String, String)>,
        &str,
    ) -> Result<(), String>;

    fn assert_unix_golden(
        generator: UnixGenerator,
        python_env: Option<&str>,
        file_name: &str,
        template: &str,
        activate_name: &str,
        deactivate_path: impl Fn(&str) -> String,
    ) {
        let (_tmp, dir) = golden_script_dir();
        generator(
            &dir,
            "/opt/esp idf/v5.3",
            "/opt/esp tools",
            python_env,
            GOLDEN_VERSION,
            golden_export_paths(),
            golden_env_vars(),
            "/opt/esp tools/python/bin/python",
        )
        .expect("generate script");

        let (env_path, env_path_escaped) = match python_env {
            Some(p) => (p.to_string(), p.replace(' ', "\\ ")),
            None => (
                "/opt/esp tools/python".to_string(),
                "/opt/esp\\ tools/python".to_string(),
            ),
        };
        let vars = expected_unix_vars(
            &env_path,
            &env_path_escaped,
            format!("{}/{}", dir, activate_name),
            deactivate_path(&dir),
        );
        let expected = render_template(template, &vars);
        let written_path = PathBuf::from(&dir).join(file_name);
        let actual = fs::read_to_string(&written_path).expect("read generated script");
        assert_eq!(actual, expected, "golden mismatch for {}", file_name);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&written_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    #[test]
    fn golden_create_activation_shell_script() {
        assert_unix_golden(
            create_activation_shell_script,
            None,
            "activate_idf_v5.3.sh",
            include_str!("../../bash_scripts/activate_idf_template.sh"),
            "activate_idf_v5.3.sh",
            |_| "/opt/esp tools/deactivate_idf_v5.3.sh".to_string(),
        );
    }

    #[test]
    fn golden_create_fish_script() {
        assert_unix_golden(
            create_fish_script,
            Some("/opt/esp tools/py env"),
            "activate_idf_v5.3.fish",
            include_str!("../../bash_scripts/activate_idf_template.fish"),
            "activate_idf_v5.3.fish",
            |_| "/opt/esp tools/deactivate_idf_v5.3.sh".to_string(),
        );
    }

    #[test]
    fn golden_create_deactivation_shell_script() {
        assert_unix_golden(
            create_deactivation_shell_script,
            Some("/opt/esp tools/py env"),
            "deactivate_idf_v5.3.sh",
            include_str!("../../bash_scripts/deactivate_idf_template.sh"),
            "activate_idf_v5.3.sh",
            |dir| format!("{}/deactivate_idf_v5.3.sh", dir),
        );
    }

    #[test]
    fn golden_create_deactivation_fish_script() {
        assert_unix_golden(
            create_deactivation_fish_script,
            None,
            "deactivate_idf_v5.3.fish",
            include_str!("../../bash_scripts/deactivate_idf_template.fish"),
            "activate_idf_v5.3.fish",
            |dir| format!("{}/deactivate_idf_v5.3.fish", dir),
        );
    }

    type WindowsGenerator = fn(
        &str,
        &str,
        &str,
        Option<&str>,
        &str,
        Vec<String>,
        Vec<(String, String)>,
        &str,
    ) -> Result<String, std::io::Error>;

    struct WindowsGolden<'a> {
        generator: WindowsGenerator,
        python_env: Option<&'a str>,
        python_env_escaped: &'a str,
        file_name: &'a str,
        template: &'a str,
        batch: bool,
        activate_name: &'a str,
        deactivate_path: fn(&str) -> String,
    }

    fn assert_windows_golden(case: WindowsGolden) {
        let (_tmp, dir) = golden_script_dir();
        let written = (case.generator)(
            &dir,
            "C:\\esp idf\\v5.3",
            "C:\\esp tools",
            case.python_env,
            GOLDEN_VERSION,
            golden_export_paths(),
            golden_env_vars(),
            "C:\\esp tools\\python\\python.exe",
        )
        .expect("generate profile");
        let mut expected_path = PathBuf::from(&dir);
        expected_path.push(case.file_name);
        assert_eq!(written, expected_path.display().to_string());

        let mut vars: Vec<(&str, String)> = vec![
            ("idf_path", "C:\\esp` idf\\v5.3".to_string()),
            ("idf_version", GOLDEN_VERSION.to_string()),
        ];
        if case.batch {
            vars.push((
                "env_var_pairs",
                "set IDF_TOOLS_PATH=/opt/esp/tools\nset OPENOCD_SCRIPTS=/opt/esp/openocd scripts"
                    .to_string(),
            ));
            vars.push((
                "env_var_pairs_print",
                "echo IDF_TOOLS_PATH=/opt/esp/tools\necho OPENOCD_SCRIPTS=/opt/esp/openocd scripts"
                    .to_string(),
            ));
        } else {
            vars.push((
                "env_var_pairs",
                "$env_var_pairs = @{\n    \"IDF_TOOLS_PATH\" = \"/opt/esp/tools\"\n    \"OPENOCD_SCRIPTS\" = \"/opt/esp/openocd scripts\"\n}".to_string(),
            ));
        }
        vars.extend([
            ("idf_tools_path", "C:\\esp` tools".to_string()),
            ("idf_python_env_path", case.python_env_escaped.to_string()),
            (
                "add_paths_extras",
                "/opt/esp/tools/xtensa bin;/opt/esp/tools/riscv/bin".to_string(),
            ),
            ("current_system_path", env::var("PATH").unwrap_or_default()),
            (
                "python_bin_path",
                "C:\\esp` tools\\python\\python.exe".to_string(),
            ),
            (
                "activate_script_path",
                format!("{}\\{}", dir, case.activate_name),
            ),
            ("deactivate_script_path", (case.deactivate_path)(&dir)),
        ]);
        let mut expected = render_template(case.template, &vars);
        if std::env::consts::OS == "windows" {
            expected = expected.replace("\r\n", "\n").replace("\n", "\r\n");
        }
        let actual = fs::read_to_string(&written).expect("read generated profile");
        assert_eq!(actual, expected, "golden mismatch for {}", case.file_name);
    }

    #[test]
    fn golden_create_powershell_profile() {
        assert_windows_golden(WindowsGolden {
            generator: create_powershell_profile,
            python_env: None,
            python_env_escaped: "C:\\esp` tools\\python",
            file_name: "Microsoft.v5.3.PowerShell_profile.ps1",
            template: include_str!("../../powershell_scripts/idf_tools_profile_template.ps1"),
            batch: false,
            activate_name: "Microsoft.v5.3.PowerShell_profile.ps1",
            deactivate_path: |_| "C:\\esp tools\\Microsoft.v5.3.PowerShell_deactivate.ps1".into(),
        });
    }

    #[test]
    fn golden_create_powershell_deactivate_profile() {
        assert_windows_golden(WindowsGolden {
            generator: create_powershell_deactivate_profile,
            python_env: Some("C:\\esp tools\\py env"),
            python_env_escaped: "C:\\esp` tools\\py` env",
            file_name: "Microsoft.v5.3.PowerShell_deactivate.ps1",
            template: include_str!(
                "../../powershell_scripts/idf_tools_profile_deactivate_template.ps1"
            ),
            batch: false,
            activate_name: "Microsoft.v5.3.PowerShell_profile.ps1",
            deactivate_path: |dir| format!("{}\\Microsoft.v5.3.PowerShell_deactivate.ps1", dir),
        });
    }

    #[test]
    fn golden_create_batch_profile() {
        assert_windows_golden(WindowsGolden {
            generator: create_batch_profile,
            python_env: Some("C:\\esp tools\\py env"),
            python_env_escaped: "C:\\esp` tools\\py` env",
            file_name: "Microsoft.v5.3_profile.bat",
            template: include_str!("../../powershell_scripts/idf_tools_profile_template.bat"),
            batch: true,
            activate_name: "Microsoft.v5.3_profile.bat",
            deactivate_path: |_| "C:\\esp tools\\Microsoft.v5.3_deactivate.bat".into(),
        });
    }

    #[test]
    fn golden_create_batch_deactivate_profile() {
        assert_windows_golden(WindowsGolden {
            generator: create_batch_deactivate_profile,
            python_env: None,
            python_env_escaped: "C:\\esp` tools\\python",
            file_name: "Microsoft.v5.3_deactivate.bat",
            template: include_str!(
                "../../powershell_scripts/idf_tools_profile_deactivate_template.bat"
            ),
            batch: true,
            activate_name: "Microsoft.v5.3_profile.bat",
            deactivate_path: |dir| format!("{}\\Microsoft.v5.3_deactivate.bat", dir),
        });
    }
}
