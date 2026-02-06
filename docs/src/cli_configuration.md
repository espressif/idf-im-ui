# CLI Configuration

> **Important:** CLI releases (e.g., `eim-cli-*.exe` on Windows) must be run from a terminal like PowerShell rather than double-clicking the executable. Double-clicking will open a terminal window briefly, display help information, and then close immediately, which may appear as if the installer crashed.

The command-line interface supports multiple configuration methods with the following priority (highest to lowest):

1. Command line arguments
2. Environment variables
3. Configuration files
4. Default values

## Command Structure

ESP-IDF Installation Manager (EIM) now uses a command-based structure with the following format:

```bash
eim [OPTIONS] [COMMAND] [COMMAND_OPTIONS]
```

For example:
```bash
# Install ESP-IDF with specific version
eim install -i v5.3.2

# Run the interactive wizard
eim wizard

# List installed versions
eim list
```

For a complete list of available commands and their options, see [CLI Commands](./cli_commands.md).

## Command Line Arguments

View all available options with:
```bash
eim --help
```

For help with a specific command:
```bash
eim <command> --help
```

## Environment Variables

Override any configuration setting using environment variables prefixed with `ESP_`. For example:
- `ESP_TARGET`: Set target platform
- `ESP_PATH`: Set installation path
- `ESP_IDF_VERSION`: Set IDF version

Example:
```bash
export ESP_PATH="/opt/esp-idf"
export ESP_IDF_VERSION="v5.3.2"
eim install
```

## Configuration Files

> **Note on Python versions:** ESP-IDF supports Python versions 3.10, 3.11, 3.12, and 3.13. Python 3.14 and later are not supported.

Use TOML format configuration files for reproducible installations:

```toml
path = "/Users/testusername/.espressif"
idf_path = "/Users/testusername/.espressif/v5.5/esp-idf"
esp_idf_json_path = "/Users/testusername/.espressif/tools"
tool_download_folder_name = "/Users/testusername/.espressif/dist"
tool_install_folder_name = "/Users/testusername/.espressif/tools"
python_env_folder_name = "python_env"
cleanup = true
target = ["all"]
idf_versions = ["v5.5"]
tools_json_file = "tools/tools.json"
config_file_save_path = "eim_config.toml"
non_interactive = true
wizard_all_questions = false
mirror = "https://github.com"
idf_mirror = "https://github.com"
pypi_mirror = "https://pypi.org/simple"
recurse_submodules = true
install_all_prerequisites = true
skip_prerequisites_check = false
idf_features = ["ci", "docs"]
```

Load a configuration file:
```bash
eim install --config path/to/config.toml
```

## IDF Features Configuration

ESP-IDF supports optional features (such as `ci`, `docs`, `pytest`, etc.) that install additional Python dependencies. You can configure these features in several ways:

### Global Features (All Versions)

Use the `--idf-features` flag or `idf_features` config option to apply the same features to all ESP-IDF versions being installed:

```bash
# Via command line
eim install -i v5.3.2,v5.4 --idf-features=ci,docs

# Via configuration file
idf_features = ["ci", "docs", "pytest"]
```

### Per-Version Features

When installing multiple ESP-IDF versions, you may want different features for each version. Use the `idf_features_per_version` configuration option in your TOML file:

```toml
idf_versions = ["v5.3.2", "v5.4", "v5.5"]

# Per-version feature selection
[idf_features_per_version]
"v5.3.2" = ["ci", "docs"]
"v5.4" = ["ci", "pytest"]
"v5.5" = ["ci", "docs", "pytest", "sbom"]
```

### Feature Selection Priority

The installer determines which features to use for each version in the following order:

1. **Per-version features** (`idf_features_per_version`): If specified for the version, these are used
2. **Global features** (`idf_features` or `--idf-features`): Applied to all versions without per-version settings
3. **Interactive selection**: In wizard mode, you'll be prompted to select features for each version
4. **Required only**: In non-interactive mode without any feature configuration, only required features are installed

## IDF Tools Configuration

Similar to features, ESP-IDF supports configuring which development tools to install. You can configure these tools in several ways:

### Global Tools (All Versions)

Use the `--idf-tools` flag or `idf_tools` config option to apply the same tools to all ESP-IDF versions being installed:

```bash
# Via command line
eim install -i v5.3.2,v5.4 --idf-tools=cmake,openocd

# Via configuration file
idf_tools = ["cmake", "openocd", "idf-exe"]
```

### Per-Version Tools

When installing multiple ESP-IDF versions, you may want different tools for each version. Use the `idf_tools_per_version` configuration option in your TOML file:

```toml
idf_versions = ["v5.3.2", "v5.4", "v5.5"]

# Per-version tool selection
[idf_tools_per_version]
"v5.3.2" = ["cmake", "openocd"]
"v5.4" = ["cmake", "openocd", "idf-exe"]
"v5.5" = ["cmake", "openocd", "idf-exe", "esp-venv"]
```

### Tool Selection Priority

The installer determines which tools to use for each version in the following order:

1. **Per-version tools** (`idf_tools_per_version`): If specified for the version, these are used
2. **Global tools** (`idf_tools` or `--idf-tools`): Applied to all versions without per-version settings
3. **Interactive selection**: In wizard mode, you'll be prompted to select tools for each version
4. **Required only**: In non-interactive mode without any tool configuration, only required tools are installed

### Interactive Tool Selection

When using the wizard command, you'll be prompted to select tools for each ESP-IDF version:

```bash
eim wizard -i v5.3.2,v5.4
```

The wizard will:
- Fetch available tools for each version and your platform
- Display required tools (pre-selected and cannot be deselected)
- Allow you to select/deselect optional tools independently for each version
- Save your selections to the configuration if you choose to export it

### Interactive Feature Selection

When using the wizard command, you'll be prompted to select features for each ESP-IDF version:

```bash
eim wizard -i v5.3.2,v5.4
```

The wizard will:
- Display available features for each version (features may differ between versions)
- Show which features are required vs optional
- Allow you to select/deselect optional features independently for each version
- Save your selections to the configuration if you choose to export it

Load a configuration file:
```bash
eim install --config path/to/config.toml
```

## Headless Configuration

For automated installations, use the `install` command which runs in non-interactive mode by default:

```bash
# Basic headless installation
eim install

# Headless with specific version and path
eim install -i v5.3.2 -p /opt/esp-idf

# Headless with config file
eim install --config path/to/config.toml

# Headless with specific features
eim install -i v5.3.2 --idf-features=ci,docs

# To run in interactive mode, explicitly set non-interactive to false
eim install -n false
```

See [Headless Usage](./headless_usage.md) for more details about automated installations.
