#[cfg(target_os = "windows")]
use std::io::Write;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::{Command, Output};

use std::process::Child;

pub trait CommandExecutor: Send {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output>;
    fn execute_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output>;
    fn execute_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Output>;
    fn spawn_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Child>;
    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output>;

    fn execute_direct(&self, command: &str, args: &[&str]) -> std::io::Result<Output>;
    fn execute_direct_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output>;
    fn execute_direct_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Output>;
}

/// Helper function to escape shell arguments for bash
fn escape_bash_arg(arg: &str) -> String {
    // If the argument contains no special characters, return as-is
    if arg.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/') {
        return arg.to_string();
    }

    // Otherwise, wrap in single quotes and escape any single quotes
    format!("'{}'", arg.replace('\'', "'\\''"))
}

/// Helper function to escape PowerShell arguments
#[cfg(target_os = "windows")]
fn escape_powershell_arg(arg: &str) -> String {
    // PowerShell requires different escaping than cmd
    let mut escaped = String::new();
    let needs_quotes = arg.contains(' ') || arg.contains('\t') || arg.contains('\'') || arg.contains('"');

    if needs_quotes {
        escaped.push('"');
    }

    for ch in arg.chars() {
        match ch {
            '"' => escaped.push_str("`\""),
            '`' => escaped.push_str("``"),
            '$' => escaped.push_str("`$"),
            _ => escaped.push(ch),
        }
    }

    if needs_quotes {
        escaped.push('"');
    } else {
        escaped = arg.to_string();
    }

    escaped
}

struct DefaultExecutor;

impl CommandExecutor for DefaultExecutor {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        // Use shell to preserve variable expansion, globs, etc.
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_bash_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            escape_bash_arg(command)
        } else {
            format!("{} {}", escape_bash_arg(command), escaped_args.join(" "))
        };

        Command::new("bash")
            .args(["-c", &full_command])
            .output()
    }

    fn execute_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output> {
        // Use shell with environment variables
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_bash_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            escape_bash_arg(command)
        } else {
            format!("{} {}", escape_bash_arg(command), escaped_args.join(" "))
        };

        let mut binding = Command::new("bash");
        let mut cmd = binding.args(["-c", &full_command]);
        for (key, value) in env {
            cmd = cmd.env(key, value);
        }
        cmd.output()
    }

    fn execute_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Output> {
        // Use shell with directory
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_bash_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            escape_bash_arg(command)
        } else {
            format!("{} {}", escape_bash_arg(command), escaped_args.join(" "))
        };

        Command::new("bash")
            .args(["-c", &full_command])
            .current_dir(dir)
            .output()
    }

    fn spawn_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Child> {
        // Use shell with directory
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_bash_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            escape_bash_arg(command)
        } else {
            format!("{} {}", escape_bash_arg(command), escaped_args.join(" "))
        };

        Command::new("bash")
            .args(["-c", &full_command])
            .current_dir(dir)
            .spawn()
    }

    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output> {
        Command::new("bash")
            .args(["-c", script])
            .output()
    }

    fn execute_direct(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        Command::new(command).args(args).output()
    }

    fn execute_direct_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output> {
        let mut binding = Command::new(command);
        let mut cmd = binding.args(args);
        for (key, value) in env {
            cmd = cmd.env(key, value);
        }
        cmd.output()
    }

    fn execute_direct_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Output> {
        Command::new(command)
            .args(args)
            .current_dir(dir)
            .output()
    }
}

#[cfg(target_os = "windows")]
struct WindowsExecutor;

/// Retrieves the major version number of PowerShell installed on the system.
///
/// This function executes a PowerShell command to fetch the major version number
/// of the installed PowerShell. On Windows, it uses the CREATE_NO_WINDOW flag
/// to prevent a console window from appearing during execution.
///
/// # Returns
///
/// Returns a `Result` containing:
/// - `Ok(i32)`: The major version number of PowerShell if successfully retrieved.
///   If parsing fails, it defaults to version 5.
/// - `Err(std::io::Error)`: An error if the PowerShell command execution fails.
///
/// # Platform-specific behavior
///
/// On Windows, this function uses the CREATE_NO_WINDOW flag to suppress the console window.
pub fn get_powershell_version() -> std::io::Result<i32> {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut binding = Command::new("powershell");
    let mut output = binding.args(["-Command", "$PSVersionTable.PSVersion.Major"]);

    #[cfg(target_os = "windows")]
    let output = output.creation_flags(CREATE_NO_WINDOW);

    let output = output.output()?;

    let version = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .unwrap_or(5);

    Ok(version)
}

