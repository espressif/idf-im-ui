# ESP-IDF Installation Manager

The ESP-IDF Installation Manager (EIM) is a unified tool that simplifies the setup process for ESP-IDF and integrated development environments (IDEs) across multiple platforms. This cross-platform installer facilitates the installation of prerequisites, ESP-IDF itself, and essential tools, offering a consistent and user-friendly experience on macOS, Linux, and Windows.

> **💡 Quick Install:**
> For most users, we recommend installing EIM using one of the package managers below — they make updates and maintenance easier:
>
> | Platform | Recommended Method |
> |----------|-------------------|
> | **Windows** | [WinGet](#windows-installation) |
> | **macOS** | [Homebrew](#macos-installation-via-homebrew) |
> | **Linux (Debian/Ubuntu)** | [APT](#debian-based-linux-installation-via-apt-repository) |
> | **Linux (Fedora/RHEL)** | [RPM](#rpm-based-linux-installation-via-dnf-repository) |
>
> If you prefer a manual installation, you can download standalone installers for **all platforms** and **ESP-IDF versions** directly from the
> 👉 [**Espressif Download Page**](https://dl.espressif.com/dl/eim/).

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
  * **Prerequisites auto-detection and installation**: EIM can automatically detect and install missing prerequisites on supported platforms (installation of prerequisites is supported only on Windows).
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

Choose your installation method based on your operating system.

---

### Windows Installation

<a id="windows-installation"></a>

#### Option 1: WinGet (Recommended)

The easiest way to install EIM on Windows is using [WinGet](https://learn.microsoft.com/en-us/windows/package-manager/winget/), the Windows Package Manager:

```powershell
# Install the GUI application (recommended for most users)
winget install Espressif.EIM

# Or install the CLI-only version
winget install Espressif.EIM-CLI
```

To update EIM later:
```powershell
winget upgrade Espressif.EIM
```

#### Option 2: Direct Download

Download the latest installer directly:
- From [Espressif Download Page](https://dl.espressif.com/dl/eim/)
- Or from [GitHub Releases](https://github.com/espressif/idf-im-ui/releases)

After downloading, simply run the `.exe` installer.
It will automatically detect and install any missing prerequisites and guide you through the setup process.

#### Running EIM on Windows

After installation, you can launch the GUI from the Start Menu or use the CLI:
```powershell
eim install
```

---

### macOS Installation (via Homebrew)

<a id="macos-installation-via-homebrew"></a>

The **recommended** method for macOS is via [Homebrew](https://brew.sh/) — this ensures EIM stays up to date automatically and integrates seamlessly with your system.

```bash
# Add the Espressif tap
brew tap espressif/eim

# Install the GUI app (launchable from Applications)
brew install --cask eim-gui

# Or install the CLI app (runnable from terminal)
brew install eim
```

To update EIM later:
```bash
brew upgrade eim
# or
brew upgrade --cask eim-gui
```

> 🧩 **Alternative:**
> You can also download macOS `.dmg` or `.zip` installers from the [**Espressif Download Page**](https://dl.espressif.com/dl/eim/).
> This page contains versions for **Windows, macOS, and Linux**, as well as **all supported ESP-IDF releases** — useful if you prefer manual setup or offline installation.

---

### Debian-Based Linux Installation (via APT Repository)

<a id="debian-based-linux-installation-via-apt-repository"></a>

For **Debian-based** distributions (Ubuntu, Debian, Linux Mint, etc.), use the official Espressif APT repository:

```bash
# Add the Espressif APT repository
echo "deb [trusted=yes] https://dl.espressif.com/dl/eim/apt/ stable main" | sudo tee /etc/apt/sources.list.d/espressif.list

# Update package lists
sudo apt update

# Install CLI only
sudo apt install eim-cli

# Or install GUI (includes CLI)
sudo apt install eim
```

To update EIM later:
```bash
sudo apt update && sudo apt upgrade eim
```

---

### RPM-Based Linux Installation (via DNF Repository)

<a id="rpm-based-linux-installation-via-dnf-repository"></a>

For **RPM-based** distributions (Fedora, RHEL, CentOS, openSUSE, etc.), use the official Espressif RPM repository:

```bash
# Add the Espressif RPM repository
sudo tee /etc/yum.repos.d/espressif-eim.repo << 'EOF'
[eim]
name=ESP-IDF Installation Manager
baseurl=https://dl.espressif.com/dl/eim/rpm/$basearch
enabled=1
gpgcheck=0
EOF

# Install CLI only
sudo dnf install eim-cli

# Or install GUI (includes CLI)
sudo dnf install eim
```

To update EIM later:
```bash
sudo dnf upgrade eim
```

---

### Alternative: Direct Downloads

> 🧩 You can download `.deb`, `.rpm`, `.msi`, `.dmg` packages or binaries for Linux from the [**Espressif Download Page**](https://dl.espressif.com/dl/eim/).
> The download page includes packages for **all platforms** and **ESP-IDF versions** — ideal for offline or custom deployments.

> **Note:**
> The *GUI application* includes full *CLI capabilities* and can be invoked directly from the command line (`eim <command>`).
> The *standalone CLI* (`eim-cli`) is *statically linked* and works on most Linux systems without additional dependencies.

---

## Post-Installation

1. Launch the GUI for a visual installation experience or use the command line for automation.
2. The **welcome page** will adapt based on your environment:

   * **No previous installation or offline archive**: The installer will present options for a **New Installation**.
   * **Offline archive detected**: The installer will offer to **Install from Archive** or proceed with a **New Installation** from online sources.
   * **Previous installation detected**: The installer will offer to **Manage Installations** from the dashboard or start a **New Installation**.
3. Follow the installation steps for your chosen method.

For detailed instructions, see:

* [Simplified Installation](./simple_installation.md) for GUI-based quick setup
* [Expert Installation](./expert_installation.md) for advanced GUI configuration
* [Command Line Installation](./cli_installation.md) for CLI usage
* [Headless Usage](./headless_usage.md) for automated installations

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
