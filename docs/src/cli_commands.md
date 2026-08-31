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
- `--do-not-track <DO_NOT_TRACK>`: If set to true, the installer will not send any usage data. Default is false. Applies to every subcommand, including `eim gui` — when the GUI is launched via the CLI it inherits this flag and starts with telemetry disabled. [possible values: true, false]
- `--esp-idf-json-path <PATH>`: Path to the directory for `eim_idf.json`. During install, the configuration file is saved here. For version management commands (list, select, rename, remove, etc.), it specifies where to read the file. Defaults to `~/.espressif/tools` on POSIX, `C:\Espressif\tools` on Windows.
- `-h, --help`: Print help information
- `-V, --version`: Print version information

### Commands Overview

| Command | Description |
|---------|-------------|
| `install` | Install ESP-IDF versions |
| `wizard` | Run the ESP-IDF Installer Wizard (interactive mode) |
| `list` | List installed ESP-IDF versions |
| `list-tools` | List tools declared in an installed ESP-IDF's `tools.json`, with their on-disk installation status |
| `list-features` | List features declared in an installed ESP-IDF's `requirements.json`, with their install status |
| `select` | Select an ESP-IDF version as active |
| `rename` | Rename a specific ESP-IDF version |
| `remove` | Remove a specific ESP-IDF version |
| `fix` | Fix (repair/reinstall) an existing ESP-IDF installation, preserving its original tools/features unless overridden |
| `purge` | Purge all ESP-IDF installations |
| `import` | Import existing ESP-IDF installation using tools_set_config.json |
| `run` | Run a command in the context of a specific ESP-IDF version |
| `shell` | Start an interactive shell with the environment of a specific ESP-IDF version activated |
| `discover` | Discover available ESP-IDF versions (not implemented yet) |
| `completions` | Generate shell completion script to stdout |
| `help-json` | Print help in JSON format for machine reading |

## Command Details

### Install Command

Non-interactive installation of ESP-IDF versions. This command runs in non-interactive mode by default.

> **Note on Python versions:** ESP-IDF supports Python versions 3.10, 3.11, 3.12, 3.13, and 3.14 fully on Linux, macOS and Windows. Please ensure you have a compatible version installed. Offline installations have stricter requirements, see the `--use-local-archive` option for details..

```bash
eim install [OPTIONS]
```