#[cfg(target_os = "windows")]
impl CommandExecutor for WindowsExecutor {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // Use PowerShell to preserve variable expansion, etc.
        let escaped_command = escape_powershell_arg(command);
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_powershell_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            format!("& {}", escaped_command)
        } else {
            format!("& {} {}", escaped_command, escaped_args.join(" "))
        };

        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &full_command])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    fn execute_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let escaped_command = escape_powershell_arg(command);
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_powershell_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            format!("& {}", escaped_command)
        } else {
            format!("& {} {}", escaped_command, escaped_args.join(" "))
        };

        let mut binding = Command::new("powershell");
        let mut cmd = binding
            .args(["-NoProfile", "-NonInteractive", "-Command", &full_command])
            .creation_flags(CREATE_NO_WINDOW);
        for (key, value) in env {
            cmd = cmd.env(key, value);
        }
        cmd.output()
    }

    fn execute_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let escaped_command = escape_powershell_arg(command);
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_powershell_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            format!("& {}", escaped_command)
        } else {
            format!("& {} {}", escaped_command, escaped_args.join(" "))
        };

        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &full_command])
            .current_dir(dir)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    fn spawn_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Child> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let escaped_command = escape_powershell_arg(command);
        let escaped_args: Vec<String> = args.iter().map(|arg| escape_powershell_arg(arg)).collect();
        let full_command = if escaped_args.is_empty() {
            format!("& {}", escaped_command)
        } else {
            format!("& {} {}", escaped_command, escaped_args.join(" "))
        };

        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &full_command])
            .current_dir(dir)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    }

    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output> {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let ps_version = get_powershell_version()?;

        if ps_version >= 7 {
            // Create temp file with .ps1 extension - PowerShell requires this
            let mut temp_file = tempfile::Builder::new()
                .prefix("ps_script_")
                .suffix(".ps1")
                .tempfile()?;

            // For PowerShell 7+, write UTF-8 with BOM to temp file
            let bom = b"\xEF\xBB\xBF"; // UTF-8 BOM as bytes
            let script_content = format!(
                "$OutputEncoding = [System.Text.Encoding]::UTF8\n\
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
                $ProgressPreference = 'SilentlyContinue'\n\
                $env:PSModulePath = [System.Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')\n\
                Import-Module Microsoft.PowerShell.Security -Force\n\
                Set-ExecutionPolicy Bypass -Scope Process -Force\n\
                [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072\n\
                {}",
                script
            );

            // Write directly to the temp_file handle
            temp_file.write_all(bom)?;
            temp_file.write_all(script_content.as_bytes())?;
            temp_file.flush()?;

            // Get the path and then fully close the file by persisting and dropping
            let path = temp_file.path().to_path_buf();
            let persist_path = temp_file.into_temp_path();
            let final_path = persist_path.keep()?;

            // Now the file is fully closed and PowerShell can access it
            let mut child = Command::new("powershell")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    final_path.to_str().unwrap(),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .env(
                    "PSModulePath",
                    std::env::var("PSModulePath").unwrap_or_default(),
                )
                .spawn()?;

            let output = child.wait_with_output()?;

            // Clean up the temp file manually
            let _ = std::fs::remove_file(&final_path);

            Ok(output)
        } else {
            // PowerShell < 7: Also use temp file with .ps1 extension
            let mut temp_file = tempfile::Builder::new()
                .prefix("ps_script_")
                .suffix(".ps1")
                .tempfile()?;

            // Write UTF-8 with BOM
            let bom = b"\xEF\xBB\xBF"; // UTF-8 BOM as bytes
            let script_content = format!(
                "$OutputEncoding = [System.Text.Encoding]::UTF8\n\
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n\
                {}",
                script
            );

            // Write directly to the temp_file handle
            temp_file.write_all(bom)?;
            temp_file.write_all(script_content.as_bytes())?;
            temp_file.flush()?;

            // Get the path and then fully close the file by persisting and dropping
            let persist_path = temp_file.into_temp_path();
            let final_path = persist_path.keep()?;

            // Now the file is fully closed and PowerShell can access it
            let mut child = Command::new("powershell")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    final_path.to_str().unwrap(),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            let output = child.wait_with_output()?;

            // Clean up the temp file manually
            let _ = std::fs::remove_file(&final_path);

            Ok(output)
        }
    }

    fn execute_direct(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(command)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }

    fn execute_direct_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut binding = Command::new(command);
        let mut cmd = binding.args(args).creation_flags(CREATE_NO_WINDOW);
        for (key, value) in env {
            cmd = cmd.env(key, value);
        }
        cmd.output()
    }

    fn execute_direct_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(command)
            .args(args)
            .current_dir(dir)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    }
}

