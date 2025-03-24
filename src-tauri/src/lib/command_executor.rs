#[cfg(target_os = "windows")]
use std::io::Write;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::{Command, Output};

pub trait CommandExecutor: Send {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output>;
    fn execute_with_env(
        &self,
        command: &str,
        args: &Vec<&str>,
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output>;
    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output>;
}

struct DefaultExecutor;

impl CommandExecutor for DefaultExecutor {
    fn execute(&self, command: &str, args: &[&str]) -> std::io::Result<Output> {
        Command::new(command).args(args).output()
    }
    fn execute_with_env(
        &self,
        command: &str,
        args: &Vec<&str>,
        env: Vec<(&str, &str)>,
    ) -> std::io::Result<Output> {
        let mut binding = Command::new(command);
        let mut command = binding.args(args);
        for (key, value) in env {
            command = command.env(key, value);
        }
        command.output()
    }
    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output> {
        self.execute("bash", &["-c", script])
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
        .unwrap_or(5); // Default to 5 if parsing fails

    Ok(version)
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
        args: &Vec<&str>,
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

    fn run_script_from_string(&self, script: &str) -> std::io::Result<Output> {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let ps_version = get_powershell_version()?;

        if ps_version >= 7 {
            // PowerShell 7+ approach

            let mut temp_file = tempfile::NamedTempFile::new()?;

            // Write the script content with necessary setup
            let script_content = format!(
                "$ProgressPreference = 'SilentlyContinue'\n\
                $env:PSModulePath = [System.Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')\n\
                Import-Module Microsoft.PowerShell.Security -Force\n\
                Set-ExecutionPolicy Bypass -Scope Process -Force\n\
                [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072\n\
                {}", 
                script
            );

            temp_file.write_all(script_content.as_bytes())?;

            let mut child = Command::new("powershell")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    temp_file.path().to_str().unwrap(),
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
            Ok(output)
        } else {
            // PowerShell < 7 approach
            let mut child = Command::new("powershell")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "-",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(script.as_bytes())?;
            }

            let output = child.wait_with_output()?;
            Ok(output)
        }
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
