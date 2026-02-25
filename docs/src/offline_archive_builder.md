# Offline Archive Builder

The ESP-IDF Installation Manager provides a command-line tool called `offline-installer-builder` that allows you to create **custom offline installation archives** for specific or all ESP-IDF versions. These archives contain everything needed for offline installation — ESP-IDF source, tools, Python wheels, and prerequisites — making them ideal for air-gapped environments, enterprise deployment, or CI/CD pipelines.

## Getting the Tool

You can download the `offline-installer-builder` from:

-   **GitHub Releases**: The tool is included as an asset in the [latest release](https://github.com/espressif/idf-im-ui/releases/latest) of the ESP-IDF Installation Manager. Look for `offline_installer_builder-<platform>-*.zip` (e.g., `offline_installer_builder-windows-x64-*.zip`).

## Prerequisites

Before using the `offline-installer-builder`, ensure the following is installed:

-   **uv**: A fast Python package and project manager. The builder uses `uv` to install Python and manage virtual environments.
    - Install from: https://github.com/astral-sh/uv
    - Verify with: `uv --version`

> 💡 The builder does **not** bundle `uv` — you must install it separately.

> **Note on Python versions:** ESP-IDF supports Python versions 3.10, 3.11, 3.12, 3.13, and 3.14. Python 3.14 is supported on Linux and macOS only; Windows does not support Python 3.14 because ESP-IDF dependencies do not yet support it. When building an offline archive, you can specify which Python versions to include wheels for. See the `--python-version` and `--wheel-python-versions` options for more details.

---

## Usage

After downloading and extracting the builder, run it from your terminal. On Linux/macOS, make it executable first:

```bash
chmod +x ./offline_installer_builder
```

On Windows, use `offline_installer_builder.exe`.

---

### Creating an Offline Archive

#### 1. Build Archive for a Specific IDF Version

```bash
./offline_installer_builder -c default --idf-version-override v5.1.2
```

This creates a single `.zst` archive for version `v5.1.2`, named like:
`archive_v5.1.2_<platform>.zst` (e.g., `archive_v5.1.2_linux-x64.zst`)

#### 2. Build Archives for All Supported IDF Versions

```bash
./offline_installer_builder -c default --build-all-versions
```

This builds **one `.zst` archive per supported IDF version**, each named with its version and platform.

> ✅ Use this for full offline mirror creation.

#### 3. Build Using a Custom Configuration File

```bash
./offline_installer_builder -c /path/to/your/config.toml
```

Your `config.toml` can specify which versions and targets to include. Example:

```toml
# config.toml
idf_versions = ["v5.0.4", "v5.1.2"]
target = ["esp32", "esp32s3", "esp32c3"]
mirror = "https://github.com"
tools_json_file = "tools/tools.json"
```

> ⚠️ When using `--build-all-versions` or `--idf-version-override`, the `idf_versions` field in your config is **ignored**.

---

### Command-Line Options

| Short | Long | Description |
|-------|------|-------------|
| `-c` | `--create-from-config <CONFIG>` | Create archive from TOML config. Use `"default"` for defaults. |
| `-a` | `--archive <FILE>` | Extract a `.zst` archive for inspection. |
| `-p` | `--python-version <VERSION>` | Python version to bundle (default: `3.11`). |
| `--wheel-python-versions <V1,V2,...>` | Comma-separated Python versions for which to download wheels (e.g., `3.10,3.11,3.12,3.14`). Defaults to all supported on POSIX, single version on Windows. |
| `--idf-version-override <VERSION>` | Build archive for **only** this IDF version (e.g., `v5.1.2`). |
| `--build-all-versions` | Build **separate archives for all** supported IDF versions. |
| `-v` | `--verbose` | Increase log verbosity (use `-vv` or `-vvv` for more detail). |

---

### Inspecting an Archive

To examine the contents of a `.zst` archive:

```bash
./offline_installer_builder --archive archive_v5.1.2_linux-x64.zst
```

This extracts the archive into a directory named `archive_v5.1.2_linux-x64.zst_extracted/`.

Useful for debugging or verifying contents.

---

## Output

The builder generates `.zst` archives in the **current working directory**. Each archive is self-contained and can be used with:

```bash
eim install --use-local-archive archive_v5.1.2_linux-x64.zst
```

> 📁 **Archive structure** (when extracted):
> ```
> ├── esp-idf/           # ESP-IDF source
> ├── dist/              # Downloaded tools (xtensa-esp-elf, etc.)
> ├── python_env_*/      # Python virtual environments
> ├── wheels_py*/        # Pre-downloaded Python wheels
> ├── scoop/             # (Windows only) Offline Scoop prerequisites
> └── config.toml        # Configuration used to build this archive
> ```

---

## Advanced Usage

### Building for CI/CD or Release Automation

You can integrate the builder into GitHub Actions or other CI systems. Example workflow:

```yaml
- name: Build all versions
  run: |
    ./offline_installer_builder -c default --build-all-versions
    mkdir -p artifacts
    mv archive_v* artifacts/
```

Each platform (Linux, Windows, macOS) must run the builder separately — archives are platform-specific.

---

### Python Wheel Compatibility

By default, the builder downloads wheels for multiple Python versions to maximize compatibility. You can override this:

```bash
./offline_installer_builder -c default --idf-version-override v5.1.2 --wheel-python-versions 3.11,3.12,3.14
```

This ensures the archive includes wheels compatible with Python 3.11, 3.12, and 3.14 on Linux and macOS.

---

## Troubleshooting

-   **“UV not found”**: Install `uv` first. See https://github.com/astral-sh/uv
-   **No archives generated**: Check logs (`offline_installer.log`). Ensure network connectivity and disk space.
-   **Checksum failures**: Retry or check mirror URLs in config.
-   **Windows prerequisites fail**: Ensure you’re running in an environment with internet access during build.
