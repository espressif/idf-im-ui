# Offline Installation

The offline installation feature allows you to install ESP-IDF without an active internet connection. This is particularly useful in environments with restricted network access or for creating reproducible builds.

## How to Get the Offline Installer

To perform an offline installation, you first need to download the appropriate offline installer artifact for your operating system, platform, and desired ESP-IDF version. These artifacts are available from:

-   **GitHub Releases:** [https://github.com/espressif/idf-im-ui/releases](https://github.com/espressif/idf-im-ui/releases)
-   **Espressif Download Portal:** [https://dl.espressif.com/dl/eim/](https://dl.espressif.com/dl/eim/)

The artifacts are named using the following convention: `offline_installer-OS-PLATFORM-IDF_VERSION`. For example, `offline_installer-linux-aarch64-5.5`. On the download page, the artifacts are clearly distinguished to help you find the correct one.

## How to Use the Offline Installer

The downloaded artifact is a zip archive containing:
-   The `eim` installer binary.
-   A `.zst` archive with the necessary data for the installer.
-   A `README.md` file with instructions.

To run the offline installation, follow these steps:

1.  Unzip the downloaded artifact.
2.  Open your terminal or command prompt and navigate to the extracted folder. On Windows, please use PowerShell as cmd is not supported.
3.  Run the installer using the `--use-local-archive` command-line option, providing the path to the `.zst` archive.

```bash
eim install --use-local-archive PATH_TO_ARCHIVE
```

If you are running the command from the same directory where you extracted the archive, the `PATH_TO_ARCHIVE` will simply be the name of the `.zst` file.

> **Important**
> You **must not** unpack the `.zst` archive. The installer uses the archive directly. Please make sure to not unpack the `.zst` archive.

## Prerequisites

### Windows

On Windows, the offline installer will automatically install the necessary prerequisites, Git and Python, if they are not found on your system.

### macOS & Linux

On macOS and Linux, you must have the prerequisites installed on your system before running the offline installer. For a detailed list of prerequisites, please refer to the [Prerequisites](./prerequisites.md) page.

> **Important Note on Python Version**
> The offline installation currently **only supports Python 3.11**. Please ensure you have Python 3.11 installed and available in your system's PATH before starting the installation. This applies to all operating systems.

> **Reminder**
> Do not unpack the `.zst` archive. The installer needs the archive file intact to proceed with the offline installation.
