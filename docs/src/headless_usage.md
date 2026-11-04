# Headless Usage

The ESP-IDF Installation Manager supports headless mode for automated installations, particularly useful for CI/CD pipelines, Docker environments, and automated deployments.

## Basic Headless Usage

The `install` command runs in non-interactive (headless) mode by default. You don't need to explicitly specify the `-n` or `--non-interactive` flag:

```bash
eim install
```

This will install the latest version of ESP-IDF with default settings in non-interactive mode.

If you want to run the install command in interactive mode, you would need to explicitly specify:

```bash
eim install -n false
```

## Advanced Headless Usage

### Custom Installation Options

Use the install command with various parameters:

```bash
# Install specific version
eim install -i v5.3.2

# Custom installation path
eim install -p /opt/esp-idf

# Install prerequisites (Windows only)
eim install -a true

# Clean up tool archives after installation (reduces final image size)
eim install --cleanup true
```

### Reducing Docker Image Size

For Docker and CI environments, use the `--cleanup` flag to remove tool archive files after installation:

```bash
# Clean up tool archives after installation (reduces final image size)
eim install --cleanup true
```

This removes the `dist` directory containing downloaded tool archives, which can save significant disk space in environments where no further installations are expected. This is particularly useful for:

- **Docker images**: Reduces final image size
- **CI/CD pipelines**: Clean up after successful installation
- **Single-use environments**: Where archives won't be reused

### Using Configuration Files

For reproducible installations, use a configuration file:

```bash
eim install --config path/to/config.toml
```

Configuration files support tool selection in addition to other settings:

```toml
# Example configuration with tool selection
idf_versions = ["v5.3.2", "v5.4"]

# Global tools for all versions
idf_tools = ["cmake", "openocd"]

# Or per-version tool selection
[idf_tools_per_version]
"v5.3.2" = ["cmake", "openocd"]
"v5.4" = ["cmake", "openocd", "idf-exe"]
```

### Managing Installations

You can also use other commands:

```bash
# List installed versions
eim list

# List the tools for an installed IDF and their on-disk status
eim list-tools v5.3.2

# Show only tools whose on-disk version is older than what tools.json declares
eim list-tools v5.3.2 --outdated

# Select a specific version
eim select v5.3.2

# Remove a specific version
eim remove v5.3.2
```

When you install an ESP-IDF version, EIM also writes a matching **deactivation** script next to the activation one. Inside a single shell session you can use it to undo the activation without removing the installation:

```bash
# Activate the toolchain for v5.3.2
. /root/.espressif/tools/activate_idf_v5.3.2.sh

# ... build / flash / etc ...

# Undo the activation in the same shell (no need to exit)
. /root/.espressif/tools/deactivate_idf_v5.3.2.sh
```

The deactivation script is removed automatically when the corresponding version is uninstalled via `eim remove`. See [After Installing](./after_installing.md#deactivating-the-environment) for the file names and the variables each deactivation script unsets.

## CI/CD Integration

### GitHub Actions

Use the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action) for GitHub workflows:

```yaml
steps:
  - uses: actions/checkout@v6
  - name: Install ESP-IDF
    uses: espressif/install-esp-idf-action@v1
    with:
      version: "v5.0"
      path: "/custom/path/to/esp-idf"
      tools-path: "/custom/path/to/tools"
```

### Docker Integration

> **Note on Python in Docker:** The example Dockerfile below installs the default `python3` package. On Debian Bookworm, this is currently a supported version (Python 3.11). If you are using a different base image, ensure that the installed Python version is one of 3.10, 3.11, 3.12, 3.13, or 3.14. Python 3.14 is now fully supported on Linux, macOS, and Windows.

Example Dockerfile using EIM:

