# Espressif Installation Manager automated tests

## Concepts

The EMI application should have a test structure that would allow validation of customer use cases on the final artifacts. The automated tests intends to execute basic validation, with additional specific tests executed manually. The roadmap includes automating most tests possible to reduce manual testing workload

All tests are developed in Node.js using Chai and Mocha as test libraries in combination with Node-PTY for terminal emulation. It is required to install node on the test runner machine.

The GUI tests uses Selenium webdriver in combination with tauri-driver to validate the interface user experience. Although the install procedure backend is the same for both CLI and GUI, the tests implement full IDF installation for both artifacts.

## Environment Setup

On the test machine, the first step is to copy the testing artifacts. The location of the artifacts can be set using environment variable, or the test will look for the `eim` file in the default location:

CLI
Windows: `$USERPROFILE\eim-cli\`
Linux/MacOS: `$HOME/eim-cli/`

GUI
Windows: `$USERPROFILE\eim-gui\`
Linux/MacOS: `$HOME/eim-gui/`

### Windows

Install chocolatey package manager:

> https://docs.chocolatey.org/en-us/choco/setup/

Run this command with administrator privileges.

`Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))`

Install Node.js:

> https://nodejs.org/en/download/prebuilt-installer/current

`choco install nodejs-lts --version="20.18.1" -y`

Install git:

> https://git-scm.com/download/win

`choco install git.install -y`

Clone the public repository:

`git clone https://github.com/espressif/idf-im-ui.git`

### Linux:

Install Git and curl and build-essential packages

`sudo apt install -y git curl build-essential`

`curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.0/install.sh | bash`

Start a new terminal (to load nvm)

`nvm install 20`

Clone the public repository:

`git clone https://github.com/espressif/idf-im-ui.git`

> **At his point test for prerequisites can be run, the remaining tests requires the pre-requisites to be installed.**

Install ESP-IDF pre-requisites

> https://docs.espressif.com/projects/esp-idf/en/v5.3.1/esp32/get-started/linux-macos-setup.html

`sudo apt install git cmake ninja-build wget flex bison gperf ccache libffi-dev libssl-dev dfu-util libusb-dev python3 python3-venv python3-pip`

### MacOS

Install homebrew package manager if not already installed:

> https://brew.sh/

`/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"`

Install node.js

> https://nodejs.org/en/download/package-manager

`brew install node@20`

`echo 'export PATH="/usr/local/opt/node@20/bin:$PATH"' >> ~/.zshrc`

> This requires to restart the terminal in order to load Node.JS

Install git

> https://git-scm.com/downloads/mac

`brew install git`

Clone the public repository:

`git clone https://github.com/espressif/idf-im-ui.git`

> **At his point test for prerequisites can be run, the remaining tests requires the pre-requisites to be installed.**

Install ESP-IDF pre-requisites

> https://docs.espressif.com/projects/esp-idf/en/v5.3.1/esp32/get-started/linux-macos-setup.html

`brew install cmake ninja dfu-util`

## For GUI testing

In order to install the Tauri specific files make sure to have Rust and Cargo installed in the system.

On Linux:
`curl https://sh.rustup.rs -sSf | sh`

