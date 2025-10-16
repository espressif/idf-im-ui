# ESP-IDF Installation Manager

The ESP-IDF Installation Manager (EIM) is a unified tool that simplifies the setup process for ESP-IDF and integrated development environments (IDEs) across multiple platforms. This cross-platform installer facilitates the installation of prerequisites, ESP-IDF itself, and essential tools, offering a consistent and user-friendly experience on macOS, Linux, and Windows.

> **💡 Quick Install:**
> You can install EIM using:
> - **[Windows Installer](#windows-installation)**
> - **[Homebrew (macOS)](#macos-installation-via-homebrew)**
> - **[APT (Debian-based Linux)](#debian-based-linux-installation-via-apt-repository)**

## Features

### Cross-Platform Support
- Windows (x64)
- macOS (x64, arm64)
- Linux (x64, arm64)

### Multiple Interfaces
- **Graphical User Interface (GUI)**: User-friendly interface with simplified and expert installation modes, now including visual version management and offline installation capabilities.
- **Command Line Interface (CLI)**: Full functionality available through command line for automation and headless environments

### Advanced Capabilities

  * **Multiple ESP-IDF version management**: Easily view, manage, and switch between multiple installed ESP-IDF versions from a centralized dashboard.
  * **Configurable installation paths**: Choose where your ESP-IDF environment is installed.
  * **Mirror selection for downloads**: Select a preferred mirror for downloading ESP-IDF and tools.
  * **Prerequisites auto-detection and installation**: EIM can automatically detect and install missing prerequisites on supported platforms (installation of prerequisities is supported only on windows).
  * **Configuration import/export**: Save and share your installation configurations with others in your team.
  * **Headless operation support**: Automate installations without user interaction.
  * **Offline installation support**: Install the entire ESP-IDF environment without an active internet connection using a single archive.
  * **Utilize Existing ESP-IDF Repositories**: EIM can use an already existing ESP-IDF Git repository on your filesystem to install all necessary tools, which is particularly useful for ESP-IDF development.

### Integration Support
* CI/CD pipeline integration
* Docker container support
* GitHub Actions compatibility
* Configuration sharing between team members

### User Experience
* Interactive wizards for guided installation
* Progress tracking and detailed logging
* Error recovery options
* Multiple language support
* **Integrated issue reporting**: Easily generate and report issues directly to GitHub.

## Getting Started

1.  Download or install the appropriate version for your platform.

### Windows Installation
<a id="windows-installation"></a>

On **Windows**, the recommended way to get EIM is to download the latest installer directly:

- From [GitHub Releases](https://github.com/espressif/idf-im-ui/releases)
- Or from the official [Espressif download mirror](https://dl.espressif.com/dl/eim/)

After downloading, simply run the `.exe` installer.
It will automatically detect and install any missing prerequisites and guide you through the setup process.

You can also launch the GUI or use the CLI directly after installation:
```bash
eim install
````

### macOS Installation (via Homebrew)

<a id="macos-installation-via-homebrew"></a>

If you're using **macOS**, you can install EIM directly from the Espressif Homebrew tap:

```bash
brew tap espressif/eim
brew install eim
eim install
```

### Debian-Based Linux Installation (via APT Repository)

<a id="debian-based-linux-installation-via-apt-repository"></a>

If you're using a **Debian-based Linux** distribution (e.g. Ubuntu), you can install EIM from the official Espressif APT repository:

```bash
# Add repository
echo "deb [trusted=yes] https://dl.espressif.com/dl/eim/apt/ stable main" | sudo tee /etc/apt/sources.list.d/espressif.list

sudo apt update

# Install CLI only
sudo apt install eim-cli

# Install GUI (includes CLI)
sudo apt install eim
```

> **Note:**
> The *GUI application* includes full *CLI capabilities* and can be invoked directly from the command line (`eim <command>`).
> The *standalone CLI* (`eim-cli`) is *statically linked* and works on most Linux systems without additional dependencies.

2. Launch the GUI for a visual installation experience or use the command line for automation.
3. The **welcome page** will adapt based on your environment:

   * **No previous installation or offline archive**: The installer will present options for a **New Installation**.
   * **Offline archive detected**: The installer will offer to **Install from Archive** or proceed with a **New Installation** from online sources.
   * **Previous installation detected**: The installer will offer to **Manage Installations** from the dashboard or start a **New Installation**.
4. Follow the installation steps for your chosen method.

For detailed instructions, see:

* [Simplified Installation](./simple_installation.md) for GUI-based quick setup
* [Expert Installation](./expert_installation.md) for advanced GUI configuration
* [Command Line Installation](./cli_installation.md) for CLI usage
* [Headless Usage](./headless_usage.md) for automated installations

### Shell Completions

EIM can generate completion scripts for: **bash, zsh, fish, elvish, and PowerShell.**

```bash
eim completions <SHELL>
```

Replace <SHELL> with one of: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

#### Bash (Linux / Ubuntu)

##### Quick (current session only)
```bash
# Loads completion just for this terminal
source <(eim completions bash)
```

##### Persistent (per-user) - Ubuntu/Debian style
```bash
# 1) Ensure bash-completion package is installed
sudo apt update
sudo apt install -y bash-completion

# 2) Save completion to your user directory
mkdir -p ~/.local/share/bash-completion/completions
eim completions bash > ~/.local/share/bash-completion/completions/eim

# 3) Make sure bash-completion is sourced (add to ~/.bashrc if missing)
grep -q 'bash_completion' ~/.bashrc || cat >> ~/.bashrc <<'EOF'
# Enable bash-completion if available
if [ -f /usr/share/bash-completion/bash_completion ]; then
  . /usr/share/bash-completion/bash_completion
elif [ -f /etc/bash_completion ]; then
  . /etc/bash_completion
fi
EOF

# 4) Reload shell
exec bash
```

##### Persistent (system-wide)
```bash
# Writes to a system directory (affects all users)
# Adjust path if your distro uses a different location
eim completions bash | sudo tee /etc/bash_completion.d/eim >/dev/null
exec bash
```
> **Note:** If you usually run ./eim (relative path), it’s best to put eim on your PATH so completion matches the command name:
> ```bash
> mkdir -p ~/bin
> cp ./eim ~/bin/
> echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
> exec bash
> ```

#### Zsh (macOs default, also Linux)

##### Quick (current session only)

```zsh
# Loads completion just for this terminal
# (zsh needs compinit and a function path)
fpath+=(/tmp/eim-completions)
mkdir -p /tmp/eim-completions
eim completions zsh > /tmp/eim-completions/_eim
autoload -Uz compinit; compinit
```

##### Persistent (per-user)

```zsh
# 1) Generate into a per-user completions dir
mkdir -p ~/.zsh/completions
eim completions zsh > ~/.zsh/completions/_eim

# 2) zsh requires "secure" dirs (no group/other write)
chmod go-w ~ ~/.zsh ~/.zsh/completions

# 3) Ensure zsh uses this directory (add to ~/.zshrc if missing)
grep -q 'fpath+=(~/.zsh/completions)' ~/.zshrc || echo 'fpath+=(~/.zsh/completions)' >> ~/.zshrc
grep -q 'compinit' ~/.zshrc || echo 'autoload -Uz compinit; compinit' >> ~/.zshrc

# 4) Reload shell
exec zsh
```

> **Invoking as** ./eim? zsh maps _eim → eim. If you insist on ./eim, add this to ~/.zshrc:
> ```zsh
> compdef _eim ./eim=eim
> ```


##### Troubleshooting (zsh)

```zsh
# If zsh warns about “insecure directories”
compaudit
# Then fix each listed path:
chmod go-w <each_path>
```

#### Fish (Linux / macOS)

##### Quick (current session only)

```fish
eim completions fish | source
```


##### Persistent (per-user)

```fish
mkdir -p ~/.config/fish/completions
eim completions fish > ~/.config/fish/completions/eim.fish
# Open a new fish session or:
exec fish
```

#### Elvish

##### Quick (current session only)

```sh
# Elvish can `eval` the output in the current session
eim completions elvish | elvish
```

##### Persistent (per-user)

```sh
# Save as a module and "use" it from rc
mkdir -p ~/.elvish/lib
eim completions elvish > ~/.elvish/lib/eim.elv

# Ensure your rc file imports it (usually ~/.elvish/rc.elv)
# Add the following line if missing:
# use eim
```

#### PowerShell (Windows, macOS, Linux)

##### Quick (current session only)

```powershell
eim completions powershell | Out-String | Invoke-Expression
```

##### Persistent (per-user)

```powershell
# Ensure you have a profile file
if (!(Test-Path -Path $PROFILE)) { New-Item -Type File -Path $PROFILE -Force | Out-Null }

# Append the completion to your profile
eim completions powershell >> $PROFILE

# Reload profile for current session
. $PROFILE
```


> On `PowerShell 7+`, the profile is typically at: <br>
>
> **Windows:** $HOME\Documents\PowerShell\Microsoft.PowerShell_profile.ps1 <br>
> **macOS/Linux:** $HOME/.config/powershell/Microsoft.PowerShell_profile.ps1

#### Dash (Ubuntu’s /bin/sh)

`dash` is a minimal shell focused on POSIX compatibility and does not support programmable completion like bash/zsh. To use tab completion with EIM on Ubuntu/Debian:

- Run your CLI from `bash` (or zsh/fish):
  ```bash
  bash -lc "eim <TAB>"
  ```

- Or switch your interactive shell to bash:
  ```bash
  chsh -s /bin/bash
  # log out and back in
  ```

#### Updating Completions
After updating EIM or its CLI options, regenerate the script:

```sh
# bash (per-user)
eim completions bash > ~/.local/share/bash-completion/completions/eim

# zsh
eim completions zsh > ~/.zsh/completions/_eim

# fish
eim completions fish > ~/.config/fish/completions/eim.fish

# elvish
eim completions elvish > ~/.elvish/lib/eim.elv

# PowerShell
eim completions powershell > $PROFILE; . $PROFILE
```
<br>

> If you change how you invoke the binary (e.g., rename it), regenerate the completion with the new name and reinstall it.

## Architecture

EIM is built with a modular architecture that separates the core functionality from the user interfaces. This allows both the GUI and CLI to provide the same capabilities while catering to different use cases.

```
┌─────────────────┐    ┌─────────────────┐
│   GUI Frontend  │    │   CLI Frontend   │
└────────┬────────┘    └────────┬────────┘
         │                      │
         v                      v
┌────────────────────────────────────────┐
│           Core Installation            │
│            & Configuration             │
└────────────────────────────────────┬───┘
                                     │
                                     v
┌────────────────────────────────────────┐
│        ESP-IDF & Tools Manager         │
└────────────────────────────────────────┘
```

## Contributing

EIM is an open-source project, and contributions are welcome. Visit our [GitHub repository](https://github.com/espressif/idf-im-ui) for:

* Source code
* Issue tracking
* Feature requests
* Pull requests

## Support

If you need help with EIM:

* Check the [FAQ](./faq.md) for common questions
* Visit the [ESP32 forum](https://esp32.com/) for community support
* Open an issue on [GitHub](https://github.com/espressif/idf-im-ui/issues) for bug reports

## Privacy and Data Collection

To help us improve the ESP-IDF Installation Manager, we collect some anonymous usage data. We are committed to transparency and want you to be fully informed about what data we collect and how to opt-out.

### What data do we collect?

We collect the following information to understand how the installer is used and to identify areas for improvement:

- **Environment / System Info**
  - OS & version (Windows, macOS, Linux)
  - Architecture (x64, ARM, etc.)
  - App version (EIM version)
- **User Flows**
  - Online Installation
  - Offline Installation
- **Usage Tracking**
  - Which ESP-IDF version was selected for installation
  - Time taken for the installation
- **Error & Failure Tracking**
  - Installation step that failed
  - Error message related to the failure

This data is completely anonymous and does not contain any personal information.

### How to disable data collection?

You have full control over data collection.

- **GUI**: On the welcome page of the installer, you will find a checkbox to disable telemetry. Unchecking this box will completely prevent any data from being sent.
- **CLI**: When using the command-line interface, you can use the `--do-not-track true` flag to disable telemetry for that session.
