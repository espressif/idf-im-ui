# ESP-IDF Installation Manager

The ESP-IDF Installation Manager (EIM) is a unified tool that simplifies the setup process for ESP-IDF and integrated development environments (IDEs) across multiple platforms. This cross-platform installer facilitates the installation of prerequisites, ESP-IDF itself, and essential tools, offering a consistent and user-friendly experience on macOS, Linux, and Windows.

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

1.  Download the appropriate version for your platform from the [GitHub](https://github.com/espressif/idf-im-ui/releases) or from [dl.espressif.com](https://dl.espressif.com/dl/eim/) mirror.
2.  Launch the GUI for a visual installation experience or use the command line for automation.
3.  The **welcome page** will adapt based on your environment:
      * **No previous installation or offline archive**: The installer will present options for a **New Installation**.
      * **Offline archive detected**: The installer will offer to **Install from Archive** or proceed with a **New Installation** from online sources.
      * **Previous installation detected**: The installer will offer to **Manage Installations** from the dashboard or start a **New Installation**.
4.  Follow the installation steps for your chosen method.

For detailed instructions, see:
- [Simplified Installation](./simple_installation.md) for GUI-based quick setup
- [Expert Installation](./expert_installation.md) for advanced GUI configuration
- [Command Line Installation](./cli_installation.md) for CLI usage
- [Headless Usage](./headless_usage.md) for automated installations

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
- Source code
- Issue tracking
- Feature requests
- Pull requests

## Support

If you need help with EIM:
- Check the [FAQ](./faq.md) for common questions
- Visit the [ESP32 forum](https://esp32.com/) for community support
- Open an issue on [GitHub](https://github.com/espressif/idf-im-ui/issues) for bug reports
