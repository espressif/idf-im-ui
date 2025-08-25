# Prerequisites

Below are the minimum requirements for running the ESP-IDF. The Installation Manager itself has no dependencies, but during its run, it checks the system to ensure the dependencies of IDF are met.

## Windows

To get started with ESP-IDF, you need Git and Python. The ESP-IDF Installation Manager will verify the required prerequisites on your system and install any that are missing.

For more details about ESP-IDF prerequisites, please refer to [the ESP-IDF documentation](https://docs.espressif.com/projects/esp-idf/en/v4.2.2/esp32/get-started/windows-setup.html).

> **Note**
> If any of these prerequisites are missing, the installer will prompt you to install them. If you agree, the installer will automatically install and configure everything required to run ESP-IDF.
> For offline installations, **Python 3.11** is required.

## MacOS

- `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
- `glib`: Runtime library for GLib (QEMU dependency).
- `pixman`: Runtime library for pixman (QEMU dependency).
- `sdl2`: Runtime library for SDL2 (QEMU dependency).
- `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).
- dfu-util
- Python with pip capable of creating virtual environments and handling SSL requests

> **Note**
> On POSIX systems, the installer will check for the required prerequisites. If they are not met, the installation will not proceed.
> For offline installations, **Python 3.11** is required.

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

### Other Linux prerequisites based on distro

#### Debian/Ubuntu

- `libffi-dev`: Development headers for Foreign Function Interface.
- `libusb-1.0-0`: Runtime library for USB device access.
- `libssl-dev`: Development headers for OpenSSL (SSL/TLS cryptography).
- `libgcrypt20`: Runtime library for cryptographic functions (QEMU dependency).
- `libglib2.0-0`: Runtime library for GLib (QEMU dependency).
- `libpixman-1-0`: Runtime library for pixman (QEMU dependency).
- `libsdl2-2.0-0`: Runtime library for SDL2 (QEMU dependency).
- `libslirp0`: Runtime library for SLIRP user-mode networking (QEMU dependency).

#### Fedora/RHEL/CentOS

- `libffi-devel`: Development headers for Foreign Function Interface.
- `libusb`: Runtime library for USB device access.
- `openssl-devel`: Development headers for OpenSSL.
- `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
- `glib2`: Runtime library for GLib (QEMU dependency).
- `pixman`: Runtime library for pixman (QEMU dependency).
- `SDL2`: Runtime library for SDL2 (QEMU dependency).
- `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).

#### Arch Linux

- `libusb`: Includes both runtime and development files for USB access.
- `libffi`: Includes both runtime and development files for Foreign Function Interface.
- `openssl`: Includes both runtime and development files for OpenSSL.
- `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
- `glib`: Runtime library for GLib (QEMU dependency).
- `pixman`: Runtime library for pixman (QEMU dependency).
- `sdl2`: Runtime library for SDL2 (QEMU dependency).
- `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).

#### openSUSE/SUSE Linux Enterprise

- `libusb-1_0-0`: Runtime library for USB device access.
- `libffi-devel`: Development headers for Foreign Function Interface.
- `libopenssl-devel`: Development headers for OpenSSL.
- `libgcrypt`: Runtime library for cryptographic functions (QEMU dependency).
- `glib2`: Runtime library for GLib (QEMU dependency).
- `pixman-1`: Runtime library for pixman (QEMU dependency).
- `libsdl2-2_0_0`: Runtime library for SDL2 (QEMU dependency).
- `libslirp`: Runtime library for SLIRP user-mode networking (QEMU dependency).

> **Note**
> On POSIX systems, the installer will check for the required prerequisites. If they are not met, the installation will not proceed unless `--skip-prerequisites-check` is used. In that case it's user's own responsibility to have all the needed prerequisites already there. For offline installations, **Python 3.11** is required.
