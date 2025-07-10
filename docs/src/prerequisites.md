# Prerequisites

Below are the minimum requirements for running the ESP-IDF. The Installation Manager itself has no dependencies, but during its run, it checks the system to ensure the dependencies of IDF are met.

## Windows

To get started with ESP-IDF, you need Git and Python. The ESP-IDF Installation Manager will verify the required prerequisites on your system and install any that are missing.

For more details about ESP-IDF prerequisites, please refer to [the ESP-IDF documentation](https://docs.espressif.com/projects/esp-idf/en/v4.2.2/esp32/get-started/windows-setup.html).

> **Note**
> If any of these prerequisites are missing, the installer will prompt you to install them. If you agree, the installer will automatically install and configure everything required to run ESP-IDF.

## MacOS

- dfu-util
- Python with pip capable of creating virtual environments and handling SSL requests

> **Note**
> On POSIX systems, the installer will check for the required prerequisites. If they are not met, the installation will not proceed.

## Linux

- git
- wget
- flex
- bison
- gperf
- ccache
- libffi-dev
- libssl-dev
- dfu-util
- libusb-1.0-0
- Python with pip capable of creating virtual environments and handling SSL requests

> **Note**
> On POSIX systems, the installer will check for the required prerequisites. If they are not met, the installation will not proceed.
