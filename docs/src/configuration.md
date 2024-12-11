# Configuration

You can open the configuration file and proceed with simplified installation or go through the wizard.

![Instalation setup](./screenshots/instal_setup.png)

#### Config File

The installer can use a TOML configuration file. the file can be loaded (or draged & dropped) just before choosing between simplified or wizard installation.

Here is an example of what a configuration file might look like:

```toml
path = "/Users/Username/.espressif"
esp_idf_json_path = "/Users/Username/.espressif/tools"
tool_download_folder_name = "dist"
tool_install_folder_name = "tools"
target = ["all"]
idf_versions = ["v5.3.2"]
tools_json_file = "tools/tools.json"
idf_tools_path = "tools/idf_tools.py"
config_file_save_path = "/Users/Username/Downloads/config.toml"
non_interactive = false
wizard_all_questions = false
mirror = "https://github.com"
idf_mirror = "https://github.com"
recurse_submodules = false
install_all_prerequisites = false

```
