# CLI Configuration

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

Use TOML format configuration files for reproducible installations:

```toml
path = "/opt/esp-idf"
esp_idf_json_path = "/opt/esp-idf/tools"
tool_download_folder_name = "dist"
tool_install_folder_name = "tools"
target = ["all"]
idf_versions = ["v5.3.2"]
tools_json_file = "tools/tools.json"
idf_tools_path = "tools/idf_tools.py"
config_file_save_path = "config.toml"
non_interactive = false
wizard_all_questions = false
mirror = "https://github.com"
idf_mirror = "https://github.com"
recurse_submodules = true
install_all_prerequisites = false
```

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

# To run in interactive mode, explicitly set non-interactive to false
eim install -n false
```

See [Headless Usage](./headless_usage.md) for more details about automated installations.