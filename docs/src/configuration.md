# Configuration

The ESP-IDF Installation Manager supports configuration through both its graphical interface and command-line options. Choose the method that best suits your needs:

- [GUI Configuration](./gui_configuration.md): Configure through the graphical interface.
- [CLI Configuration](./cli_configuration.md): Configure using command-line arguments or configuration files.

## Handling Existing ESP-IDF Repositories
If you specify an installation path that already contains a valid ESP-IDF Git repository, EIM will detect and use this existing repository. In this scenario, any ESP-IDF version selections made in the configuration file, command line, or GUI will be disregarded. EIM will proceed to install the necessary tools based on the version of ESP-IDF found in the existing repository, without overwriting its contents. This allows you to manage your ESP-IDF Git clone independently and use EIM solely for toolchain setup.

## Configuration Priority

The configuration priority order is:
1. Command line arguments (highest)
2. Environment variables
3. Configuration files
4. Default values (lowest)

## Using Configuration Files

Configuration files can be used with both the GUI and CLI versions of the installer. They provide a simple way to replicate and share installation setups. For detailed usage, refer to the [GUI Configuration](./gui_configuration.md) and [CLI Configuration](./cli_configuration.md) pages.

## Configuration File Format

The installer uses the TOML format for configuration files. Every line is optional; you only need to include the parameters you want to configure.

Here is an example of a comprehensive configuration file:

```toml
path = "/Users/testusername/.espressif"
idf_path = "/Users/testusername/.espressif/v5.5/esp-idf"
esp_idf_json_path = "/Users/testusername/.espressif/tools"
tool_download_folder_name = "/Users/testusername/.espressif/dist"
tool_install_folder_name = "/Users/testusername/.espressif/tools"
python_env_folder_name = "python_env"
target = ["all"]
idf_versions = ["v5.5"]
tools_json_file = "tools/tools.json"
config_file_save_path = "eim_config.toml"
non_interactive = true
wizard_all_questions = false
mirror = "[https://github.com](https://github.com)"
idf_mirror = "[https://github.com](https://github.com)"
recurse_submodules = true
install_all_prerequisites = true
skip_prerequisites_check = false
```
