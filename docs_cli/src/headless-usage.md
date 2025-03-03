# Headless Usage

The ESP-IDF Installation Manager (EIM) supports headless mode, which allows you to run the installer without any user interaction. This is particularly useful for automated setups, CI/CD pipelines, and Docker environments.

---

## Running the Installer in Headless Mode

To run the installer in headless mode, use the -n or --non-interactive flag. This will skip all prompts and use default values for the installation. You can still customize the installation by providing additional command-line arguments

### Basic Headless Installation

To perform a basic headless installation with default settings, run:

```bash
./eim -n true
```

This will install the latest version of ESP-IDF in the default installation path (`C:\esp` on Windows or `~/.espressif` on macOS/Linux).

---

### Customizing the Headless Installation

You can combine the `--non-interactive` flag with other parameters to customize the installation. For example:

- **Install a Specific IDF Version:** Use the `-i` or `--idf-version` flag to specify the version of ESP-IDF to install.

```bash
./eim -n true -i v5.3.2
```

- **Install All Prerequisites Automatically:** Use the -a or --install-prerequisites flag to automatically install any missing prerequisites.

```bash
./eim -n true -a true
```

> **Note:** The prerequisities installation is currently only supported on Windows.

- **Specify an Installation Path:** Use the -p or --path flag to specify a custom installation path.

```bash
./eim -n true -p /opt/esp-idf
```

## Using EIM in GitHub CI/CD Pipelines

The ESP-IDF Installation Manager can be integrated into GitHub CI/CD workflows using the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action). This GitHub Action allows you to install ESP-IDF (or even build IDF projects) on Windows, macOS, and Linux platforms.

### Example GitHub Workflow

Basic usage with default settings:

```yaml
steps:
  - uses: actions/checkout@v4
  - name: Install ESP-IDF
    uses: espressif/install-esp-idf-action@v1
  - name: Build your project
    run: |
      idf.py build
```

Advanced usage with custom configuration:

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

## Using EIM in Docker

The headless mode of EIM is ideal for installing ESP-IDF inside Docker containers. This allows you to create reproducible development environments or build environments for CI/CD pipelines.

### Example Dockerfile

```Dockerfile
FROM bitnami/minideb:bookworm

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN install_packages git cmake ninja-build wget flex bison gperf ccache \
    libffi-dev libssl-dev dfu-util libusb-1.0-0 python3 python3-pip \
    python3-setuptools python3-wheel xz-utils unzip python3-venv && \
    rm -rf /var/lib/apt/lists/*

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
    echo "Downloading ${EIM_BINARY}" && \
    wget "https://github.com/espressif/idf-im-cli/releases/download/v0.1.6/${EIM_BINARY}" -O /tmp/eim.zip && \
    unzip /tmp/eim.zip -d /usr/local/bin && \
    chmod +x /usr/local/bin/eim && \
    rm /tmp/eim.zip

RUN eim -n true -i v5.3.2

RUN mkdir /tmp/project
WORKDIR /tmp/project

ENTRYPOINT ["/bin/bash", "-c", "source /root/.espressif/activate_idf_v5.3.1.sh && python3 /root/.espressif/v5.3.1/esp-idf/tools/idf.py build"]
```

# Summary

- Use `eim -n true` or `--non-interactive` for headless installations.
- Combine with other flags like `-i` (IDF version) or `-a` (install prerequisites) for customization.
- Integrate EIM into GitHub CI/CD pipelines using the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action).
- Use EIM in Docker to create reproducible development or build environments.

By leveraging headless mode, you can automate ESP-IDF installations in various environments, making your development workflow more efficient and consistent.
