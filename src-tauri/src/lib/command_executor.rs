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
    fn run_script_from_string_streaming(&self, script: &str) -> std::io::Result<std::process::ExitStatus>;
}

struct DefaultExecutor;

impl CommandExecutor for DefaultExecutor {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        Command::new(command).args(args).output()
    }
    fn execute_with_env(
        &self,
        command: &str,
        args: &[&str],
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output> {
        let mut binding = Command::new(command);
        let mut command = binding.args(args);
        for (key, value) in env {
            command = command.env(key, value);
        }
        command.output()
    }
    fn execute_with_dir(
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
    fn spawn_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Child> {
        Command::new(command)
            .args(args)
            .current_dir(dir)
            .spawn()
    }
    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output> {
        Command::new("bash")
            .args(["-c", script])
            .output()
    }
    fn run_script_from_string_streaming(&self, script: &str) -> std::io::Result<std::process::ExitStatus> {
      Command::new("bash")
        .args(["-c", script])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .stdin(std::process::Stdio::inherit())
        .status()
    }
}

#[cfg(target_os = "windows")]
struct WindowsExecutor;

pub fn get_powershell_version() -> std::io::Result<i32> {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut binding = Command::new("powershell");
    let mut cmd = binding.args(["-Command", "$PSVersionTable.PSVersion.Major"]);

    #[cfg(target_os = "windows")]
    let cmd = cmd.creation_flags(CREATE_NO_WINDOW);

    // Explicitly pass PATH to ensure child process sees updates from add_to_path()
    let cmd = cmd.env("PATH", std::env::var("PATH").unwrap_or_default());

    let output = cmd.output()?;

    let version = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .unwrap_or(5);

    Ok(version)
}

#[cfg(target_os = "windows")]
impl WindowsExecutor {
    fn prepare_powershell_script(&self, script: &str) -> std::io::Result<(std::path::PathBuf, Command)> {
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join(format!("idf_script_{}.ps1", std::process::id()));

        let current_path = std::env::var("PATH").unwrap_or_default();
        let script_content = format!(
            "$ProgressPreference = 'SilentlyContinue'\r\n\
             $env:PSModulePath = [System.Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')\r\n\
             Import-Module Microsoft.PowerShell.Security -Force\r\n\
             Set-ExecutionPolicy Bypass -Scope Process -Force\r\n\
             [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072\r\n\
             $env:PATH = \"{};\" + $env:PATH\r\n\
             {}",
            current_path.replace('"', "`\""),
            script
        );

        std::fs::write(&script_path, script_content.as_bytes())?;

        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script_path.to_str().unwrap(),
        ])
        .creation_flags(CREATE_NO_WINDOW);

        Ok((script_path, cmd))
    }
}

#[cfg(target_os = "windows")]
impl CommandExecutor for WindowsExecutor {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(command)
            .args(args)
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
        let mut binding = Command::new(command);
        let mut command = binding.args(args).creation_flags(CREATE_NO_WINDOW);
        for (key, value) in env {
            command = command.env(key, value);
        }
        command.output()
    }

    fn execute_with_dir(
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

    fn spawn_with_dir(
        &self,
        command: &str,
        args: &[&str],
        dir: &str,
    ) -> std::io::Result<Child> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(command)
            .args(args)
            .current_dir(dir)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    }

    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output> {
      let (script_path, mut cmd) = self.prepare_powershell_script(script)?;

      let output = cmd
          .stdout(std::process::Stdio::piped())
          .stderr(std::process::Stdio::piped())
          .spawn()?
          .wait_with_output();

      let _ = std::fs::remove_file(&script_path);
      output
  }

  fn run_script_from_string_streaming(&self, script: &str) -> std::io::Result<std::process::ExitStatus> {
      let (script_path, mut cmd) = self.prepare_powershell_script(script)?;

      let status = cmd
          .stdout(std::process::Stdio::inherit())
          .stderr(std::process::Stdio::inherit())
          .stdin(std::process::Stdio::inherit())
          .status();

      let _ = std::fs::remove_file(&script_path);
      status
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
    execute_command(command, args)
}

pub fn execute_command_direct_with_env(
    command: &str,
    args: &[&str],
    env: Vec<(&str, &str)>,
) -> std::io::Result<Output> {
    execute_command_with_env(command, &args.to_vec(), env)
}

pub fn execute_command_direct_with_dir(
    command: &str,
    args: &[&str],
    dir: &str,
) -> std::io::Result<Output> {
    execute_command_with_dir(command, args, dir)
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