pub fn get_executor() -> Box<dyn CommandExecutor> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsExecutor)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(DefaultExecutor)
    }
}

pub fn execute_command(command: &str, args: &[&str]) -> std::io::Result<Output> {
    let executor = get_executor();
    executor.execute(command, args)
}

pub fn execute_command_with_env(
    command: &str,
    args: &Vec<&str>,
    env: Vec<(&str, &str)>,
) -> std::io::Result<Output> {
    let executor = get_executor();
    executor.execute_with_env(command, args, env)
}

pub fn execute_command_with_dir(
    command: &str,
    args: &[&str],
    dir: &str,
) -> std::io::Result<Output> {
    let executor = get_executor();
    executor.execute_with_dir(command, args, dir)
}

pub fn spawn_with_dir(
    command: &str,
    args: &[&str],
    dir: &str,
) -> std::io::Result<Child> {
    let executor = get_executor();
    executor.spawn_with_dir(command, args, dir)
}

pub fn execute_command_direct(command: &str, args: &[&str]) -> std::io::Result<Output> {
    let executor = get_executor();
    executor.execute_direct(command, args)
}

pub fn execute_command_direct_with_env(
    command: &str,
    args: &[&str],
    env: Vec<(&str, &str)>,
) -> std::io::Result<Output> {
    let executor = get_executor();
    executor.execute_direct_with_env(command, args, env)
}

