# CLI Commands

The ESP-IDF Installation Manager provides a comprehensive command-line interface with various commands to manage your ESP-IDF installations. This document details all available commands and their usage.

## Available Commands

```bash
eim [OPTIONS] [COMMAND]
```

### Global Options

These options can be used with any command:

- `-l, --locale <LOCALE>`: Set the language for the wizard (en, cn)
- `-v, --verbose`: Increase verbosity level (can be used multiple times)
- `--log-file <LOG_FILE>`: File in which logs will be stored (default: eim.log)
- `--do-not-track <DO_NOT_TRACK>`: If set to true, the installer will not send any usage data. Default is false. [possible values: true, false]
- `-h, --help`: Print help information
- `-V, --version`: Print version information

### Commands Overview

| Command | Description |
|---------|-------------|
| `install` | Install ESP-IDF versions |
| `wizard` | Run the ESP-IDF Installer Wizard (interactive mode) |
| `list` | List installed ESP-IDF versions |
| `select` | Select an ESP-IDF version as active |
| `rename` | Rename a specific ESP-IDF version |
| `remove` | Remove a specific ESP-IDF version |
| `purge` | Purge all ESP-IDF installations |
| `import` | Import existing ESP-IDF installation using tools_set_config.json |
| `discover` | Discover available ESP-IDF versions (not implemented yet) |

## Command Details

### Install Command

Non-interactive installation of ESP-IDF versions. This command runs in non-interactive mode by default.

```bash
eim install [OPTIONS]
```

Options:
- `-p, --path <PATH>`: Base path to which all files and folders will be installed
- `--esp-idf-json-path <ESP_IDF_JSON_PATH>`: Absolute path to save eim_idf.json file
- `-c, --config <FILE>`: Path to configuration file
- `-t, --target <TARGET>`: Target platforms (comma-separated)
- `-i, --idf-versions <IDF_VERSIONS>`: ESP-IDF versions to install (comma-separated)
- `--tool-download-folder-name <TOOL_DOWNLOAD_FOLDER_NAME>`: Name of the folder for tool downloads
- `--tool-install-folder-name <TOOL_INSTALL_FOLDER_NAME>`: Name of the folder for tool installations
- `--idf-tools-path <IDF_TOOLS_PATH>`: Path to idf_tools.py file relative from ESP-IDF installation folder
- `--tools-json-file <TOOLS_JSON_FILE>`: Path to tools.json file relative from ESP-IDF installation folder
- `-n, --non-interactive <NON_INTERACTIVE>`: Run in interactive mode if set to false (default is true for non-interactive mode)
- `-m, --mirror <MIRROR>`: URL for tools download mirror to be used instead of github.com
- `--idf-mirror <IDF_MIRROR>`: URL for ESP-IDF download mirror to be used instead of github.com
- `-r, --recurse-submodules <RECURSE_SUBMODULES>`: Should the installer recurse into submodules of the ESP-IDF repository (default true)
- `-a, --install-all-prerequisites <INSTALL_ALL_PREREQUISITES>`: Should the installer attempt to install all missing prerequisites (Windows only)
- `--config-file-save-path <CONFIG_FILE_SAVE_PATH>`: Path to save the configuration file
- `--idf-features <IDF_FEATURES>`: Comma-separated list of additional IDF features (ci, docs, pytests, etc.) to be installed with ESP-IDF
- `--repo-stub <REPO_STUB>`: Custom repository stub to use instead of the default ESP-IDF repository. Allows using custom IDF repositories
- `--skip-prerequisites-check`: Skip prerequisites check. This is useful if you are sure that all prerequisites are already installed and you want to skip the check. This is not recommended unless you know what you are doing. This can produce installation which will not work or kill your kittens. Use at your own risk.
- `--version-name`: Version name to be used for the installation. If not provided, the version will be derived from the ESP-IDF repository tag or commit hash.
- `--use-local-archive <PATH_TO_ARCHIVE>`: Use a local archive for offline installation. The installer will use the provided archive instead of downloading from the internet. The archive should be a `.zst` file. **Do not unpack the .zst archive.** This option is not compatible with online installation options like `--idf-versions`, `--mirror`, etc. At this time, offline installation only supports Python 3.11 to 3.13.

### Wizard Command

Run the interactive ESP-IDF Installer Wizard.

```bash
eim wizard [OPTIONS]
```

The wizard command accepts the same options as the install command but runs in interactive mode, guiding you through the installation process with a series of prompts.

### List Command

List all installed ESP-IDF versions.

```bash
eim list
```

This command displays all ESP-IDF versions installed on your system, with the currently selected version marked.

### Select Command

Select an ESP-IDF version as active.

```bash
eim select [VERSION]
```

If `VERSION` is not provided, the command will prompt you to select from available versions. Selecting version means setting the `idfSelectedId` in the `eim_idf.json` file. This is used by the IDEs to know which of the IDF versions you prefer to use.

### Rename Command

Rename a specific ESP-IDF version.

```bash
eim rename [VERSION] [NEW_NAME]
```

If `VERSION` is not provided, the command will prompt you to select from available versions.
If `NEW_NAME` is not provided, the command will prompt you to enter a new name.

### Remove Command

Remove a specific ESP-IDF version.

```bash
eim remove [VERSION]
```

If `VERSION` is not provided, the command will prompt you to select from available versions.

### Purge Command

Purge all ESP-IDF installations.

```bash
eim purge
```

This command removes all known ESP-IDF installations from your system.

### Import Command

Import an existing ESP-IDF installation using a tools_set_config.json file.

```bash
eim import [PATH]
```

If `PATH` is not provided, the command will inform you that no config file was specified.

### Discover Command

Discover available ESP-IDF versions (not implemented yet).

```bash
eim discover
```

This command is planned to discover ESP-IDF installations on your system but is not yet implemented.

### Fix Command

Fix the ESP-IDF installation by reinstalling the tools and dependencies

```bash
eim fix [PATH]
```

If no `PATH` is provided, the user will be presented with selection of all known IDF installation to select from.

## Examples

```bash
# Install ESP-IDF v5.3.2 non-interactively (default behavior)
eim install -i v5.3.2

# Install ESP-IDF v5.3.2 in interactive mode
eim install -i v5.3.2 -n false

# Install using custom repository mirror and stub
eim install -i v5.3.2 --mirror https://my.custom.mirror --repo-stub my-custom-idf

# Run the interactive wizard
eim wizard

# List installed versions
eim list

# Select a specific version
eim select v5.3.2

# Rename a version
eim rename v5.3.2 "ESP-IDF 5.3.2 Stable"

# Remove a specific version
eim remove v5.3.2

# Purge all installations
eim purge

# Import from a config file
eim import /path/to/tools_set_config.json
```
