# Command Line Installation

The command line interface (CLI) of ESP-IDF Installation Manager provides a flexible way to install ESP-IDF, especially useful for automation and headless environments.

## Getting Started

### Windows
Run EIM from PowerShell (do not use x86 version). Navigate to the EIM directory and run:
```bash
.\eim --help
```

### macOS & Linux
After downloading and extracting EIM, make it executable:
```bash
chmod +x ./eim
./eim --help
```

## Installation Methods

### Interactive Wizard
Running `eim wizard` starts an interactive wizard that guides you through the installation:

1. Prerequisites check
2. Platform selection
3. ESP-IDF version selection
4. Mirror selection
5. Installation path configuration

### Command Line Arguments
For automated installations, use command line arguments:

```bash
# Install specific version
eim install -i v5.3.2

# Install with custom path
eim install -p /opt/esp-idf

# Non-interactive installation
eim install -n true
```

### Important Note on Installation Path and Version:
If the path provided for installation (e.g., via `-p` or in the interactive wizard) is a valid, existing ESP-IDF Git repository, EIM will use that repository directly and will not overwrite its contents. In such a scenario, any ESP-IDF version specified through other parameters (e.g., `-i`for a specific version) will be ignored, as the installer will work with the version already present in the existing repository.

See [CLI Configuration](./cli_configuration.md) for all available options.
