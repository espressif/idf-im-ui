# Headless Usage

The ESP-IDF Installation Manager supports headless mode for automated installations, particularly useful for CI/CD pipelines, Docker environments, and automated deployments.

## Basic Headless Usage

To run the installer in headless mode, use the `-n` or `--non-interactive` flag:

```bash
eim -n true
```

This will install the latest version of ESP-IDF with default settings.

## Advanced Headless Usage

### Custom Installation Options

Combine the non-interactive flag with other parameters:

```bash
# Install specific version
eim -n true -i v5.3.2

# Custom installation path
eim -n true -p /opt/esp-idf

# Install prerequisites (Windows only)
eim -n true -a true
```

### Using Configuration Files

For reproducible installations, use a configuration file:

```bash
eim -n true --config path/to/config.toml
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
RUN eim -n true -i v5.3.2

WORKDIR /workspace
ENTRYPOINT ["/bin/bash", "-c", "source /root/.espressif/activate_idf_v5.3.2.sh && $0 $@"]
```

## Best Practices

1. **Version Control**: Always specify the ESP-IDF version explicitly to ensure reproducible builds
2. **Configuration Files**: Use configuration files for complex setups to ensure consistency
3. **Error Handling**: In CI/CD environments, ensure proper error handling and logging
4. **Prerequisites**: On Windows, use `-a true` to automatically install prerequisites
5. **Path Management**: Use absolute paths to avoid any ambiguity 