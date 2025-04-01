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
```

### Using Configuration Files

For reproducible installations, use a configuration file:

```bash
eim install --config path/to/config.toml
```

### Managing Installations

You can also use other commands:

```bash
# List installed versions
eim list

# Select a specific version
eim select v5.3.2

# Remove a specific version
eim remove v5.3.2
```

## CI/CD Integration

### GitHub Actions

Use the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action) for GitHub workflows:

```yaml
steps:
  - uses: actions/checkout@v4
  - name: Install ESP-IDF
    uses: espressif/install-esp-idf-action@v1
    with:
      version: "v5.0"
      path: "/custom/path/to/esp-idf"
      tools-path: "/custom/path/to/tools"
```

### Docker Integration

Example Dockerfile using EIM:

```Dockerfile
FROM bitnami/minideb:bookworm

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Install prerequisites
RUN apt update && apt install -y git cmake ninja-build wget flex bison gperf ccache \
    libffi-dev libssl-dev dfu-util libusb-1.0-0 python3 python3-pip \
    python3-setuptools python3-wheel xz-utils unzip python3-venv && \
    rm -rf /var/lib/apt/lists/*

# Download and install EIM
ARG TARGETARCH
RUN set -x && \
    EIM_BINARY="eim-v0.1.6-linux-" && \
    if [ "$TARGETARCH" = "amd64" ]; then \
        EIM_BINARY="${EIM_BINARY}x64.zip"; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
        EIM_BINARY="${EIM_BINARY}arm64.zip"; \
    else \
        echo "Unsupported architecture: ${TARGETARCH}" && exit 1; \
    fi && \
    wget "https://github.com/espressif/idf-im-cli/releases/download/v0.1.6/${EIM_BINARY}" -O /tmp/eim.zip && \
    unzip /tmp/eim.zip -d /usr/local/bin && \
    chmod +x /usr/local/bin/eim && \
    rm /tmp/eim.zip

# Install ESP-IDF
RUN eim install -i v5.3.2

WORKDIR /workspace
ENTRYPOINT ["/bin/bash", "-c", "source /root/.espressif/activate_idf_v5.3.2.sh && $0 $@"]
```

## Best Practices

1. **Version Control**: Always specify the ESP-IDF version explicitly to ensure reproducible builds
2. **Configuration Files**: Use configuration files for complex setups to ensure consistency
3. **Error Handling**: In CI/CD environments, ensure proper error handling and logging
4. **Prerequisites**: On Windows, use `-a true` to automatically install prerequisites
5. **Path Management**: Use absolute paths to avoid any ambiguity