```Dockerfile
# syntax=docker/dockerfile:1

FROM bitnami/minideb:bookworm

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Install required packages including curl and jq for API handling
RUN install_packages git wget flex bison gperf ccache \
    libffi-dev libssl-dev dfu-util libusb-1.0-0 python3 python3-pip \
    python3-setuptools python3-wheel xz-utils unzip python3-venv curl jq && \
    rm -rf /var/lib/apt/lists/*

ARG TARGETARCH=arm64
RUN set -x && \
    # Get the latest release info
    LATEST_RELEASE=$(curl -s https://api.github.com/repos/espressif/idf-im-ui/releases/latest) && \
    # Determine correct architecture name for asset pattern
    if [ "$TARGETARCH" = "amd64" ]; then \
        ARCH_PATTERN="linux-x64"; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
        ARCH_PATTERN="linux-aarch64"; \
    else \
        echo "Unsupported architecture: ${TARGETARCH}" && exit 1; \
    fi && \
    # Extract download URL for the CLI tool
    EIM_DOWNLOAD_URL=$(echo "$LATEST_RELEASE" | jq -r --arg PATTERN "eim-cli-$ARCH_PATTERN.zip" \
        '.assets[] | select(.name | contains($PATTERN)) | .browser_download_url') && \
    # Verify a URL was found
    if [ -z "$EIM_DOWNLOAD_URL" ]; then \
        echo "Failed to find download URL for eim-cli-$ARCH_PATTERN.zip" && exit 1; \
    fi && \
    echo "Downloading eim-cli from: $EIM_DOWNLOAD_URL" && \
    # Download and extract
    wget "$EIM_DOWNLOAD_URL" -O /tmp/eim.zip && \
    unzip /tmp/eim.zip -d /tmp/eim && \
    # Find and move the eim binary (handles possible subdirectory in zip)
    find /tmp/eim -name "eim" -type f -exec cp {} /usr/local/bin/eim \; && \
    # Make executable
    chmod +x /usr/local/bin/eim && \
    # Cleanup
    rm -rf /tmp/eim.zip /tmp/eim

RUN eim install -i v5.3.1 -n true -a true -r false --cleanup true

RUN mkdir /tmp/project
WORKDIR /tmp/project

ENTRYPOINT ["/bin/bash", "-c", "source /root/.espressif/tools/activate_idf_v5.3.1.sh && python3 /root/.espressif/v5.3.1/esp-idf/tools/idf.py build"]
```

## Custom Repository Configuration

When installing from custom repositories, you can use the following options:

1. **For GitHub repositories**: Only the `--repo-stub` parameter is needed to specify the repository name:
```bash
eim install -i v5.3.2 --repo-stub my-github-user/my-custom-idf
```

2. **For completely custom repositories** (like GitLab or self-hosted): Use both `--mirror` and `--repo-stub` parameters:
```bash
eim install -i v5.3.2 --mirror https://gitlab.example.com --repo-stub my-gitlab-user/my-custom-idf
```

The mirror parameter should point to the root URL of your repository host, while repo-stub specifies the repository path.

## Failure handling

- Exit codes are `0` (success) and `1` (any failure). The code does not say which failure it was.
- On failure, find the line that starts with `Error executing CLI:`. The lines after it are the specific error's `Fix:` and `Docs:`, then the generic E900 block. The last stderr line is the E900 link. stdout uses `[FAIL]` and `Hint:` for Python and prerequisite checks.
- In `eim.log`, `Fix:` and `Docs:` follow the `ERROR` or `WARN` line they belong to and have no timestamp prefix. Read up to the next timestamped line. `Fix:` and `Docs:` are the last two lines of that block. `grep -A10 ERROR` is a wider example and can still miss a long inserted tail. A `Fix:` on a `WARN` line is conditional. Messages keep their original sentence, then `Fix:` and `Docs:` with an `EIM-E` ID. The IDs are listed in [Troubleshooting](./troubleshooting.md).
- CLI log: `<data dir>/eim/logs/eim.log` (`eim_gui.log` for the GUI, `offline_installer.log` for the offline archive builder). `--log-file` sets the CLI path. `-v` repeats to print more on stderr.
- `eim help-json` prints machine-readable help and does not open the log.
- `--do-not-track true`, or `EIM_DO_NOT_TRACK=1` / `EIM_DO_NOT_TRACK=true`, turns telemetry off.
- On Linux and macOS, prerequisites are never installed for you. `-a` / `--install-all-prerequisites` installs Git and Python on Windows only.
- Script, CI, and agent recovery steps are in [Using EIM from scripts, CI and AI agents](./scripts_ci_and_agents.md).

## Best Practices

1. **Version Control**: Always specify the ESP-IDF version explicitly to ensure reproducible builds
2. **Configuration Files**: Use configuration files for complex setups to ensure consistency
3. **Error Handling**: On failure, read from the line that starts with `Error executing CLI:` through the following `Fix:` and `Docs:` lines, plus `[FAIL]` and `Hint:` on stdout. In `eim.log`, read from the `ERROR` or `WARN` line up to the next timestamp. `grep -A10 ERROR` is only an example. Match the specific `EIM-E` ID in [Troubleshooting](./troubleshooting.md)
4. **Prerequisites**: On Windows, use `-a true` to automatically install prerequisites. On Linux and macOS, install them yourself before `eim` runs
5. **Path Management**: Use absolute paths to avoid any ambiguity
6. **Cleanup**: Use `--cleanup true` in Docker and CI environments to reduce image size by removing temporary tool archives
