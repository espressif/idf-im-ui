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
| `run` | Run a command in the context of a specific ESP-IDF version |
| `discover` | Discover available ESP-IDF versions (not implemented yet) |
| `completions` | Generate shell completion script to stdout |

## Command Details

### Install Command

Non-interactive installation of ESP-IDF versions. This command runs in non-interactive mode by default.

> **Note on Python versions:** ESP-IDF supports Python versions 3.10, 3.11, 3.12, 3.13, and 3.14. Python 3.14 is supported on Linux and macOS only; Windows does not support Python 3.14 because ESP-IDF dependencies do not yet support it. Please ensure you have a compatible version installed. Offline installations have stricter requirements, see the `--use-local-archive` option for details.

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
- `--python-env-folder-name <PYTHON_ENV_FOLDER_NAME>`: Folder name to be used for the python environments. If not provided, it will default to `python`.
- `--tools-json-file <TOOLS_JSON_FILE>`: Path to tools.json file relative from ESP-IDF installation folder
- `-n, --non-interactive <NON_INTERACTIVE>`: Run in interactive mode if set to false (default is true for non-interactive mode)
- `-m, --mirror <MIRROR>`: URL for tools download mirror to be used instead of github.com
- `--idf-mirror <IDF_MIRROR>`: URL for ESP-IDF download mirror to be used instead of github.com
- `--pypi-mirror <PYPI_MIRROR>`: URL for PyPI mirror to be used instead of https://pypi.org/simple
- `-r, --recurse-submodules <RECURSE_SUBMODULES>`: Should the installer recurse into submodules of the ESP-IDF repository (default true)
- `-a, --install-all-prerequisites <INSTALL_ALL_PREREQUISITES>`: Should the installer attempt to install all missing prerequisites (Windows only)
- `--config-file-save-path <CONFIG_FILE_SAVE_PATH>`: Path to save the configuration file
- `--idf-features <IDF_FEATURES>`: Comma-separated list of additional IDF features (ci, docs, pytests, etc.) to be installed with ESP-IDF. When installing multiple versions, these features are applied to all versions. For per-version feature configuration, use a configuration file with the `idf_features_per_version` option.
- `--repo-stub <REPO_STUB>`: Custom repository stub to use instead of the default ESP-IDF repository. Allows using custom IDF repositories
- `--skip-prerequisites-check`: Skip prerequisites check. This is useful if you are sure that all prerequisites are already installed and you want to skip the check. This is not recommended unless you know what you are doing, as it can result in a non-functional installation. Use at your own risk.
- `--version-name`: Version name to be used for the installation. If not provided, the version will be derived from the ESP-IDF repository tag or commit hash.
- `--cleanup`: If set to true, the installer will remove temporary tool archive files after installation. Default is false. This is useful for headless, CI, and Docker environments where the installation artifacts are not needed after installation and can significantly reduce the final image size.
- `--use-local-archive <PATH_TO_ARCHIVE>`: Use a local archive for offline installation. The installer will use the provided archive instead of downloading from the internet. The archive should be a `.zst` file. **Do not unpack the .zst archive.** This option is not compatible with online installation options like `--idf-versions`, `--mirror`, etc. At this time, offline installation only supports Python 3.11 to 3.14 on Linux and macOS.
- `--activation-script-path-override`: Optional override for activation script path. This allows specifying a custom path for the activation script to be saved to instead of the default one.
- `--idf-tools <IDF_TOOLS>`: Comma separated list of tools to be installed with ESP-IDF. When installing multiple versions, these tools are applied to all versions. For per-version tool configuration, use a configuration file with the `idf_tools_per_version` option.

### Wizard Command

Run the interactive ESP-IDF Installer Wizard.

```bash
eim wizard [OPTIONS]
```

The wizard command accepts the same options as the install command but runs in interactive mode, guiding you through the installation process with a series of prompts.

When installing multiple ESP-IDF versions, the wizard will prompt you to select features for each version independently, allowing you to customize the installation per version.

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

### Run Command

Run a command in the context of a specific ESP-IDF version. This command sources the activation script for the specified IDF version before executing your command, making all IDF tools and environment variables available.

```bash
eim run <COMMAND> [IDF_VERSION]
```

Arguments:
- `COMMAND`: The command to run (required)
- `IDF_VERSION`: The ID, name, or path of the installed IDF version (optional)

If `IDF_VERSION` is not provided, the command will use the currently selected IDF version (set via `eim select`). If no version is selected and none is specified, an error will be returned.

**Important:** If your command contains special shell characters, you should wrap it in quotes:

```bash
# Correct - command is quoted
eim run "espidf.py build"

# On Windows (PowerShell)
eim run "espidf.py build"

# If you need to use shell features like pipes or redirects, quote the entire command
eim run "idf.py fullclean > cleanup.log"
```

The IDF version can be identified by:
- **ID**: The internal identifier (e.g., `espidf_5.3.2`)
- **Name**: The display name (e.g., `v5.3.2`)
- **Path**: The full installation path

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

### Completions Command

Generate shell completion script to stdout.
```bash
eim completions <SHELL>
```

`SHELL`  Shell for which to generate completion. <br>
**Possible values:** `bash`, `elvish`, `fish`, `powershell`, `zsh`

## Examples

```bash
# Install ESP-IDF v5.3.2 non-interactively (default behavior)
eim install -i v5.3.2

# Install ESP-IDF v5.3.2 in interactive mode
eim install -i v5.3.2 -n false

# Install with specific features
eim install -i v5.3.2 --idf-features=ci,docs

# Install multiple versions with features applied to all
eim install -i v5.3.2,v5.4 --idf-features=ci,docs

# Install with specific tools
eim install -i v5.3.2 --idf-tools=cmake,openocd

# Install multiple versions with tools applied to all
eim install -i v5.3.2,v5.4 --idf-tools=cmake,openocd

# Install using custom repository mirror and stub
eim install -i v5.3.2 --mirror https://my.custom.mirror --repo-stub my-custom-idf

# Run the interactive wizard (allows per-version feature selection)
eim wizard

# Run wizard with multiple versions
eim wizard -i v5.3.2,v5.4

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

# Run a command in the context of a specific IDF version
eim run "idf.py build" v5.3.2

# Run a command using the currently selected IDF version
eim run "idf.py build"

# Run a command with output redirection (command must be quoted)
eim run "idf.py size > sizes.txt" v5.4
```

## Per-Version Feature Configuration

When you need different features for different ESP-IDF versions, use a configuration file:

```toml
# config.toml
idf_versions = ["v5.3.2", "v5.4", "v5.5"]

[idf_features_per_version]
"v5.3.2" = ["ci"]
"v5.4" = ["ci", "docs"]
"v5.5" = ["ci", "docs", "pytest", "sbom"]
```

Then run:
```bash
eim install --config config.toml
```

For more details on feature configuration, see [CLI Configuration](./cli_configuration.md#idf-features-configuration).
