# EIM Maintenance Guide

This document provides comprehensive guidance for maintaining the various distribution components of the ESP-IDF Installation Manager (EIM). It covers all external repositories, package managers, and documentation that need periodic review and updates.

## Table of Contents

- [Overview](#overview)
  - [Release and Workflow Overview](#release-and-workflow-overview)
- [1. Scoop Manifests for Offline Installer](#1-scoop-manifests-for-offline-installer)
- [2. Scoop Installer PowerShell Scripts](#2-scoop-installer-powershell-scripts)
- [3. Docker Repository](#3-docker-repository)
- [4. GitHub Install Action](#4-github-install-action)
- [5. Homebrew EIM](#5-homebrew-eim)
- [6. TLDR Pages Entry](#6-tldr-pages-entry)
- [7. Man Page](#7-man-page)
- [8. APT Repository](#8-apt-repository)
- [9. RPM Repository](#9-rpm-repository)
- [10. WinGet](#10-winget)
- [11. Scoop Distribution (Online)](#11-scoop-distribution-online)
- [Maintenance Checklists](#maintenance-checklists)
- [Secrets Reference](#secrets-reference)

---

## Overview

EIM is distributed through multiple channels to support different platforms and installation methods. Each channel requires periodic maintenance to ensure compatibility, security, and functionality.

### Distribution Architecture

```
Release Trigger (GitHub Release)
         │
         ▼
    Build Phase (build.yaml)
         │
         ├── CLI Binaries
         ├── GUI Binaries
         ├── .deb packages
         ├── .rpm packages
         ├── .dmg files
         └── .msi installers
         │
         ▼
  Distribution Phase
         │
         ├── update-homebrew.yml ──────► espressif/homebrew-eim
         ├── update-linux-repos.yml ───► APT Repository (S3)
         │                             └► RPM Repository (S3)
         └── update-windows-packages.yml ─► WinGet (microsoft/winget-pkgs)
                                          └► Scoop Manifests (Release assets)
```

### Release and Workflow Overview

Releases are **not** created by CI. A maintainer creates a GitHub Release (e.g. tag `v0.7.1` and "Publish release"). That triggers the following:

1. **Trigger:** `build.yaml` runs when `release.type` is `created` (`.github/workflows/build.yaml`).

2. **Build jobs:** `build-cli`, `build-cli-linux`, and `build-gui` build binaries for all platforms. Each job uploads **artifacts** (e.g. `eim-cli-windows-x64-v0.7.1`) and, on release, uploads the same files as **release assets** to the GitHub Release.

3. **Offline archives:** The job `build-offline-archives` calls `build_offline_installer_archives.yaml`. It uses the `offline_installer_builder` binary (built in `build-cli`/`build-cli-linux`) and the **scoop manifest templates** (compiled into that binary) to build offline archives and upload them to S3.

4. **Release info:** The job `update-release-info` fetches the latest release JSON and uploads it to S3 (`eim_unified_release.json`).

5. **Distribution workflows:** After `update-release-info`, the main workflow **calls** three reusable workflows with `version: ${{ github.ref_name }}` (e.g. `v0.7.1`):
   - **update-homebrew.yml** — Downloads macOS assets from the **release API**, computes SHA256, updates `espressif/homebrew-eim`.
   - **update-linux-repos.yml** — Downloads **artifacts** from the **same run** (e.g. `eim-gui-linux-x64-v0.7.1-deb`), updates APT and RPM repos on S3.
   - **update-windows-packages.yml** — Downloads **artifacts** from the same run (e.g. `eim-cli-windows-x64-v0.7.1`), generates Scoop manifests, uploads them to the release, then runs WinGet releaser (PR to `microsoft/winget-pkgs`).

Artifacts are shared across jobs in the same workflow run, so distribution jobs use the versioned artifact names (e.g. `eim-cli-windows-x64-v0.7.1`) to download what the build jobs uploaded.

---

## 1. Scoop Manifests for Offline Installer

### Purpose

These JSON manifest templates define how Scoop installs dependencies (7-Zip, Git, Python, etc.) during offline Windows installation. They are bundled into offline archives and processed at runtime.

### Used by Workflows

These files are **not** read by any workflow directly. They are embedded at **compile time** in the Rust binary:

- **build.yaml** — The `build-cli` and `build-cli-linux` jobs build the `offline_installer_builder` binary. That binary is compiled with `include_str!("../../scoop_manifest_templates/7zip.json")` etc. in `src-tauri/src/lib/offline_installer.rs`, so the template contents are baked into the executable.
- **build_offline_installer_archives.yaml** — Runs the `offline_installer_builder` binary to produce offline archives. When a user runs the offline installer, that binary (or the EIM GUI/CLI using the same logic) expands the templates with `{{offline_archive_scoop_dir}}` and uses them to install Scoop dependencies from the archive.

No workflow edits these JSON files; they are maintained in the repo and only affect behaviour when the binary is built and when offline archives are built or used.

### File Locations

| File | Description | Lines |
|------|-------------|-------|
| `src-tauri/scoop_manifest_templates/7zip.json` | 7-Zip archiver (v25.01) | 73 |
| `src-tauri/scoop_manifest_templates/git.json` | Git for Windows (v2.50.1) | 82 |
| `src-tauri/scoop_manifest_templates/python311.json` | Python 3.11.9 | 95 |
| `src-tauri/scoop_manifest_templates/python310.json` | Python 3.10.11 | 93 |
| `src-tauri/scoop_manifest_templates/dark.json` | WiX Toolset Decompiler (v3.14.1) | 10 |

### How It Works

1. Templates are embedded at compile time in `src-tauri/src/lib/offline_installer.rs` (lines 319-344):

```rust
let packages = [
    ScoopPackage {
        name: "7zip",
        template_content: include_str!("../../scoop_manifest_templates/7zip.json"),
        manifest_filename: "7zip.json",
        test_command: "echo 0"
    },
    // ... more packages
];
```

2. The placeholder `{{offline_archive_scoop_dir}}` is replaced at runtime (line 316):

```rust
context.insert("offline_archive_scoop_dir", &scoop_path.to_str().unwrap().replace("\\", "/"));
```

3. Scoop then installs each package using the processed manifest.

### Version Update Procedure

**Example: Updating 7-Zip from v25.01 to v25.02**

1. Check the upstream Scoop bucket for the latest version:
   - URL: https://github.com/ScoopInstaller/Main/blob/master/bucket/7zip.json

2. Update `src-tauri/scoop_manifest_templates/7zip.json`:

```json
{
    "version": "25.02",  // Line 2 - update version
    "architecture": {
        "64bit": {
            "url": "file://{{offline_archive_scoop_dir}}/7z2502-x64.msi",  // Line 9 - update filename
            "hash": "NEW_SHA256_HASH_HERE"  // Line 10 - update hash
        },
        "32bit": {
            "url": "file://{{offline_archive_scoop_dir}}/7z2502.msi",  // Line 14
            "hash": "NEW_SHA256_HASH_HERE"  // Line 15
        }
    }
}
```

3. Download the new binaries and calculate SHA256:

```bash
# Download the new version
curl -LO https://www.7-zip.org/a/7z2502-x64.msi

# Calculate SHA256
sha256sum 7z2502-x64.msi
# or on Windows:
certutil -hashfile 7z2502-x64.msi SHA256
```

4. Update the `autoupdate` URL pattern if the naming convention changed.

### Fetching Upstream Changes

```bash
# Compare with upstream Scoop manifests
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/7zip.json | jq .version
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/git.json | jq .version
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/python.json | jq .version
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/dark.json | jq .version
```

### Important Notes

- The `url` field uses `file://{{offline_archive_scoop_dir}}/...` for offline installation
- Keep `checkver` and `autoupdate` sections for reference, even though offline installs don't use them
- Python manifests include PEP-514 registry entries for Python discovery by other tools
- Test offline installation after any manifest changes

---

## 2. Scoop Installer PowerShell Scripts

### Purpose

These scripts install and configure Scoop package manager on Windows. The offline version allows installation without internet access.

### Used by Workflows

No workflow in this repository **runs** these scripts. They are bundled into the **offline installation archive** when `build_offline_installer_archives.yaml` runs the `offline_installer_builder`. The archive content is then used on a user's Windows machine: the offline installer extracts and runs `install_scoop_offline.ps1` (and may use `install_scoop.ps1` in online flows). So these files are **payload** for the offline archive, not invoked by CI. Changing them only affects future offline archive builds and end-user offline installs.

### File Locations

| File | Description | Lines |
|------|-------------|-------|
| `src-tauri/powershell_scripts/install_scoop_offline.ps1` | Offline Scoop installer | 639 |
| `src-tauri/powershell_scripts/install_scoop.ps1` | Online Scoop installer | 716 |

### Upstream Source

**Official Scoop Installer:** https://github.com/ScoopInstaller/Install/blob/master/install.ps1

### How to Fetch Upstream Changes

```bash
# Download the latest upstream installer
curl -o /tmp/upstream_install.ps1 https://raw.githubusercontent.com/ScoopInstaller/Install/master/install.ps1

# Compare with our online version
diff src-tauri/powershell_scripts/install_scoop.ps1 /tmp/upstream_install.ps1

# Or use a visual diff tool
code --diff src-tauri/powershell_scripts/install_scoop.ps1 /tmp/upstream_install.ps1
```

### Key Functions to Monitor

When syncing with upstream, pay attention to these functions:

| Function | Local Lines | Purpose |
|----------|-------------|---------|
| `Test-Prerequisite` | 148-168 | PowerShell version and TLS checks |
| `Expand-ZipArchive` | 197-235 | Archive extraction logic |
| `Import-ScoopShim` | 264-329 | Shim creation for scoop command |
| `Add-ShimsDirToPath` | 401-423 | PATH configuration |
| `Add-DefaultConfig` | 474-513 | Scoop configuration setup |

### EIM-Specific Additions (Do NOT Remove)

The offline script contains EIM-specific code that must be preserved:

1. **`$OfflineDir` parameter** (line 57):
```powershell
param(
    [String] $ScoopDir,
    [String] $ScoopGlobalDir,
    [String] $ScoopCacheDir,
    [String] $OfflineDir,  # EIM-specific
    [Switch] $RunAsAdmin
)
```

2. **`Test-OfflineFiles` function** (lines 113-140):
```powershell
function Test-OfflineFiles {
    param([String] $OfflineDirectory)
    # Validates scoop-master.zip and main-master.zip exist
    $requiredFiles = @('scoop-master.zip', 'main-master.zip')
    # ...
}
```

3. **`Install-ScoopOffline` function** (lines 523-574):
```powershell
function Install-ScoopOffline {
    param([String] $OfflineDirectory)
    # Extracts from local zip files instead of downloading
    # ...
}
```

### Sync Procedure

1. Download upstream changes
2. Diff with local version
3. Apply relevant changes to `install_scoop.ps1`
4. Port applicable changes to `install_scoop_offline.ps1` while preserving EIM-specific code
5. Test both online and offline installation

---

## 3. Docker Repository

### Used by Workflows

No workflow in **this** repository builds or updates the Docker image. The image lives in a separate repository (Hahihula/eim-idf-build-docker). Optionally, that repo could be triggered on new EIM releases (e.g. via `repository_dispatch` or a manual trigger); currently it is updated manually when EIM or ESP-IDF versions change.

### Repository

**URL:** https://github.com/Hahihula/eim-idf-build-docker

**Docker Hub:** https://hub.docker.com/r/hahihula/eim-idf-build

### Purpose

Provides a proof-of-concept Docker image demonstrating non-interactive EIM installation for CI/CD pipelines.

### Current Dockerfile Key Sections

```dockerfile
# Base image - update when Debian releases new stable version
FROM bitnami/minideb:bookworm

# Required packages for ESP-IDF development
RUN install_packages git cmake ninja-build wget flex bison gperf ccache \
    libffi-dev libssl-dev dfu-util libusb-1.0-0 python3 python3-pip \
    python3-setuptools python3-wheel xz-utils unzip python3-venv curl jq

# EIM download - automatically fetches latest release
ARG TARGETARCH=arm64
RUN set -x && \
    LATEST_RELEASE=$(curl -s https://api.github.com/repos/espressif/idf-im-ui/releases/latest) && \
    # Architecture detection and download logic...

# ESP-IDF installation via EIM
RUN eim -vvv install -n true -a true -r false

# Entrypoint - HARDCODED IDF VERSION - NEEDS UPDATE
ENTRYPOINT ["/bin/bash", "-c", "source /root/.espressif/tools/activate_idf_v5.3.1.sh && python3 /root/.espressif/v5.3.1/esp-idf/tools/idf.py build"]
```

### Maintenance Tasks

1. **Update ESP-IDF version in entrypoint:**
   - When the default IDF version changes, update the `activate_idf_v5.3.1.sh` path
   - This is currently hardcoded and requires manual update

2. **Test multi-architecture builds:**
```bash
# Build for both architectures
docker buildx build --platform linux/amd64,linux/arm64 -t hahihula/eim-idf-build:test .

# Test the image
docker run --rm -it -v $(pwd):/tmp/project hahihula/eim-idf-build:latest
```

3. **Update base image:**
   - Monitor for new Debian stable releases
   - Test with new base image before updating

4. **Sync with EIM documentation:**
   - Keep Docker usage aligned with: https://docs.espressif.com/projects/idf-im-ui/en/latest/headless_usage.html

### Example Usage

```bash
# Build ESP-IDF project using the Docker image
docker run --rm -it -v $(pwd):/tmp/project hahihula/eim-idf-build:latest

# Specify different target
docker run --rm -it -v $(pwd):/tmp/project -e IDF_TARGET=esp32s3 hahihula/eim-idf-build:latest
```

---

## 4. GitHub Install Action

### Used by Workflows

No workflow in **this** repository uses or updates the install-esp-idf-action. It is consumed by **other** repositories when they add `uses: espressif/install-esp-idf-action@v1` to their workflows. Maintenance (testing, version bumps) is done in the action's own repository.

### Repository

**URL:** https://github.com/espressif/install-esp-idf-action

### Purpose

GitHub Action that automates ESP-IDF installation on GitHub-hosted runners using EIM. Supports Windows, macOS (Intel and ARM), and Linux.

### Key Files

| File | Purpose |
|------|---------|
| `action.yml` | Action metadata and input definitions |
| `index.js` | Main action logic |
| `dist/index.js` | Bundled code (committed to repo) |

### Current Inputs (action.yml)

```yaml
inputs:
  version:
    description: "Version of ESP-IDF to install"
    required: false
    default: "latest"
  path:
    description: "Installation path for ESP-IDF"
    required: false
  tools-path:
    description: "Path for ESP-IDF tools"
    required: false
  eim-version:
    description: "Version of EIM to use (default is latest)"
    required: false
```

### Maintenance Tasks

1. **Test after each EIM release:**
```yaml
# Test workflow
name: Test Install Action
on: push
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: espressif/install-esp-idf-action@v1
        with:
          version: "v5.3.2"
      - run: idf.py --version
```

2. **Update when EIM CLI arguments change:**
   - If EIM adds/removes/changes CLI flags, update `index.js`
   - Rebuild with: `ncc build index.js --license licenses.txt`
   - Commit the updated `dist/` directory

3. **Platform-specific testing:**
   - Verify on all three platforms after changes
   - Pay attention to PATH handling differences

4. **Keep README examples current:**
   - Update example workflows when new features are added
   - Document any breaking changes

### Development Workflow

```bash
# Clone the repository
git clone https://github.com/espressif/install-esp-idf-action.git
cd install-esp-idf-action

# Install dependencies
npm install

# Make changes to index.js

# Build the action
npm install -g @vercel/ncc
ncc build index.js --license licenses.txt

# Commit both source and dist
git add .
git commit -m "Update action"
```

---

## 5. Homebrew EIM

### Repository

**URL:** https://github.com/espressif/homebrew-eim

### Used by Workflows

**Invocation:** In `build.yaml`, the job `update-homebrew` runs only when `github.event_name == 'release' && github.event.action == 'created'`. It calls the reusable workflow `.github/workflows/update-homebrew.yml` with `version: ${{ github.ref_name }}` (e.g. `v0.7.1`). It depends on `update-release-info`, so release metadata is available.

**What the workflow does:** `update-homebrew.yml` checks out `espressif/homebrew-eim`, uses the **GitHub release API** (and `secrets.GITHUB_TOKEN`) to get the list of release assets, downloads the macOS CLI and GUI assets (e.g. `eim-cli-macos-x64.zip`, `eim-gui-macos-aarch64.dmg`), computes SHA256, then generates `Formula/eim.rb` and `Casks/eim-gui.rb` and pushes to the homebrew-eim repo using `secrets.HOMEBREW_UPDATE_TOKEN`. It does **not** use workflow artifacts; it always uses the release assets for the given tag.

### Automated Workflow

**File:** `.github/workflows/update-homebrew.yml`

This workflow automatically updates the Homebrew formula and cask when a new release is created.

### Formula Structure (Formula/eim.rb)

```ruby
# typed: false
# frozen_string_literal: true

class Eim < Formula
  desc "ESP-IDF Installation Manager - CLI tool for setting up ESP-IDF development environment"
  homepage "https://github.com/espressif/idf-im-ui"
  version "0.7.1"  # Auto-updated by workflow
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/espressif/idf-im-ui/releases/download/v0.7.1/eim-cli-macos-x64.zip"
      sha256 "459c56e703c10e66b0642b8f2d8f585743336ed657f69bef98bc83e34386234a"
    end
    on_arm do
      url "https://github.com/espressif/idf-im-ui/releases/download/v0.7.1/eim-cli-macos-aarch64.zip"
      sha256 "f5dc2b9f15e24235041a31865051923551b8e7ce4524e56e2d5a72a549189a0f"
    end
  end

  depends_on "dfu-util"
  depends_on "python@3.12"  # Update if Python requirements change

  def install
    bin.install "eim"
    (zsh_completion/"_eim").write Utils.safe_popen_read("#{bin}/eim", "completions", "zsh")
  end

  test do
    assert_match "eim", shell_output("#{bin}/eim --version")
  end
end
```

### Cask Structure (Casks/eim-gui.rb)

```ruby
cask "eim-gui" do
  version "0.7.1"

  on_intel do
    url "https://github.com/espressif/idf-im-ui/releases/download/v0.7.1/eim-gui-macos-x64.dmg"
    sha256 "..."
  end
  on_arm do
    url "https://github.com/espressif/idf-im-ui/releases/download/v0.7.1/eim-gui-macos-aarch64.dmg"
    sha256 "..."
  end

  name "ESP-IDF Installation Manager"
  desc "GUI application for installing and managing ESP-IDF development environment"
  homepage "https://github.com/espressif/idf-im-ui"

  app "eim.app"
end
```

### Manual Verification

```bash
# Add the tap
brew tap espressif/eim

# Install CLI
brew install eim --verbose
eim --version

# Install GUI
brew install --cask eim-gui

# Update
brew upgrade eim

# Troubleshoot
brew doctor
```

### Required Secret

**`HOMEBREW_UPDATE_TOKEN`** - Personal Access Token with:
- Push access to `espressif/homebrew-eim` repository
- Scope: `repo`

### Maintenance Tasks

1. **Verify formula after releases** - Check that the workflow ran successfully
2. **Update dependencies** - If EIM requires different Python versions, update `depends_on`
3. **Test zsh completions** - Verify shell completions work after installation
4. **Monitor Homebrew deprecation warnings** - Keep formula syntax up to date

---

## 6. TLDR Pages Entry

### Used by Workflows

No workflow in this repository creates or updates the TLDR page. The page lives in the community repo `tldr-pages/tldr`. Adding or updating the `eim` entry is done manually (fork, edit `pages/common/eim.md`, open a PR).

### Repository

**URL:** https://github.com/tldr-pages/tldr

### Purpose

TLDR pages provide simplified, community-maintained help pages for command-line tools. Having an entry for `eim` makes it easier for users to discover and use the tool.

### Expected File Location

`pages/common/eim.md`

### Required Format

```markdown
# eim

> ESP-IDF Installation Manager - CLI tool for installing and managing ESP-IDF development environments.
> More information: <https://github.com/espressif/idf-im-ui>.

- Install ESP-IDF non-interactively:

`eim install -i {{v5.3.2}}`

- Run the interactive installation wizard:

`eim wizard`

- List all installed ESP-IDF versions:

`eim list`

- Select an ESP-IDF version as active:

`eim select {{version}}`

- Remove a specific ESP-IDF version:

`eim remove {{version}}`

- Install ESP-IDF from an offline archive:

`eim install --use-local-archive {{path/to/archive.zst}}`

- Generate shell completions:

`eim completions {{bash|zsh|fish|powershell}}`
```

### Submission Process

1. **Fork the repository:**
```bash
gh repo fork tldr-pages/tldr
```

2. **Create the page:**
```bash
cd tldr
mkdir -p pages/common
# Create pages/common/eim.md with the content above
```

3. **Validate the page:**
```bash
# Install tldr-lint
npm install -g tldr-lint

# Lint your page
tldr-lint pages/common/eim.md
```

4. **Submit PR:**
```bash
git add pages/common/eim.md
git commit -m "eim: add page"
git push origin main
gh pr create --title "eim: add page" --body "Add tldr page for ESP-IDF Installation Manager"
```

### Contributing Guidelines

Follow: https://github.com/tldr-pages/tldr/blob/main/CONTRIBUTING.md

Key rules:
- Use `{{placeholder}}` syntax for user-provided values
- Maximum 8 examples per page
- Each example must have a description ending with a colon
- Commands must be wrapped in backticks

---

## 7. Man Page

### Used by Workflows

The file `man/eim.1` is part of the repository and is **included in the Linux packages** produced by `build.yaml`. When the GUI and CLI are built for Linux, the packaging step (e.g. for `.deb` and `.rpm`) typically installs the man page into the package so that `man eim` works after installation. No separate workflow "generates" the man page; it is maintained as source in `man/eim.1` and shipped with the built packages.

### File Location

`man/eim.1` (392 lines)

### Purpose

Unix manual page installed on Linux/macOS systems, accessible via `man eim`.

### Structure

```troff
.TH EIM 1 "2025" "ESP-IDF Installation Manager" "User Commands"
.SH NAME
eim \- ESP-IDF Installation Manager command-line interface
.SH SYNOPSIS
.B eim
[\fIOPTIONS\fR] [\fICOMMAND\fR]
.SH DESCRIPTION
...
.SH GLOBAL OPTIONS
.TP
.BR \-l ", " \-\-locale " " \fILOCALE\fR
Set the language for the wizard (en, cn)
...
.SH COMMANDS
.SS install
.SS wizard
.SS list
.SS select
.SS rename
.SS remove
.SS purge
.SS import
.SS fix
.SS completions
.SS discover
.SH CONFIGURATION
.SH EXAMPLES
.SH OFFLINE INSTALLATION
.SH CUSTOM REPOSITORIES
.SH PRIVACY
.SH FILES
.SH SEE ALSO
.SH BUGS
.SH AUTHOR
.SH COPYRIGHT
```

### Update Triggers

Update the man page when:
- New CLI commands are added
- New options are added to existing commands
- Default values change
- New features like offline installation are modified

### Adding a New Command

Example: Adding a new `upgrade` command

```troff
.SS upgrade
Upgrade an existing ESP-IDF installation to a newer version.

.B eim upgrade
[\fIVERSION\fR]

If VERSION is not provided, upgrades to the latest stable version.

.B Options:
.TP
.BR \-\-keep\-tools
Keep existing tools instead of reinstalling
```

### Adding a New Option

Example: Adding `--quiet` flag to install command

```troff
.TP
.BR \-q ", " \-\-quiet
Suppress non-essential output during installation
```

### Testing the Man Page

```bash
# View locally without installing
man ./man/eim.1

# Check for formatting errors
groff -man -Tascii man/eim.1 > /dev/null

# After installation
man eim
```

### Man Page Formatting Reference

| Macro | Purpose | Example |
|-------|---------|---------|
| `.TH` | Title header | `.TH EIM 1 "2025"` |
| `.SH` | Section header | `.SH COMMANDS` |
| `.SS` | Subsection | `.SS install` |
| `.TP` | Tagged paragraph | `.TP` followed by option |
| `.BR` | Bold/Roman alternating | `.BR \-v ", " \-\-verbose` |
| `.B` | Bold text | `.B eim install` |
| `.I` | Italic text | `.I PATH` |
| `\fI` / `\fR` | Inline italic/roman | `\fIOPTIONS\fR` |

---

## 8. APT Repository

### Hosted Location

- **URL:** https://dl.espressif.com/dl/eim/apt/
- **S3 Bucket:** `s3://espdldata/dl/eim/apt/`

### Used by Workflows

**Invocation:** In `build.yaml`, the job `update-linux-repos` runs when `github.event_name == 'release' && github.event.action == 'created'`. It calls `.github/workflows/update-linux-repos.yml` with `version: ${{ github.ref_name }}`. It depends on `build-cli-linux`, `build-gui`, and `update-release-info`.

**What the workflow does:** The job `update-apt-repo` in `update-linux-repos.yml` downloads **artifacts from the same workflow run** (e.g. `eim-gui-linux-x64-v0.7.1-deb`, `eim-cli-linux-x64-v0.7.1-deb`), not from the release. It uses `actions/download-artifact@v4` with names like `eim-gui-linux-x64-${{ inputs.version }}-deb`. So the `.deb` files must have been uploaded as artifacts by the build jobs in that run. The job then builds the APT repository layout, uploads to S3, and invalidates the CloudFront cache.

### Automated Workflow

**File:** `.github/workflows/update-linux-repos.yml` (lines 17-125)

### Repository Structure

```
apt/
├── pool/main/
│   ├── eim_0.7.1_amd64.deb
│   ├── eim_0.7.1_arm64.deb
│   ├── eim-gui_0.7.1_amd64.deb
│   └── eim-gui_0.7.1_arm64.deb
└── dists/stable/
    ├── Release
    └── main/
        ├── binary-amd64/
        │   ├── Packages
        │   └── Packages.gz
        └── binary-arm64/
            ├── Packages
            └── Packages.gz
```

### How the Workflow Works

1. Downloads `.deb` **artifacts from the same run** (artifact names include `inputs.version`, e.g. `eim-gui-linux-x64-v0.7.1-deb`)
2. Syncs existing packages from S3
3. Generates APT metadata:
   - `dpkg-scanpackages` creates Packages files
   - `apt-ftparchive release` creates Release file
4. Uploads to S3 with `public-read` ACL
5. Invalidates CloudFront cache

### Manual Verification

```bash
# Add repository (no GPG key required currently)
echo "deb https://dl.espressif.com/dl/eim/apt stable main" | sudo tee /etc/apt/sources.list.d/eim.list

# Update package lists
sudo apt update

# Check available versions
apt-cache policy eim
apt-cache policy eim-gui

# Install
sudo apt install eim
sudo apt install eim-gui

# Verify installation
eim --version
```

### Required Secrets

| Secret | Purpose |
|--------|---------|
| `AWS_ACCESS_KEY_ID` | S3 authentication |
| `AWS_SECRET_ACCESS_KEY` | S3 authentication |
| `DL_DISTRIBUTION_ID` | CloudFront cache invalidation |

### Troubleshooting

```bash
# Check repository metadata
curl -s https://dl.espressif.com/dl/eim/apt/dists/stable/Release

# Check package list
curl -s https://dl.espressif.com/dl/eim/apt/dists/stable/main/binary-amd64/Packages

# Clear local cache and retry
sudo rm -rf /var/lib/apt/lists/*
sudo apt update
```

---

## 9. RPM Repository

### Hosted Location

- **URL:** https://dl.espressif.com/dl/eim/rpm/
- **S3 Bucket:** `s3://espdldata/dl/eim/rpm/`

### Used by Workflows

Same workflow as the APT repository: **update-linux-repos.yml**. The job `update-rpm-repo` runs after `update-apt-repo` and downloads **artifacts from the same run** (e.g. `eim-cli-linux-x64-v0.7.1-rpm`, `eim-gui-linux-x64-v0.7.1-rpm`). It uses `actions/download-artifact@v4` with names like `eim-cli-linux-x64-${{ inputs.version }}-rpm`. The job then builds the RPM repository with `createrepo_c`, uploads to S3, and invalidates CloudFront.

### Automated Workflow

**File:** `.github/workflows/update-linux-repos.yml` (lines 126-253)

### Repository Structure

```
rpm/
├── eim.repo
├── x86_64/
│   ├── eim-0.7.1.x86_64.rpm
│   ├── eim-gui-0.7.1.x86_64.rpm
│   └── repodata/
│       ├── repomd.xml
│       ├── primary.xml.gz
│       └── ...
└── aarch64/
    ├── eim-0.7.1.aarch64.rpm
    └── repodata/
        └── ...
```

### Repository Configuration File (eim.repo)

```ini
[eim]
name=ESP-IDF Installation Manager
baseurl=https://dl.espressif.com/dl/eim/rpm/$basearch
enabled=1
gpgcheck=0
```

### How the Workflow Works

1. Downloads `.rpm` **artifacts from the same run** (e.g. `eim-cli-linux-x64-v0.7.1-rpm`)
2. Organizes by architecture (x86_64, aarch64)
3. Generates metadata using `createrepo_c`
4. Creates `eim.repo` configuration file
5. Uploads to S3

### Manual Verification

```bash
# Add repository (Fedora/RHEL/CentOS)
sudo wget -O /etc/yum.repos.d/eim.repo https://dl.espressif.com/dl/eim/rpm/eim.repo

# Or manually create the file
sudo tee /etc/yum.repos.d/eim.repo << 'EOF'
[eim]
name=ESP-IDF Installation Manager
baseurl=https://dl.espressif.com/dl/eim/rpm/$basearch
enabled=1
gpgcheck=0
EOF

# Update and install
sudo dnf check-update
sudo dnf install eim

# Verify
eim --version
```

### Troubleshooting

```bash
# Check repository metadata
curl -s https://dl.espressif.com/dl/eim/rpm/x86_64/repodata/repomd.xml

# Clear DNF cache
sudo dnf clean all
sudo dnf makecache

# List available packages
dnf list available eim*
```

---

## 10. WinGet

### Package Identifiers

| Package | Identifier |
|---------|------------|
| CLI | `Espressif.EIM-CLI` |
| GUI | `Espressif.eim` |

### Used by Workflows

**Invocation:** In `build.yaml`, the job `update-windows-packages` runs when `github.event_name == 'release' && github.event.action == 'created'`. It calls `.github/workflows/update-windows-packages.yml` with `version: ${{ github.ref_name }}` and depends on `build-cli`, `build-gui`, and `update-release-info`.

**What the workflow does:** The workflow has three jobs. The first job, `generate-windows-packages`, downloads **artifacts from the same run** (`eim-cli-windows-x64-${{ inputs.version }}`, `eim-gui-windows-x64-${{ inputs.version }}`) and generates Scoop manifests, then uploads them to the release. The second and third jobs (`publish-winget-cli`, `publish-winget-gui`) sync the fork `Hahihula/winget-pkgs` with upstream and use `vedantmgoyal9/winget-releaser@v2` to create a PR to `microsoft/winget-pkgs`; the releaser uses the **published release assets** (e.g. `eim-cli-windows-x64.exe`, `eim-gui-windows-x64.msi`) and `secrets.WINGET_PAT`.

### Automated Workflow

**File:** `.github/workflows/update-windows-packages.yml` (lines 111-149)

### How the Workflow Works

1. **Syncs the fork** with upstream microsoft/winget-pkgs:
```yaml
- name: Sync WinGet fork
  run: gh repo sync Hahihula/winget-pkgs --source microsoft/winget-pkgs --force
```

2. **Creates PR** using winget-releaser action:
```yaml
- uses: vedantmgoyal9/winget-releaser@v2
  with:
    identifier: Espressif.EIM-CLI
    installers-regex: 'eim-cli-windows-x64\.exe$'
    token: ${{ secrets.WINGET_PAT }}
    fork-user: Hahihula
```

### Manual Verification

```powershell
# Search for packages
winget search Espressif

# Install CLI
winget install Espressif.EIM-CLI

# Install GUI
winget install Espressif.eim

# Update
winget upgrade Espressif.EIM-CLI

# Verify
eim --version
```

### Required Secret

**`WINGET_PAT`** - Personal Access Token requirements:
- Access to `Hahihula/winget-pkgs` fork
- Scopes: `repo`, `workflow`

### PAT Renewal Process

1. Go to https://github.com/settings/tokens
2. Click "Generate new token (classic)"
3. Set expiration (recommend 1 year)
4. Select scopes:
   - `repo` (Full control of private repositories)
   - `workflow` (Update GitHub Action workflows)
5. Generate and copy the token
6. Update in repository settings:
   - Go to `Settings > Secrets and variables > Actions`
   - Update `WINGET_PAT` with new token

### Troubleshooting

If PRs are not being created:
1. Check if `WINGET_PAT` has expired
2. Verify fork sync succeeded
3. Check workflow run logs for errors
4. Ensure package identifier matches exactly

---

## 11. Scoop Distribution (Online)

### Used by Workflows

**Invocation:** Same as WinGet — the job `update-windows-packages` in `build.yaml` calls `update-windows-packages.yml` with `version: ${{ github.ref_name }}` on release.

**What the workflow does:** The first job of `update-windows-packages.yml`, `generate-windows-packages`, downloads the Windows CLI and GUI **artifacts** from the same run (e.g. `eim-cli-windows-x64-v0.7.1`, `eim-gui-windows-x64-v0.7.1`), computes SHA256 hashes of the binaries, and generates two Scoop manifest files (`eim-cli.json`, `eim.json`) with version and download URLs pointing at the GitHub release (e.g. `https://github.com/espressif/idf-im-ui/releases/download/v0.7.1/eim-cli-windows-x64.exe#/eim.exe`). It then uploads these manifests to the **same GitHub Release** as assets using `gh release upload ${{ inputs.version }} manifests-scoop/*.json --clobber`. Users install EIM via Scoop with the manifest URL (e.g. `scoop install https://github.com/espressif/idf-im-ui/releases/latest/download/eim-cli.json`). The workflow does **not** publish to the main Scoop bucket; distribution is via release-hosted manifests only.

### Automated Workflow

**File:** `.github/workflows/update-windows-packages.yml` (lines 37-108)

### Generated Manifests

The workflow generates and uploads these manifests to GitHub releases:

| File | Package |
|------|---------|
| `eim-cli.json` | CLI tool |
| `eim.json` | GUI application |

### Manifest Structure

```json
{
    "version": "0.7.1",
    "description": "ESP-IDF Installation Manager CLI - Setup tool for ESP-IDF development environment",
    "homepage": "https://github.com/espressif/idf-im-ui",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "https://github.com/espressif/idf-im-ui/releases/download/v0.7.1/eim-cli-windows-x64.exe#/eim.exe",
            "hash": "abc123..."
        }
    },
    "bin": "eim.exe",
    "checkver": {
        "github": "https://github.com/espressif/idf-im-ui"
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/espressif/idf-im-ui/releases/download/v$version/eim-cli-windows-x64.exe#/eim.exe"
            }
        }
    }
}
```

### Manual Installation

```powershell
# Install CLI from release manifest
scoop install https://github.com/espressif/idf-im-ui/releases/latest/download/eim-cli.json

# Install GUI from release manifest
scoop install https://github.com/espressif/idf-im-ui/releases/latest/download/eim.json

# Verify
eim --version
```

### Difference from Offline Manifests

| Aspect | Online (Release) | Offline (Templates) |
|--------|------------------|---------------------|
| URL | GitHub release URLs | `file://{{offline_archive_scoop_dir}}/...` |
| Purpose | End-user installation | Bundled in offline archives |
| Generated | Automatically by workflow | Manually maintained |
| Location | GitHub release assets | `src-tauri/scoop_manifest_templates/` |

---

## Maintenance Checklists

### After Each Release

- [ ] Verify Homebrew formula updated automatically (check https://github.com/espressif/homebrew-eim)
- [ ] Verify WinGet PR created (check https://github.com/microsoft/winget-pkgs/pulls)
- [ ] Verify WinGet PR merged (may take 1-3 days for Microsoft review)
- [ ] Verify APT repository updated (`apt-cache policy eim`)
- [ ] Verify RPM repository updated (`dnf info eim`)
- [ ] Verify Scoop manifests attached to release
- [ ] Test Docker image with new release

### Monthly Maintenance

- [ ] Check Scoop manifest template versions against upstream:
  ```bash
  curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/7zip.json | jq .version
  curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/git.json | jq .version
  ```
- [ ] Sync PowerShell scripts with upstream if needed (diff with ScoopInstaller/Install)
- [ ] Verify all PAT tokens are valid:
  - `WINGET_PAT` - test with `gh auth status`
  - `HOMEBREW_UPDATE_TOKEN` - check workflow logs
- [ ] Test installation on all platforms

### With Major EIM Changes

- [ ] Update man page with new commands/options
- [ ] Update or create TLDR page
- [ ] Update Docker repository if installation flags changed
- [ ] Update install-esp-idf-action if CLI interface changed
- [ ] Update documentation references

### Quarterly Security Review

- [ ] Rotate PAT tokens approaching expiration
- [ ] Review AWS IAM permissions
- [ ] Check for security advisories on dependencies
- [ ] Update base images (Docker, etc.)

---

## Secrets Reference

| Secret Name | Purpose | Where Used | Renewal Location |
|-------------|---------|------------|------------------|
| `HOMEBREW_UPDATE_TOKEN` | Push to espressif/homebrew-eim | update-homebrew.yml | GitHub PAT settings |
| `WINGET_PAT` | Fork sync and PR creation to WinGet | update-windows-packages.yml | GitHub PAT settings |
| `AWS_ACCESS_KEY_ID` | S3 upload for APT/RPM repos | update-linux-repos.yml | AWS IAM Console |
| `AWS_SECRET_ACCESS_KEY` | S3 upload for APT/RPM repos | update-linux-repos.yml | AWS IAM Console |
| `DL_DISTRIBUTION_ID` | CloudFront cache invalidation | update-linux-repos.yml | AWS CloudFront Console |
| `GITHUB_TOKEN` | Automatic, for release asset uploads | Various workflows | Automatic (no renewal needed) |

### How to Update Secrets

1. Go to repository `Settings > Secrets and variables > Actions`
2. Click on the secret name
3. Click "Update secret"
4. Paste the new value
5. Click "Update secret"

### PAT Token Scopes Required

**HOMEBREW_UPDATE_TOKEN:**
- `repo` (for pushing to homebrew-eim)

**WINGET_PAT:**
- `repo` (for fork access)
- `workflow` (for PR creation)

---

## External Links Reference

| Component | Repository/URL |
|-----------|---------------|
| Main EIM Repository | https://github.com/espressif/idf-im-ui |
| Docker Repository | https://github.com/Hahihula/eim-idf-build-docker |
| Install Action | https://github.com/espressif/install-esp-idf-action |
| Homebrew Tap | https://github.com/espressif/homebrew-eim |
| TLDR Pages | https://github.com/tldr-pages/tldr |
| Scoop Main Bucket | https://github.com/ScoopInstaller/Main |
| Scoop Installer | https://github.com/ScoopInstaller/Install |
| WinGet Packages | https://github.com/microsoft/winget-pkgs |
| EIM Documentation | https://docs.espressif.com/projects/idf-im-ui |
