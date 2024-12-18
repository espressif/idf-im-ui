# ESP-IDF Installation Manager(EIM) - GUI automated tests

## Concepts

The test infrastructure for the EIM GUI interface should focus on the customer experience with the GUI to provide a consistent experience for users across multiple operating systems and multiple software versions.

Tests should make use of the Tauri-Driver, an interface specially designed to interact with the GUI application through the Webdriver protocol.

## Environment Setup

Tests are developed in Node.js and Selenium-Webdriver, it is required to have Node.js installed on the machine where the tests are executed.

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
If changing path note that thi sis not a permanent change and need to be redone for every new shell, or added to the $profile
`$env:Path += ';C:\<your_folder>'`
`$env:Path += ';'+$env:USERPROFILE+'\EdgeDriver'`

Install the tauri-driver using Cargo:  
`cargo install tauri-driver`

## Running the tests

Test scripts are created to allow launching and running the tests. These scripts do not build the Tauri application, it is necessary to have them compiled before running the tests.

Tests can be executed using npm scripts. Navigate to the `/tests` folder and run:
`npx mocha scripts/startup.test.js`
`npx mocha scripts/defaultInstall.test.js`

TODO: The path for the compiled application can be passed as an argument for the test script. The default location used by the scripts is `~/eim-gui/eim`, make sure the file is available in the location.

To execute tests on windows, use the script  
`.\tests\run_test.ps1 "<PATH TO EIM.EXE>" "<Version being tested>"`  
Default arguments are:  
`.\tests\run_test.ps1 "$env:USERPROFILE\eim-gui\eim.exe" "0.1.0"`

## References