Options:
- `-p, --path <PATH>`: Base path to which all files and folders will be installed
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
- `--version-name`: Version name to be used for the installation. If not provided, the version will be derived from the ESP-IDF repository tag or commit hash. The name becomes part of the Python environment path, the activation script file name and the identifier used by `select`, `run` and `remove`, so pinning it is useful whenever the checked-out revision changes over time — see [Using EIM with git bisect](./git_bisect.md#why---version-name-matters).
- `--cleanup`: If set to true, the installer will remove temporary tool archive files after installation. Default is false. This is useful for headless, CI, and Docker environments where the installation artifacts are not needed after installation and can significantly reduce the final image size.
- `--skip-components-download`: If set to true, the installer will skip the component managers' components download step (`compote registry sync`) after installation. Default is `false` for `install`/`wizard` and `true` for `fix`. Use this when you want to install the toolchain but defer fetching component definitions — for example, in CI/Docker layers where components will be pulled on demand later, or when you intentionally want to keep the installation minimal.
- `--use-local-archive <PATH_TO_ARCHIVE>`: Use a local archive for offline installation. The installer will use the provided archive instead of downloading from the internet. The archive should be a `.zst` file. **Do not unpack the .zst archive.** This option is not compatible with online installation options like `--idf-versions`, `--mirror`, etc. At this time, offline installation only supports Python 3.11 to 3.14 on Linux, macOS, and Windows.
- `--activation-script-path-override`: Optional override for activation script path. This allows specifying a custom path for the activation script to be saved to instead of the default one.
- `--create-bat-activation-script`: Optional flag to create a CMD batch activation script in addition to PowerShell profile. This is for backward compatibility only - PowerShell is recommended and batch support will be abandoned in a future release.
- `--idf-tools <IDF_TOOLS>`: Comma separated list of tools to be installed with ESP-IDF. When installing multiple versions, these tools are applied to all versions. For per-version tool configuration, use a configuration file with the `idf_tools_per_version` option.

### Wizard Command

Run the interactive ESP-IDF Installer Wizard.

```bash
eim wizard [OPTIONS]
```

The wizard command accepts the same options as the install command but runs in interactive mode, guiding you through the installation process with a series of prompts.

When installing multiple ESP-IDF versions, the wizard will prompt you to select features for each version independently, allowing you to customize the installation per version.

#### Incomplete Installation Check

Before starting the wizard, EIM checks whether any previously started installations did not finish successfully. If such installations are found, the wizard pauses and presents each one with its name, status, and path. For each entry you can choose:

- **Fix** — run the repair flow for that installation.
- **Delete** — remove the incomplete installation.
- **Skip** — leave it as-is and continue.

This check only runs in interactive mode (`--non-interactive false` or `eim wizard`). It does not run when using `eim fix` directly.

### List Command

List all installed ESP-IDF versions.

```bash
eim list
```

This command displays all ESP-IDF versions installed on your system, with the currently selected version marked. Each entry also shows the installation **status**:

| Status | Meaning |
|---|---|
| `Finished` | Installation completed successfully |
| `In Progress` | Installation was interrupted or never finished |
| `Failed` | Installation failed before completing |
| `Being Repaired` | A repair is currently in progress |
| `Broken` | A repair attempt failed |

### List Tools Command

List the tools declared in an installed ESP-IDF's `tools/tools.json`, together with their on-disk installation status.

```bash
eim list-tools [IDENTIFIER] [--outdated]
```

Arguments:
- `IDENTIFIER`: ID, name, or path of the IDF installation to inspect (optional). If omitted, the command prompts you to choose from the installed IDFs. Each entry in the interactive list includes its current status in brackets, e.g. `v5.4.0 [Finished]`.

Options:
- `--outdated`: Show only the tools whose on-disk version is older than the latest non-deprecated version declared in `tools.json`. Prints a header line `Outdated tools:` followed by `<name>: version <installed> is outdated by <available>`, or `No outdated tools.` when everything is up to date.

For each tool, the output reports:
- The tool's name and description. Tools with `install: on_request` in `tools.json` are marked with `(optional)`. Tools with `install: never` are filtered out.
- For every version of the tool that has a download for the current platform, a line like `  - <version> (<status>)` followed by either `[installed: <version>]` or `[not installed]`.

The IDF installation is resolved from `IDENTIFIER` by matching its `id`, then its `name`, and finally its normalized `path` in `eim_idf.json`.

This command is intended for inspecting the on-disk state of a toolchain — for example, to confirm which tool versions are present after a fresh install, or to detect tools that can be upgraded in place. The report is also serialized in the underlying library (`version_manager::ToolListReport`), which the GUI's **List Tools** dashboard action (see [Version Management](./version_management.md)) reuses directly.

### List Features Command

List the features declared in an installed ESP-IDF's `tools/requirements.json`, together with whether each one is currently configured to be installed.

```bash
eim list-features [IDENTIFIER]
```

Arguments:
- `IDENTIFIER`: ID, name, or path of the IDF installation to inspect (optional). If omitted, the command prompts you to choose from the installed IDFs.

For each feature, the output reports:
- The feature's name and description. Features with `optional: true` in `requirements.json` (e.g. `ci`, `docs`, `pytest`, `gdbgui`, `ide`) are marked with `(optional)`.
- `[installed]` or `[not installed]`. The required `core` feature is always reported as installed. Optional features are reported as installed if they're part of the version's recorded feature selection — the same selection [`fix`](#fix-command) recovers and preserves.

The IDF installation is resolved from `IDENTIFIER` the same way as `list-tools`: by matching its `id`, then its `name`, and finally its normalized `path` in `eim_idf.json`. Unlike `list-tools`, this command requires no network access — `requirements.json` is read directly from the local ESP-IDF checkout. The underlying report (`version_manager::FeatureListReport`) is what powers the GUI's **List Features** dashboard action (see [Version Management](./version_management.md)).

### Select Command

Select an ESP-IDF version as active.

```bash
eim select [VERSION]
```

If `VERSION` is not provided, the command will prompt you to select from available versions. Each entry in the interactive list includes its current status in brackets, e.g. `v5.4.0 [Finished]`. Selecting a version sets the `idfSelectedId` in the `eim_idf.json` file, which IDEs use to determine the preferred IDF version.

### Rename Command

Rename a specific ESP-IDF version.

```bash
eim rename [VERSION] [NEW_NAME]
```

If `VERSION` is not provided, the command will prompt you to select from available versions. Each entry in the interactive list includes its current status in brackets. If `NEW_NAME` is not provided, the command will prompt you to enter a new name.

### Remove Command

Remove a specific ESP-IDF version.

```bash
eim remove [VERSION]
```

If `VERSION` is not provided, the command will prompt you to select from available versions. Each entry in the interactive list includes its current status in brackets.

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

### Shell Command

Start an interactive shell with the environment of a specific ESP-IDF version activated. Unlike `run`, which executes a single command and exits, `shell` hands control of the current terminal to you and keeps the environment active until you exit that shell.

```bash
eim shell [IDF_VERSION]
```

Arguments:
- `IDF_VERSION`: The ID, name, or path of the installed IDF version (optional)

If `IDF_VERSION` is not provided, the command uses the currently selected IDF version (set via `eim select`). If no version is selected and none is specified, an error is returned.

The new shell is your normal login shell (`$SHELL`), started with its usual startup files (e.g. `~/.bashrc`, `~/.zshrc`) so your own aliases, functions and prompt customizations are kept, in addition to the ESP-IDF ones — `idf.py`, `esptool.py` and friends are available as real shell functions, not just as commands reachable through `PATH`. Bash, zsh and fish are supported directly; on Windows the environment is activated in a PowerShell session (`-NoExit`). Type `exit` (or press Ctrl+D) to leave the shell and return to your previous one — activation is local to that shell process and is not persisted anywhere.

### Discover Command

Discover available ESP-IDF versions (not implemented yet).

```bash
eim discover
```

This command is planned to discover ESP-IDF installations on your system but is not yet implemented.

### Fix Command

Fix (repair/reinstall) an existing ESP-IDF installation by reinstalling its tools and dependencies.

```bash
eim fix [OPTIONS]
```

Options:
- `-p, --path <PATH>`: Path of the existing installation to fix. If omitted, you will be presented with a selection of all known IDF installations to choose from. Each entry shows the installation name, path, and current status in brackets, e.g. `v5.3.0 (/home/user/.espressif/v5.3.0) [Broken]`.

Apart from that, `fix` accepts the same options as [`install`](#install-command) / [`wizard`](#wizard-command) (`--idf-features`, `--idf-tools`, `--target`, `-i, --idf-versions`, `-m, --mirror`, etc.). By default, `fix` reinstalls the version using **exactly the configuration it was originally installed with** — the same target, features and tools are preserved, so you don't lose any customization made at install time. Any option you explicitly pass on the command line overrides both the preserved value and the built-in default for that option, letting you fix an installation with a different set of tools/features than it originally had.

```bash
# Fix (repair) an installation, keeping its original tools/features
eim fix -p /path/to/existing/esp-idf

# Fix an installation and additionally install the cmake and openocd tools
eim fix -p /path/to/existing/esp-idf --idf-tools cmake,openocd

# Fix an installation and additionally install the docs and pytest features
eim fix -p /path/to/existing/esp-idf --idf-features docs,pytest

# Fix an installation, choosing interactively from all installed versions
eim fix
```

`fix` never touches the Git repository itself — it does not fetch, check out or update submodules. It re-reads `tools/tools.json` and the Python requirements from the working tree as it currently stands, which makes it the command to run after you change the checked-out revision yourself. See [Using EIM with git bisect](./git_bisect.md) for that workflow.

#### Mirrors

`fix` reuses the mirrors recorded in the installation and does not measure mirror latency. `install` and `wizard` pick the fastest reachable mirror by probing the candidates, which costs several seconds; repeating that on every `fix` would only re-derive the mirrors the installation already has, so it is skipped.

Pass `-m, --mirror`, `--idf-mirror` or `--pypi-mirror` to fix an installation against a different mirror than the one it was installed with. If the installation has no stored configuration to recover — for example one registered by an older EIM, or one left behind by a `fix` which failed part way through — `fix` falls back to latency-based selection, exactly as `install` does.

#### The Python environment

All commands (`install`, `wizard`, `fix`) keep the existing virtual environment and run pip against it without `--upgrade`, so pip installs only the packages which are missing or no longer satisfy the constraints file. Everything already satisfied is left alone, and pip does not contact the package index for it.

If that pip run fails — a corrupted environment, a missing interpreter, a non-zero pip exit — the environment is deleted and pip is retried once with `--upgrade`. A second failure is reported as an error.

Two consequences worth knowing:

- When a revision tightens a version range — `tools/requirements/*.txt` lists packages without versions and `espidf.constraints.*.txt` supplies the ranges — the installed package no longer satisfies it and pip upgrades it. Moving between revisions therefore still gets you the right package versions.
- Packages no longer drift to the newest release *within* a range that has not changed. If the constraint is `esptool~=5.2` and you have 5.2.1 installed, `fix` leaves it there rather than moving to 5.2.9.

### Completions Command

Generate shell completion script to stdout.
```bash
eim completions <SHELL>
```

`SHELL`  Shell for which to generate completion. <br>
**Possible values:** `bash`, `elvish`, `fish`, `powershell`, `zsh`

### HelpJson Command

Print help in JSON format for machine reading. This is useful for programmatic consumption of CLI help information.

```bash
eim help-json
```

The output is a JSON object containing the command structure with all subcommands, arguments, and their descriptions. This is useful for:
- Building external documentation
- Generating shell completions manually
- Integration with other tools that need to understand the CLI interface

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

# List the tools for an installed IDF and their on-disk status
eim list-tools v5.3.2

# Show only tools whose on-disk version is older than what tools.json declares
eim list-tools v5.3.2 --outdated

# List the features for an installed IDF and their install status
eim list-features v5.3.2

# Select a specific version
eim select v5.3.2

# Rename a version
eim rename v5.3.2 "ESP-IDF 5.3.2 Stable"

# Remove a specific version
eim remove v5.3.2

# Fix (repair) an installation, keeping its original tools/features
eim fix -p /path/to/existing/esp-idf

# Fix an installation while also adding tools that weren't installed originally
eim fix -p /path/to/existing/esp-idf --idf-tools cmake,openocd

# Fix an installation while also adding features that weren't installed originally
eim fix -p /path/to/existing/esp-idf --idf-features docs,pytest

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

# Start an interactive shell with a specific IDF version activated
eim shell v5.3.2

# Start an interactive shell using the currently selected IDF version
eim shell
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
