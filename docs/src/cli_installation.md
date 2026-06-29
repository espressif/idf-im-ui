# Command Line Installation

The command line interface (CLI) of ESP-IDF Installation Manager provides a flexible way to install ESP-IDF, especially useful for automation and headless environments.

## Getting Started

### Windows
> **Important:** CLI releases (e.g., `eim-cli-*.exe`) must be run from PowerShell or Command Prompt rather than double-clicking the executable. Double-clicking will open a terminal window briefly, display help information, and then close immediately, which may appear as if the installer crashed.

Run EIM from PowerShell (do not use x86 version). Navigate to the EIM directory and run:
```bash
.\eim --help
```

### macOS & Linux
After downloading and extracting EIM, make it executable:
```bash
chmod +x ./eim
./eim --help
```

Alternatively, on macOS or Linux you can install EIM via [Homebrew](https://brew.sh/):
```bash
brew tap espressif/eim
brew install eim
```

Or on Arch Linux via pacman:
```bash
# 1. Import and locally sign the Espressif signing key (required once)
curl -fsSL https://dl.espressif.com/dl/eim/eim.asc | sudo pacman-key --add -
sudo pacman-key --lsign-key 06E9A6C0325E124198C685F18B1F38BDC9383E1D

# 2. Add the EIM repository (if not already present)
if ! grep -q "\[eim\]" /etc/pacman.conf; then
  sudo tee -a /etc/pacman.conf << 'EOF'
[eim]
SigLevel = Required DatabaseOptional TrustAll
Server = https://dl.espressif.com/dl/eim/pacman/$arch
EOF
fi

# 3. Sync and install
sudo pacman -Syu eim-cli
```

## Installation Methods

### Interactive Wizard
Running `eim wizard` starts an interactive wizard that guides you through the installation:

1. Prerequisites check
2. Platform selection
3. ESP-IDF version selection
4. Mirror selection
5. Features selection
6. Installation path configuration

At the end of the install, the wizard writes both an activation and a deactivation script into the tool directory. Source the activation script to use ESP-IDF, and source the deactivation script to undo the environment changes when you are done. See [After Installing](./after_installing.md#deactivating-the-environment) for the exact file names per shell.

### Command Line Arguments
For automated installations, use command line arguments:

```bash
# Install specific version
eim install -i v5.3.2

# Install with custom path
eim install -p /opt/esp-idf

# Non-interactive installation
eim install -n true
```

### Important Note on Installation Path and Version:
If the path provided for installation (e.g., via `-p` or in the interactive wizard) is a valid, existing ESP-IDF Git repository, EIM will use that repository directly and will not overwrite its contents. In such a scenario, any ESP-IDF version specified through other parameters (e.g., `-i`for a specific version) will be ignored, as the installer will work with the version already present in the existing repository.

See [CLI Configuration](./cli_configuration.md) for all available options.

## Offline Installation

The CLI provides a way to install ESP-IDF in an offline environment. This is done by using a pre-downloaded archive that contains all the necessary components.

To run an offline installation, you need to:
1.  Download the correct offline installer artifact for your system from the [GitHub releases](https://github.com/espressif/idf-im-ui/releases) or [Espressif's download portal](https://dl.espressif.com/dl/eim/).
2.  Extract the downloaded zip file.
3.  Run the `install` command with the `--use-local-archive` option, pointing to the `.zst` archive file.

```bash
eim install --use-local-archive path/to/your/archive.zst
```

> **Important:**
> - Do **not** unpack the `.zst` archive file; the installer uses it directly.
> - The offline installation currently only supports **Python 3.11** to **3.14** on Linux, macOS, and Windows. Python 3.14 is now fully supported on all platforms for offline installations.
> - On macOS and Linux, you must install all [prerequisites](./prerequisites.md) manually before running the installer.

For a complete guide, please see the [Offline Installation](./offline_installation.md) page.