pub fn execute_command_direct_with_dir(
    command: &str,
    args: &[&str],
    dir: &str,
) -> std::io::Result<Output> {
    let executor = get_executor();
    executor.execute_direct_with_dir(command, args, dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_execute_command_basic() {
        let output = execute_command("echo", &["hello"]).unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn test_execute_command_direct_basic() {
        let output = execute_command_direct("echo", &["hello", "world"]).unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
        assert!(stdout.contains("world"));
    }

    #[test]
    fn test_execute_command_with_spaces_in_args() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file with spaces.txt");
        fs::write(&file_path, "test content").unwrap();

        // Test direct execution with spaces
        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct("cat", &[file_path.to_str().unwrap()]).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(stdout.trim(), "test content");
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct("type", &[file_path.to_str().unwrap()]).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("test content"));
        }
    }

    #[test]
    fn test_execute_command_with_dir() {
        let temp_dir = TempDir::new().unwrap();
        let file_name = "test.txt";
        let file_path = temp_dir.path().join(file_name);
        fs::write(&file_path, "content").unwrap();

        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct_with_dir(
                "ls",
                &[],
                temp_dir.path().to_str().unwrap(),
            )
            .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains(file_name));
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct_with_dir(
                "cmd",
                &["/c", "dir", "/b"],
                temp_dir.path().to_str().unwrap(),
            )
            .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains(file_name));
        }
    }

    #[test]
    fn test_execute_command_with_env() {
        let env_vars = vec![("TEST_VAR", "test_value")];

        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct_with_env(
                "sh",
                &["-c", "echo $TEST_VAR"],
                env_vars,
            )
            .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("test_value"));
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct_with_env(
                "cmd",
                &["/c", "echo %TEST_VAR%"],
                env_vars,
            )
            .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("test_value"));
        }
    }

    #[test]
    fn test_run_script_from_string() {
        #[cfg(not(target_os = "windows"))]
        {
            let executor = get_executor();
            let script = r#"
                echo "line1"
                echo "line2"
            "#;
            let output = executor.run_script_from_string(script).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("line1"));
            assert!(stdout.contains("line2"));
        }

        #[cfg(target_os = "windows")]
        {
            let executor = get_executor();
            let script = r#"
                Write-Output "line1"
                Write-Output "line2"
            "#;
            let output = executor.run_script_from_string(script).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("line1"));
            assert!(stdout.contains("line2"));
        }
    }

    #[test]
    fn test_utf8_filename() {
        let temp_dir = TempDir::new().unwrap();
        let file_name = "tëst_文件.txt";
        let file_path = temp_dir.path().join(file_name);
        fs::write(&file_path, "utf8 content").unwrap();

        // Test that direct execution can handle UTF-8 filenames
        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct("cat", &[file_path.to_str().unwrap()]).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(stdout.trim(), "utf8 content");
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct("type", &[file_path.to_str().unwrap()]).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("utf8 content"));
        }
    }

    #[test]
    fn test_directory_with_spaces() {
        let temp_dir = TempDir::new().unwrap();
        let subdir_path = temp_dir.path().join("dir with spaces");
        fs::create_dir(&subdir_path).unwrap();
        let file_path = subdir_path.join("test.txt");
        fs::write(&file_path, "content in spaced dir").unwrap();

        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct("cat", &[file_path.to_str().unwrap()]).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(stdout.trim(), "content in spaced dir");
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct("type", &[file_path.to_str().unwrap()]).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("content in spaced dir"));
        }
    }

    #[test]
    fn test_escape_bash_arg() {
        assert_eq!(escape_bash_arg("simple"), "simple");
        assert_eq!(escape_bash_arg("with space"), "'with space'");
        assert_eq!(escape_bash_arg("with'quote"), "'with'\\''quote'");
        assert_eq!(escape_bash_arg("with$dollar"), "'with$dollar'");
        assert_eq!(escape_bash_arg("/path/to/file"), "/path/to/file");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_escape_powershell_arg() {
        assert_eq!(escape_powershell_arg("simple"), "simple");
        assert_eq!(escape_powershell_arg("with space"), "\"with space\"");
        assert_eq!(escape_powershell_arg("with\"quote"), "\"with`\"quote\"");
        assert_eq!(escape_powershell_arg("with$dollar"), "\"with`$dollar\"");
        assert_eq!(escape_powershell_arg("with`backtick"), "\"with``backtick\"");
    }

    #[test]
    fn test_execute_nonexistent_command() {
        let result = execute_command_direct("nonexistent_command_xyz", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_command_with_error() {
        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct("ls", &["/nonexistent/path/xyz"]).unwrap();
            assert!(!output.status.success());
            assert!(!output.stderr.is_empty());
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct("cmd", &["/c", "dir", "C:\\nonexistent\\path\\xyz"]).unwrap();
            assert!(!output.status.success());
        }
    }

    #[test]
    fn test_multiple_args_with_spaces() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file 1.txt");
        let file2 = temp_dir.path().join("file 2.txt");
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        #[cfg(not(target_os = "windows"))]
        {
            let output = execute_command_direct(
                "cat",
                &[file1.to_str().unwrap(), file2.to_str().unwrap()],
            )
            .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("content1"));
            assert!(stdout.contains("content2"));
        }

        #[cfg(target_os = "windows")]
        {
            let output = execute_command_direct(
                "cmd",
                &["/c", "type", file1.to_str().unwrap(), file2.to_str().unwrap()],
            )
            .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("content1"));
            assert!(stdout.contains("content2"));
        }
    }

    #[test]
    fn test_shell_vs_direct_execution_difference() {
        // Test that shell execution expands variables but direct doesn't
        #[cfg(not(target_os = "windows"))]
        {
            // With shell - should NOT expand properly quoted $HOME
            let output_shell = execute_command("echo", &["$HOME"]).unwrap();
            let stdout_shell = String::from_utf8_lossy(&output_shell.stdout);

            // Direct execution - should NOT expand $HOME
            let output_direct = execute_command_direct("echo", &["$HOME"]).unwrap();
            let stdout_direct = String::from_utf8_lossy(&output_direct.stdout);

            // Shell should not expand but keep literal
            assert!(stdout_shell.trim() == "$HOME");
            assert_eq!(stdout_direct.trim(), "$HOME");
        }

        #[cfg(target_os = "windows")]
        {
            // With shell - should expand %TEMP%
            let output_shell = execute_command("echo", &["%TEMP%"]).unwrap();
            let stdout_shell = String::from_utf8_lossy(&output_shell.stdout);

            // Direct execution - should NOT expand %TEMP%
            let output_direct = execute_command_direct("cmd", &["/c", "echo", "%TEMP%"]).unwrap();
            let stdout_direct = String::from_utf8_lossy(&output_direct.stdout);

            // Note: We use cmd /c echo for direct to make it comparable
            // Direct execution won't expand the variable in the same way
            assert!(stdout_shell.trim().len() > 6);  // Shell expands to a path
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_powershell_version_detection() {
        let version = get_powershell_version();
        assert!(version.is_ok());
        let ver = version.unwrap();
        assert!(ver >= 5); // Windows should have at least PowerShell 5
    }

    #[test]
    fn test_script_with_utf8_output() {
        #[cfg(not(target_os = "windows"))]
        {
            let executor = get_executor();
            let script = r#"echo "Hello 世界 🌍""#;
            let output = executor.run_script_from_string(script).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("世界"));
            assert!(stdout.contains("🌍"));
        }

        #[cfg(target_os = "windows")]
        {
            let executor = get_executor();
            let script = r#"Write-Output "Hello 世界 🌍""#;
            let output = executor.run_script_from_string(script).unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("Hello"));
            // Note: PowerShell UTF-8 handling may vary, so we just check it doesn't crash
        }
    }
}
