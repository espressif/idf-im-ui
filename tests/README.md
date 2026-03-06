# Espressif Installation Manager automated tests

## Concepts

The EMI application should have a test structure that would allow validation of customer use cases on the final artifacts. The automated tests intends to execute basic validation, with additional specific tests executed manually. The roadmap includes automating most tests possible to reduce manual testing workload

All tests are developed in Node.js using Chai and Mocha as test libraries in combination with Node-PTY for terminal emulation. It is required to install node on the machine used for the tests.

The GUI tests uses Selenium webdriver in combination with tauri-driver to validate the interface user experience. Although the install procedure backend is the same for both CLI and GUI, the tests implement full IDF installation for both artifacts, including CLI commands to the GUI binary

## Local Environment Setup

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

EIM_CLI_PATH -> Specify the path to the EIM application -> default values defined on the config.js file
EIM_CLI_VERSION -> Version of the EIM application being tested -> default values defined on the config.js file
PREREQUISITES_OS -> Only for prerequisites tests, required to identify the current OS the test is running.
BUILD_INFO_PATH -> The path for the offline archive information file when launching offline installation.

EIM_GUI_PATH -> Specify the path to the EIM application -> default values defined on the config.js file
EIM_GUI_VERSION -> Version of the EIM application being tested -> default values defined on the config.js file

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


Packages required by EIM:

Windows:
`eim should be able to perform all requirements installation`

Linux:
`sudo apt install git cmake wget flex bison gperf ccache libffi-dev libssl-dev dfu-util libusb-dev python3 python3-venv python3-pip`

MacOS:
Install homebrew and load the application to the terminal profile
`/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"`
Then run: `brew install dfu-util cmake python3`


## Running from the terminal

The classes can be controlled from node running in the terminal.
To use it start a terminal instance on the "test" folder and launch node.

### CLI class control

The basic methods and configurations can be imported by using:

````
const {default:os} = await import("os");
const {default:path} = await import("path");
const {default:fs} = await import("fs");
const {default:CLITestRunner} = await import("./classes/CLITestRunner.class.js");
const {default:TestProxy} = await import("./classes/TestProxy.class.js");
let proxy = new TestProxy({ mode: "log" });
await proxy.start();
let eimRunner = new CLITestRunner()
await eimRunner.start();
````

The proxy settings above can be replaced by:
`let proxy = new TestProxy({ mode: "block" });`
or
```
const list = ["github.com"];
let proxy = new TestProxy({ mode: "block-list", blockedDomains: list });
```

Once started commands can be sent to the terminal by using:

`await eimRunner.sendInput(path.join(os.homedir(), "eim-cli", "eim") + " wizard");`

The terminal output can be read using

`eimRunner.output`

Some additional commands:
```
eimRunner.process.write("\x1b[B"); // down arrow
eimRunner.process.write(" "); // spacebar
eimRunner.process.write("\x03"); // ctrl + C
```

### GUI class control

The GUI application can be launched from a node running in the terminal. Note that the actual GUI should be rendered in the machine and commands send on teh node terminal are visible on the interface.

````
const {default:os} = await import("os");
const {default:path} = await import("path");
const {default:fs} = await import("fs");
const {expect} = await import("chai");
const {Builder, By, Key, until} = await import("selenium-webdriver");
const {default:logger} = await import("./classes/logger.class.js");
const {getOSName, getArchitecture, downloadOfflineArchive} = await import("./helper.js");
const {default:GUITestRunner} = await import("./classes/GUITestRunner.class.js");
const {default:TestProxy} = await import("./classes/TestProxy.class.js");
let eimRunner = new GUITestRunner(path.join(os.homedir(), "eim-gui", "eim"));
let proxy = new TestProxy({ mode: "log" });
await proxy.start();
await eimRunner.start();
````

The proxy settings above can be replaced by:
`let proxy = new TestProxy({ mode: "block" });`
or
```
const list = ["github.com"];
let proxy = new TestProxy({ mode: "block-list", blockedDomains: list });
```

Elements selection can be done using the object methods:
```
test = await cards[1].findElement(By.className("version-info"));
let test = await eimRunner.findByDataId("optional-group");
````

Or using Selenium selector:
`let element = await eimRunner.driver.wait(until.elementLocated(By.css(`[data-id="${dataId}"]`)),timeout,`Element with test ID ${dataId} not found`);`

Clicks can be requested using:
`await eimRunner.driver.executeScript("arguments[0].click();", purgeAllButton);`


### Additional notes

Configurations and helper functions can be imported using:

const {
  IDFMIRRORS,
  TOOLSMIRRORS,
  PYPIMIRRORS,
  IDFDefaultVersion,
  IDFDefaultVersionIndex,
  IDFAvailableVersions,
  availableTargets,
  pathToEIMCLI,
  pathToEIMGUI,
  pathToBuildInfo,
  EIMGUIVersion,
  EIMCLIVersion,
  INSTALLFOLDER,
  TOOLSFOLDER,
  runInDebug,
  pythonWheelsVersion,
  prerequisites,
} = await import("./config.js");

const {
  getPlatformKey,
  getPlatformKey_eim,
  getOSName,
  getArchitecture,
  downloadOfflineArchive,
  getAvailableFeatures,
  getAvailableTools,
} = await import("./helper.js");