On Windows:
Download the rust up package [here](https://doc.rust-lang.org/cargo/getting-started/installation.html).

The tests rely on WEbDriver interface from the running operating system on a cross platform wrapper from Tauri, both the webdriver kit and the tauri driver must be installed in the system in order to run the test scripts.

Linux:
Install WebKitWebDriver ( use command `which WebKitWebDriver` to check if available)
`sudo apt install webkit2gtk-driver`

Windows:
Download [Microsoft Edge Driver](https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver) at the same version from Microsoft Edge installed in the system (Please double check version)
The Edge Driver must be copied on system path, either add the file location to path, or copy it to a valid windows PATH folder (c:\windows)
If changing path note that this is not a permanent change and need to be redone for every new shell, or added to the $profile
`$env:Path += ';C:\<your_folder>'`
`$env:Path += ';'+$env:USERPROFILE+'\EdgeDriver'`

Install the tauri-driver using Cargo:
`cargo install tauri-driver`

## Running the tests

Test scripts are created to allow launching and running the tests. These scripts do not build the Tauri application, it is necessary to have them compiled before running the tests.

Navigate to the idf-im-ui folder, where the repository was cloned.

The test runs are split into multiple files, each of them with an associated JSON test script and a NPM run script for each access.
The tests relies on environmental variables for the information below, the test scripts also have default values associated to this variables in case
environmental variables are not found.

EIM_FILE_PATH -> Specify the path to the EIM application -> default value Windows: `$USERPROFILE\eim-cli\` Linux/MacOS: `$HOME/eim-cli/`
EIM_VERSION -> Version of the EIM application being tested -> default value "eim 0.2.0"
IDF_VERSION -> The latest released version of ESP-IDF, used on express installation by EIM -> default "v5.4.1"

EIM_GUI_PATH -> Specify the path to the EIM application -> default value Windows: `$USERPROFILE\eim-gui\` Linux/MacOS: `$HOME/eim-gui/`
EIM_GUI_VERSION -> Version of the EIM application being tested -> default value "0.2.0"

Option variables

LOG_TO_FILE="true" -> Enable logs saved to text file on the execution folder -> default false
DEBUG="true" -> Enable debug level messages generated by the test scripts -> default false

To modify the test parameters, modify the json files located at `/src/tests/runs/suites`
then execute the tests by running the test npm script passing the test script file name as argument:

`npm run test-CLI --file=CLI-basic`
`npm run test-CLI --file=CLI-extended`
`npm run test-CLI --file=CLI-mirrors`

`npm run test-GUI --file=GUI-basic`
`npm run test-GUI --file=GUI-extended`

> For Windows use `test-CLI-win` or `test-GUI-win`.

The test for prerequisites test can be executed to check the detection of missing prerequisites (before they are installed in the system) by running:

`npm run test-CLI --file=CLI-prerequisites`

# Installation Manager Usage

## Commands

```
Commands:
  install   Install ESP-IDF versions
  list      List installed ESP-IDF versions
  select    Select an ESP-IDF version as active
  discover  Discover available ESP-IDF versions (not implemented yet)
  remove    Remove specific ESP-IDF version
  rename    Rename specific ESP-IDF version
  import    Import existing ESP-IDF installation using tools_set_config.json
  purge     Purge all ESP-IDF installations
  wizard    Run the ESP-IDF Installer Wizard
  help      Print this message or the help of the given subcommand(s)

Options:
  -l, --locale <LOCALE>      Set the language for the wizard (en, cn)
  -v, --verbose...           Increase verbosity level (can be used multiple times)
      --log-file <LOG_FILE>  file in which logs will be stored (default: eim.log)
  -h, --help                 Print help (see more with '--help')
  -V, --version              Print version
```

## install arguments

```
Options:
  -p, --path <PATH>
          Base Path to which all the files and folder will be installed
      --esp-idf-json-path <ESP_IDF_JSON_PATH>
          Absolute path to save esp_idf.json file. Default is $HOME/.esp_installation_manager/esp_idf.json
  -c, --config <FILE>
  -t, --target <TARGET>
          You can provide multiple targets separated by comma
  -i, --idf-versions <IDF_VERSIONS>
          you can provide multiple versions of ESP-IDF separated by comma
      --tool-download-folder-name <TOOL_DOWNLOAD_FOLDER_NAME>
      --tool-install-folder-name <TOOL_INSTALL_FOLDER_NAME>
      --idf-tools-path <IDF_TOOLS_PATH>
          Path to tools.json file relative from ESP-IDF installation folder
      --tools-json-file <TOOLS_JSON_FILE>
          Path to idf_tools.py file relative from ESP-IDF installation folder
  -n, --non-interactive <NON_INTERACTIVE>
          [possible values: true, false]
  -m, --mirror <MIRROR>
          url for download mirror to use instead of github.com
      --idf-mirror <IDF_MIRROR>
          url for download mirror to use instead of github.com for downloading esp-idf
  -v, --verbose...
          Increase verbosity level (can be used multiple times)
  -l, --locale <LOCALE>
          Set the language for the wizard (en, cn)
      --log-file <LOG_FILE>
          file in which logs will be stored (default: eim.log)
  -r, --recurse-submodules <RECURSE_SUBMODULES>
          Should the installer recurse into submodules of the ESP-IDF repository (default true)
          [possible values: true, false]
  -a, --install-all-prerequisites <INSTALL_ALL_PREREQUISITES>
          Should the installer attempt to install all missing prerequisites (default false). This flag only affects Windows platforms as we do not offer prerequisites for other platforms.
          [possible values: true, false]
      --config-file-save-path <CONFIG_FILE_SAVE_PATH>
          if set, the installer will as it's very last move save the configuration to the specified file path. This file can than be used to repeat the installation with the same settings.
  -h, --help
          Print help (see a summary with '-h')
  -V, --version
          Print version
```

## Example config file

file config.toml (Linux)

```
path = "/home/virtual/.esp"
idf_path = "/home/virtual/.esp/v5.3.1/esp-idf"
tool_download_folder_name = "dist"
tool_install_folder_name = "tools"
target = ["all"]
idf_versions = ["v5.3.1"]
tools_json_file = "tools/tools.json"
idf_tools_path = "./tools/idf_tools.py"
mirror = "https://github.com"
idf_mirror = "https://github.com"
```

file config.toml (Windows)

```
path = 'C:\esp\'
idf_path = 'C:\esp\v5.3.1\esp-idf'
tool_download_folder_name = "dist"
tool_install_folder_name = "tools"
target = ["all"]
idf_versions = ["v5.3.1"]
tools_json_file = "tools/tools.json"
idf_tools_path = "./tools/idf_tools.py"
mirror = "https://github.com"
idf_mirror = "https://github.com"
```

## Full arguments

#### Windows:

`.\eim.exe install -p c:\espressif -t all -i v5.3.1 --tool-download-folder-name dist --tool-install-folder-name tools --idf-tools-path ./tools/idf_tools.py --tools-json-file tools/tools.json -m https://github.com --idf-mirror https://github.com -r true`

`.\eim.exe install -c config.toml`

`.\eim.exe install --log-file InstManager.log`

#### Linux & MacOS

`./eim install -p ~/.espressif -t all -i v5.3.1 --tool-download-folder-name dist --tool-install-folder-name tools --idf-tools-path ./tools/idf_tools.py --tools-json-file tools/tools.json -m https://github.com --idf-mirror https://github.com -r true`

`./eim install -c config.toml`

`./eim install --log-file InstManager.log`

## References

Alternative Mirrors:

IDF:
https://github.com
https://jihulab.com/esp-mirror

Tools:
https://github.com
https://dl.espressif.com/github_assets
https://dl.espressif.cn/github_assets

Packages required by EIM:

Windows:
`eim should be able to perform all requirements installation`

Linux:
`sudo apt install git cmake ninja-build wget flex bison gperf ccache libffi-dev libssl-dev dfu-util libusb-dev python3 python3-venv python3-pip`

MacOS:
Install homebrew and load the application to the terminal profile
`/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"`
Then run: `brew install dfu-util cmake ninja python3`
