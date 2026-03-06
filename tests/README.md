# Espressif Installation Manager automated tests

## Concepts

The EMI application should have a test structure that would allow validation of customer use cases on the final artifacts. The automated tests intends to execute basic validation, with additional specific tests executed manually. The roadmap includes automating most tests possible to reduce manual testing workload

All tests are developed in Node.js using Chai and Mocha as test libraries in combination with Node-PTY for terminal emulation. It is required to install node on the machine used for the tests.

The GUI tests uses Selenium webdriver in combination with tauri-driver to validate the interface user experience. Although the install procedure backend is the same for both CLI and GUI, the tests implement full IDF installation for both artifacts, including CLI commands to the GUI binary

## Autotest workflow

The test automation follows a complex workflow by using several files references, mostly intended to provide maximum flexibilities on the tests and allow easy expansion. The subgroups below explains the use of each file on the sequence they are used to trigger the tests.

### Github Workflow

There is a GitHub workflow yaml file dedicated for the test execution, this workflow is called as soon as the build is completed. Using a distinguished file allows running the tests independently directly on GitHub pages. It is possible to manually trigger the execution of the tests over any other build run that has the Installation Manager binaries.

The test workflow is intended to prepare the github runners for the test execution, which includes capturing the latest EIM tag (to compare with the version printed by the application), installing a supported version of python, setting up environmental variables used by the tests and calling the node test script.

Note that for the Windows runners, one of the runners are set to remove git and python from path, so EIM installs a new version using scoop.

The node scripts are called passing a parameter, which is the test suit json file name, by using this approach it is possible to use the same script for multiple tests suites, just specifying which script should be used for the test.

Examples of node scripts being called with a test suite specification are:
```
npm run test-CLI --file=CLI-prerequisites
npm run test-CLI --file=CLI-pythoncheck
npm run test-CLI --file=CLI-basic
npm run test-CLI --file=CLI-extended
```

The GitHub workflow finishes by uploading the log files to the run page, along with a dashboard to help visualize the test results.

### Node scripts

The node scripts are specified in the package.json file.
Each script will call a different test runner, the test-CLI script will execute CLI tests, test-GUI for the selenium/tauri GUI tests, and the test-offline specifically for the offline test runner.

Note that there are a windows specific version for the CLI and GUI scripts, this is only necessary to handle the parameter --file=<test script file>, which is different from bash and powershell, so a dedicated script is needed, but the actual test runner is the same.

The test script also specifies a report file to be saved with the same name as the test suite file being used, these files are later exported as artifacts from the workflow run.


### Test suites json files

The test files specified on the node scripts are found in the suites folder. This files are used to identify which tests should be run and the test conditions. Multiple files can be created with combination of scenarios and settings, each entry in the json file represents a different test scenario.

The key `type` is used to distinguish the test that will be run.

The test runners will read these files and execute the tests in order, providing results for each test type.

Just by adding or removing entries to the suites json files it is possible to modify the test coverage and the scenarios to be tested.


### Test Runners

This is the first entry point for the actual test execution. Up to this point all files were used to specify which tests to run, and the sequence of execution, but the test runners are the actual script that will run each test.

These are all written in javascript powered by Mocha framework. By definition Mocha will read all the tests that will need to be executed and start running them based on the `describe` priority, note that by default Mocha would try to run all tests in parallel, since we need a specific test sequency and can't handle parallel testing on the same machine, we use `describe` nesting, so Mocha can only proceed to the next test block after the previous one is finished.

The test Runners have teh task to read the environmental variables, setup the test conditions (read configs from the supporting config.js file), read the test suite json file and control the execution of each test script (do not mix the node scripts defined in the package.json with the test scripts which are single test sequencies found on the scripts folder)

```
Breaking down to an example:

The github workflow will call for the test-cli script pointing to the CLI-basic test suite.
The CLI runner is then executed and will read the CLI-basic.json fle

Inside the file there is a custom test scenario of the type `custom`
  {
    "id": 3,
    "type": "custom",
    "name": "CLI Non-Interactive Default",
    "data": {},
    "deleteAfterTest": false,
    "testProxyMode": false
  }

The test runner will read the type custom and all the other test scenario configurations provided by the json file.
On the `custom`test type there are several test parameters that are set by the runner to compile the entire test scenario which will then execute the following test scripts:
runCLICustomInstallTest()
runInstallVerification()
runCleanUP()

Mocha keeps track of the results for each test script and export it all together once the entire suite is executed.
```

### Test Scripts

Each test script is designed to cover a feature or Installation Manager use case. Examples are to run arguments tests, or to run a IDF installation using the wizard, or executing a unattended installation, or verifying the IDF installation is correct, ...

Each test script has its own set of parameters that should be passed by the test runner in order to properly execute the test, some parameters have default values but not all.

The scripts makes use of helper functions shared among multiple scripts, these are written in the helper.js file.

The test scripts represents the last nested Mocha describe function, and represents a single chunck of test. The test statements are written in chai, which is the most common statement validation used with Mocha.

Both CLI and GUI test scripts follows the same concepts, where the CLI will make use of the CLITestRunner class, and the GUI will use the GUITestRunner class.

For new use cases or application use it is recommended to create a new test script, instead of modifying the test script to test multiple scenarios. The only exception was to use the CLICustomInstall for the offline installation test, since most of the code was duplicated, and the offline archive is just a parameter of a custom unattended installation.


### CLI and GUI test classes

The test classes provided the object environment where the tests are executed, and the methods to interact with the applications.

The CLITestRunner class makes use of node-pty for terminal emulation, this is the same library used by VSCode to run emulated terminals. The node-pty is used to launch a terminal instance where EIM could be executed and interacted with by sending text strings are commands and monitoring the object output for the results of these commands.  
Node-pty was chosen apart of running a child process as it has a better stability and it is easier to interact with it. A child process is great to execute single tasks that is soon killed when completed, on the opposite, the node-pty instance is left open until the close command is sent.  
Each method has its functionality described in the class file.

The GUITestRunner class uses Selenium Webdriver in conjunction with Tauri-Driver. Tauri Driver interacts with websocket on the machine that it is running, so it works great on Windows and Linux, but there is no way to test the GUI on MacOS at this point. Other frameworks intended to test web applications cannot interact with a tauri app running on the Operating system, so the only functional option was Selenium
Similar to the CLI class, several methods were added to simplify selection of elements on the interface and click executions. The Tauri interface is basically a web app written in Vue, so selecting elements and navigating through the GUI is similar to navigating a web application.


### Supporting files

The config.js and helper.js files are shared accros all runners and test scripts.   
The Config file works as a central configuration interface, where all teh default values are defined, and all the code to read the environmental variables are intended to live in this file.  
The helper file contains assisting functions that can be shared among different test scripts. Each function has a description of what it can be used for.


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


## Debugging from the terminal

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

The GUI application can be launched from a node running in the terminal. Note that the actual GUI should be rendered in the machine and commands send on the node terminal are visible on the interface.

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
