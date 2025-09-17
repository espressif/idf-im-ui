# Offline Archive Builder

The ESP-IDF Installation Manager provides a command-line tool called `offline-installer-builder` that allows you to create your own custom offline installation archives. This is useful for distributing a specific version of ESP-IDF with a custom set of tools and configurations within an organization.

## Getting the tool

You can download the `offline-installer-builder` from two locations:

-   **Espressif Download Portal**: [https://dl.espressif.com/dl/eim/?tab=builder](https://dl.espressif.com/dl/eim/?tab=builder)
-   **GitHub Releases**: The tool is included as an asset in the [latest release](https://github.com/espressif/idf-im-ui/releases/latest) of the ESP-IDF Installation Manager. Look for `eim-offline-builder-<your-platform>.zip` (e.g., `eim-offline-builder-x86_64-pc-windows-msvc.zip`).

## Prerequisites

Before using the `offline-installer-builder`, you need to have the following software installed on your system:

-   **uv**: The tool uses `uv` to install a Python distribution and create a virtual environment. You can find installation instructions for `uv` [here](https://github.com/astral-sh/uv).

## Usage

The `offline-installer-builder` is a command-line tool. After downloading and unzipping it, you can run it from your terminal. On Linux and macOS, you may need to make it executable first (e.g., `chmod +x ./offline-installer-builder`).

### Creating an Offline Archive

To create an offline archive, you use the `--create-from-config` option.

```bash
./offline-installer-builder --create-from-config <CONFIG_PATH>
```

-   `<CONFIG_PATH>`: Path to a TOML configuration file that specifies which ESP-IDF versions and tools to include in the archive. You can also use `"default"` to use the default settings, which will create an archive with the latest ESP-IDF version.

The builder will download all the necessary components (ESP-IDF, tools, Python packages) and bundle them into a `.zst` archive file in the current directory.

#### Configuration File

The configuration file uses the same TOML format as the main installer. Here is an example:

```toml
# Example config.toml for the offline archive builder
idf_versions = ["v5.1", "v5.0.2"]
target = ["esp32", "esp32s3"]
mirror = "https://github.com"
```

This configuration will create an archive containing ESP-IDF versions `v5.1` and `v5.0.2`, with tools for `esp32` and `esp32s3` targets.

### Command-Line Options

Here are the available command-line options for the `offline-installer-builder`:

-   `-c, --create-from-config <CONFIG>`: Create installation data from a specified configuration file. Use `"default"` to use default settings.
-   `-a, --archive <FILE>`: Extract an existing `.zst` archive for inspection or debugging.
-   `-p, --python-version <VERSION>`: Specify the Python version to be included in the archive. The default is `3.11`.
-   `-v, --verbose`: Increase the verbosity of the output for debugging purposes. Can be used multiple times (e.g., `-vv`).

### Inspecting an Archive

If you want to see the contents of an existing offline archive, you can use the `--archive` option:

```bash
./offline-installer-builder --archive <PATH_TO_ARCHIVE.zst>
```

This will extract the contents of the `.zst` file into a new directory named `<ARCHIVE_NAME>_extracted`, allowing you to inspect the bundled files.
