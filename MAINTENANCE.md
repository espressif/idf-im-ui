# EIM Maintenance Guide

This document provides comprehensive guidance for maintaining the various distribution components of the ESP-IDF Installation Manager (EIM). It covers all external repositories, package managers, CI/CD workflows, and documentation that need periodic review and updates.

## Table of Contents

- [Maintenance Checklists](#maintenance-checklists)
- [Secrets Reference](#secrets-reference)
- [Overview](#overview)
  - [Distribution Architecture](#distribution-architecture)
  - [Release and Workflow Overview](#release-and-workflow-overview)
- [1. Release Automation](#1-release-automation)
- [2. Sync and Housekeeping Workflows](#2-sync-and-housekeeping-workflows)
- [3. Scoop Manifests for Offline Installer](#3-scoop-manifests-for-offline-installer)
- [4. Scoop Installer PowerShell Scripts](#4-scoop-installer-powershell-scripts)
- [5. Docker Integration](#5-docker-integration)
- [6. GitHub Install Action](#6-github-install-action)
- [7. Homebrew EIM](#7-homebrew-eim)
- [8. TLDR Pages Entry](#8-tldr-pages-entry)
- [9. Man Page](#9-man-page)
- [10. APT Repository](#10-apt-repository)
- [11. RPM Repository](#11-rpm-repository)
- [12. Pacman Repository (Arch Linux)](#12-pacman-repository-arch-linux)
- [13. WinGet](#13-winget)
- [14. Scoop Distribution (Online)](#14-scoop-distribution-online)
- [15. Mirror Infrastructure](#15-mirror-infrastructure)
- [16. CLI Features Impact on Maintenance](#16-cli-features-impact-on-maintenance)
- [External Links Reference](#external-links-reference)

The checklists and secrets reference below are the most frequently used part of this guide; the detailed sections that follow give context for each component.

---

## Maintenance Checklists

### After Each Release

- [ ] Verify correct version of EIM **and** offline installer archives is on [dl.espressif.com](https://dl.espressif.com/dl/eim/)
- [ ] Verify Homebrew formula updated automatically (check https://github.com/espressif/homebrew-eim)
- [ ] Verify WinGet PR created (check https://github.com/microsoft/winget-pkgs/pulls)
- [ ] Verify WinGet PR merged (may take 1-3 days for Microsoft review)
- [ ] Verify APT repository updated (`apt-cache policy eim`)
- [ ] Verify RPM repository updated (`dnf info eim`)
- [ ] Verify Pacman repository updated (`pacman -Si eim`)
- [ ] Verify Scoop manifests attached to release (when Scoop workflow is enabled)
- [ ] Verify Linux packages are signed (check `update-linux-repos.yml` workflow logs)
- [ ] Check telemetry endpoint is reachable and collecting data correctly
- [ ] Test activation and deactivation scripts on all platforms (bash, fish, PowerShell, batch)

### Monthly Maintenance

- [ ] Check Scoop manifest template versions against upstream:
  ```bash
  curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/7zip.json | jq .version
  curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/git.json | jq .version
  ```
- [ ] Verify `sync-git-python-to-s3.yml` weekly runs are succeeding (Git and Python installers on S3)
- [ ] Verify `purge_debug_offline_archives.yml` daily runs are cleaning up debug archives
- [ ] Sync PowerShell scripts with upstream if needed (diff with ScoopInstaller/Install)
- [ ] Verify all PAT tokens are valid:
  - `WINGET_PAT` - test with `gh auth status`
  - `HOMEBREW_UPDATE_TOKEN` - check workflow logs
- [ ] Test installation on all platforms (Windows, macOS, Linux)
- [ ] Update dependencies (Rust, Node, GitHub Actions) as needed

### With Major EIM Changes

- [ ] Update man page with new commands/options (e.g. `shell`, `list-tools`, `list-features`, `fix` with features/tools)
- [ ] Update or create TLDR page
- [ ] Update Docker examples if installation flags changed (e.g. `--cleanup`, `--skip-components-download`)
- [ ] Update `install-esp-idf-action` if CLI interface changed
- [ ] Update documentation references
- [ ] If activation/deactivation scripts changed, verify on all shells (bash, fish, PowerShell, batch)
- [ ] If mirror URLs or probing logic changed, test with `--mirror` and `--repo-stub` flags
- [ ] If offline installer changed, rebuild and test offline archives on all platforms

### Quarterly Security Review

- [ ] Check signing certificates expiration dates
- [ ] Rotate PAT tokens approaching expiration
- [ ] Review AWS IAM permissions
- [ ] Check for security advisories on dependencies
- [ ] Update base images (Docker, etc.)
- [ ] Run `cargo audit` and address findings
- [ ] Review forked dependencies (`lzma-rs`, `dialoguer`, `RustPython`) for upstream updates

---

## Secrets Reference

| Secret Name | Purpose | Where Used | Renewal Location |
|-------------|---------|------------|------------------|
| `HOMEBREW_UPDATE_TOKEN` | Push to espressif/homebrew-eim | `update-homebrew.yml` | GitHub PAT settings |
| `WINGET_PAT` | Fork sync and PR creation to WinGet | `update-windows-packages.yml` | GitHub PAT settings |
| `AWS_ACCESS_KEY_ID` | S3 upload for APT/RPM/Pacman repos, tools sync, archive purge | `update-linux-repos.yml`, `sync-git-python-to-s3.yml`, `purge_debug_offline_archives.yml` | AWS IAM Console |
| `AWS_SECRET_ACCESS_KEY` | S3 upload for APT/RPM/Pacman repos, tools sync, archive purge | `update-linux-repos.yml`, `sync-git-python-to-s3.yml`, `purge_debug_offline_archives.yml` | AWS IAM Console |
| `DL_DISTRIBUTION_ID` | CloudFront cache invalidation | `update-linux-repos.yml`, `sync-git-python-to-s3.yml` | AWS CloudFront Console |
| `ATHENA_BASE_URL` | Athena project management sync | `sync-athena.yml` | Athena admin settings |
| `ATHENA_TOKEN` | Athena API authentication | `sync-athena.yml` | Athena admin settings |
| `GITHUB_TOKEN` | Automatic, for release asset uploads | Various workflows | Automatic (no renewal needed) |
| `SIGNING_KEY` | GPG key for signing Linux packages (APT, RPM, Pacman) | `update-linux-repos.yml` | GPG key management |

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

### Athena Variables (Repository Variables, not Secrets)

| Variable | Purpose |
|----------|---------|
| `ATHENA_PROJECT_UUID` | The Athena project identifier for EIM |
| `ATHENA_DB_PATH` | Database path within Athena |

---

## Overview

EIM is distributed through multiple channels to support different platforms and installation methods. Each channel requires periodic maintenance to ensure compatibility, security, and functionality.

### Distribution Architecture

```
Release Trigger (GitHub Release)
         |
         v
    Build Phase (build.yaml)
         |
         +-- CLI Binaries (Windows, macOS, Linux)
         +-- GUI Binaries (Windows, macOS, Linux)
         +-- .deb packages (x64, arm64)
         +-- .rpm packages (x64, arm64)
         +-- .pacman packages (x86_64, aarch64, armv7h)
         +-- .dmg files (macOS)
         +-- .msi installers (Windows)
         |
         v
  Distribution Phase
         |
         +-- update-homebrew.yml ----------> espressif/homebrew-eim
         +-- update-linux-repos.yml -------> APT Repository (S3)
         |                                   RPM Repository (S3)
         |                                   Pacman Repository (S3)
         +-- update-windows-packages.yml --> WinGet (microsoft/winget-pkgs)
                                             Scoop Manifests (Release assets)

  Sync / Housekeeping (independent of releases)
         |
         +-- sync-athena.yml ------------> Athena project tracker
         +-- sync-git-python-to-s3.yml --> Git & Python installers on S3
         +-- purge_debug_offline_archives.yml -> S3 debug archive cleanup
```

### Release and Workflow Overview

Releases are **not** created by CI. A maintainer creates a GitHub Release (e.g. tag `v0.19.0` and "Publish release"). That triggers the following:

1. **Trigger:** `build.yaml` runs when `release.type` is `created`.

2. **Build jobs:** `build-cli`, `build-cli-linux`, and `build-gui` build binaries for all platforms. Each job uploads **artifacts** and, on release, uploads the same files as **release assets** to the GitHub Release. Binaries now include version tags in filenames (e.g. `eim-cli-windows-x64-v0.19.0`).

3. **Offline archives:** The job `build-offline-archives` calls `build_offline_installer_archives.yaml`. It uses the `offline_installer_builder` binary and the scoop manifest templates to build offline archives and upload them atomically to S3.

4. **Release info:** The job `update-release-info` fetches the latest release JSON and uploads it to S3 (`eim_unified_release.json`).

5. **Distribution workflows:** After `update-release-info`, the main workflow calls three reusable workflows with `version: ${{ github.ref_name }}`:
   - **`update-homebrew.yml`** -- Downloads macOS assets from the release API, computes SHA256, updates `espressif/homebrew-eim`.
   - **`update-linux-repos.yml`** -- Downloads artifacts from the same run, updates APT, RPM, and Pacman repos on S3. Packages are now **signed** with the repository GPG key.
   - **`update-windows-packages.yml`** -- Downloads artifacts from the same run, generates Scoop manifests, uploads them to the release, then runs WinGet releaser (PR to `microsoft/winget-pkgs`).

**Release Preparation:** Use `prepare-release.yml` (workflow_dispatch) to automate version bumping. It creates a release branch, bumps versions in `Cargo.toml` and `package.json`, updates lock files, and creates a PR.

---

## 1. Release Automation

### Purpose

The `prepare-release.yml` workflow automates the release preparation process, replacing manual version bumps.

### Workflow File

`.github/workflows/prepare-release.yml`

### How It Works

1. Triggered manually via `workflow_dispatch` with a version string (e.g. `0.19.0`)
2. Validates semver format
3. Creates a `release-v{version}` branch from the source branch
4. Bumps version in `src-tauri/Cargo.toml` and `package.json`
5. Runs `cargo update --workspace` to update `Cargo.lock`
6. Commits and pushes the release branch
7. Creates a PR targeting the specified branch (default: `master`)

### Maintenance Notes

- The version validation regex accepts pre-release suffixes (e.g. `1.0.0-rc.1`)
- If the release branch already exists, the workflow fails safely
- Review the generated PR carefully: check that `Cargo.lock` changes look reasonable

---

## 2. Sync and Housekeeping Workflows

### sync-athena.yml

**Purpose:** Syncs GitHub issues, comments, and PRs to the Athena project management tool.

**Triggers:** Issue events, comment events, PR events, manual dispatch (backfill).

**What it does:**
- `issue_to_task` -- Creates EIM tickets from GitHub issues
- `comment_to_task` -- Appends comments to existing EIM tickets
- `pr_to_comment` -- Links PRs to their EIM tickets

**Required secrets/variables:** `ATHENA_BASE_URL`, `ATHENA_TOKEN`, `ATHENA_PROJECT_UUID`, `ATHENA_DB_PATH`

### sync-git-python-to-s3.yml

**Purpose:** Keeps Git for Windows and Python installers on Espressif's S3 (`dl.espressif.com`) for use by the offline installer and EIM's dependency installation.

**Triggers:** Weekly (Monday 02:00 UTC), on release publish, manual dispatch.

**What it does:**
- Downloads Python build-standalone installers (Windows x64 and arm64) from `astral-sh/python-build-standalone`
- Downloads the latest Git for Windows release (x64 and arm64 tar.bz2)
- Uploads to `s3://espdldata/dl/eim/tools/{python,git}/` if not already present
- Updates a `git-windows-latest.json` manifest with the current version
- Invalidates CloudFront cache

**S3 paths:**
- `dl/eim/tools/python/` -- Python standalone builds
- `dl/eim/tools/git/` -- Git for Windows archives

### purge_debug_offline_archives.yml

**Purpose:** Daily cleanup of debug offline archive builds on S3 to control storage costs.

**Triggers:** Daily at 04:00 UTC, manual dispatch.

**What it does:**
- Lists all objects under `s3://espdldata/dl/eim/debug/`
- Deletes objects older than 48 hours (configurable via `max_age_hours`)
- Supports dry-run mode
- Writes a summary to the GitHub Actions job summary

### sync-jira.yml and jira-pr-comment.yml

**Purpose:** Syncs GitHub issues, comments, and PRs to Jira (project EIM). The `jira-pr-comment.yml` adds a PR link comment to Jira when the PR title matches `EIM-{number}: ...`.

**Triggers:** Issue/comment/PR events (sync-jira), PR open/edit (jira-pr-comment), hourly PR scan, manual `mirror-issues` dispatch.

**Required secrets:** Jira API credentials (configured in repository secrets).

---

## 3. Scoop Manifests for Offline Installer

### Purpose

These JSON manifest templates define how Scoop installs dependencies (7-Zip, Git, Python, etc.) during offline Windows installation. They are bundled into offline archives and processed at runtime.

### Used by Workflows

These files are **not** read by any workflow directly. They are used in the offline installer build process:

- **`build.yaml`** -- Builds the `offline_installer_builder` binary.
- **`build_offline_installer_archives.yaml`** -- Runs the builder to produce offline archives. The scoop manifest templates are included in the archive and processed at runtime with the `{{offline_archive_scoop_dir}}` placeholder.

### File Locations

| File | Description |
|------|-------------|
| `src-tauri/scoop_manifest_templates/7zip.json` | 7-Zip archiver |
| `src-tauri/scoop_manifest_templates/git.json` | Git for Windows |
| `src-tauri/scoop_manifest_templates/python311.json` | Python 3.11 |
| `src-tauri/scoop_manifest_templates/dark.json` | WiX Toolset Decompiler |

> **Note:** `python310.json` also exists in the templates directory but is not actively used by the current offline installer code. It is kept for potential future use.

### How It Works

1. Templates use `{{offline_archive_scoop_dir}}` as a placeholder for the local path
2. At runtime, the offline installer replaces the placeholder with the actual extraction directory
3. Scoop then installs each package using the processed manifest

### Version Update Procedure

1. Check the upstream Scoop bucket for the latest version (e.g. https://github.com/ScoopInstaller/Main/blob/master/bucket/7zip.json)
2. Update the corresponding template file with new version, URLs, and hashes
3. Download the new binaries and calculate SHA256
4. Update the `autoupdate` URL pattern if the naming convention changed

> **Important:** The actual download URLs for the dependency files (7-Zip, Git, Python) used in offline archives are managed through the `sync-git-python-to-s3.yml` workflow, which syncs them to `dl.espressif.com`. Updates to template versions must be coordinated with the S3-hosted files.

### Fetching Upstream Changes

```bash
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/7zip.json | jq .version
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/git.json | jq .version
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/python.json | jq .version
curl -s https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/dark.json | jq .version
```

### Important Notes

- The `url` field uses `file://{{offline_archive_scoop_dir}}/...` for offline installation
- Keep `checkver` and `autoupdate` sections for reference
- Python manifests include PEP-514 registry entries for Python discovery by other tools
- Test offline installation after any manifest changes

---

## 4. Scoop Installer PowerShell Scripts

### Purpose

These scripts install and configure Scoop package manager on Windows. The offline version allows installation without internet access.

### Used by Workflows

No workflow in this repository runs these scripts. They are bundled into the offline installation archive when `build_offline_installer_archives.yaml` runs the builder. The archive content is then used on a user's Windows machine.

### File Locations

| File | Description |
|------|-------------|
| `src-tauri/powershell_scripts/install_scoop_offline.ps1` | Offline Scoop installer |
| `src-tauri/powershell_scripts/install_scoop.ps1` | Online Scoop installer |

### Upstream Source

**Official Scoop Installer:** https://github.com/ScoopInstaller/Install/blob/master/install.ps1

### How to Fetch Upstream Changes

```bash
curl -o /tmp/upstream_install.ps1 https://raw.githubusercontent.com/ScoopInstaller/Install/master/install.ps1
diff src-tauri/powershell_scripts/install_scoop.ps1 /tmp/upstream_install.ps1
```

---

## 5. Docker Integration

### Overview

EIM replaces the traditional `install.sh`/`export.sh` scripts for installing ESP-IDF in Docker containers. The official esp-idf Docker images are being migrated to use EIM.

### Documentation Dockerfile

The EIM documentation at `docs/src/headless_usage.md` contains the reference Dockerfile example. This is the primary source of truth for Docker-based EIM usage.

**Key patterns:**
- Use `eim install` (runs in non-interactive mode by default)
- Use `--cleanup true` to remove tool archive files and reduce image size
- Use `--skip-components-download true` to defer component fetching
- Use `-a true` to auto-install prerequisites

### Official esp-idf Docker Images

The official Docker images in the [espressif/esp-idf](https://github.com/espressif/esp-idf) repository are being migrated to use EIM instead of the legacy Python-based `idf_tools.py` installer. The `esp-dockerfiles` repository also contains CI images.

### Maintenance Notes

- When EIM CLI flags change, update the Dockerfile examples in `docs/src/headless_usage.md`
- Verify the latest EIM binary download URL pattern works in Docker builds
- Test both x64 and arm64 Docker images
- The `--cleanup` flag is important for reducing Docker image size

---

## 6. GitHub Install Action

### Repository

**URL:** https://github.com/espressif/install-esp-idf-action

### Purpose

Provides a GitHub Action for installing ESP-IDF in CI workflows. Uses EIM under the hood.

### Usage Example

```yaml
steps:
  - uses: actions/checkout@v6
  - name: Install ESP-IDF
    uses: espressif/install-esp-idf-action@v1
    with:
      version: "v5.3.2"
      path: "/custom/path/to/esp-idf"
      tools-path: "/custom/path/to/tools"
```

### Maintenance Notes

- Update when EIM CLI interface changes
- Ensure the action downloads the correct versioned EIM binary
- Test on Windows, macOS, and Linux runners

---

## 7. Homebrew EIM

### Repository

**URL:** https://github.com/espressif/homebrew-eim

### Automated Workflow

**File:** `.github/workflows/update-homebrew.yml`

### How the Workflow Works

1. Triggered by the main `build.yaml` after a release
2. Downloads macOS assets from the GitHub release API (aarch64 and x64)
3. Computes SHA256 hashes
4. Updates the formula and cask in `espressif/homebrew-eim`
5. Pushes the changes using `HOMEBREW_UPDATE_TOKEN`

### Manual Verification

```bash
brew tap espressif/eim
brew install espressif/eim/eim
eim --version
```

### Required Secret

`HOMEBREW_UPDATE_TOKEN` -- PAT with `repo` scope for pushing to `espressif/homebrew-eim`.

---

## 8. TLDR Pages Entry

### Repository

**URL:** https://github.com/tldr-pages/tldr

### Submission Process

1. Fork https://github.com/tldr-pages/tldr
2. Create `pages/common/eim.md` following TLDR format
3. Lint with `tldr-lint pages/common/eim.md`
4. Submit PR

### Contributing Guidelines

Follow: https://github.com/tldr-pages/tldr/blob/main/CONTRIBUTING.md

Key rules:
- Use `{{placeholder}}` syntax for user-provided values
- Maximum 8 examples per page
- Each example must have a description ending with a colon

### Update Triggers

Update the TLDR page when:
- New CLI commands are added (e.g. `shell`, `list-tools`, `list-features`)
- New important flags are added (e.g. `--cleanup`, `--skip-components-download`)
- Command syntax changes

---

## 9. Man Page

### File Location

`man/eim.1`

### Purpose

Unix manual page installed on Linux/macOS systems, accessible via `man eim`.

### Used by Workflows

The man page is maintained as source in `man/eim.1` and is included in Linux packages (`.deb`, `.rpm`, `.pacman`) produced by `build.yaml`.

### Structure

The man page covers:
- `NAME`, `SYNOPSIS`, `DESCRIPTION`
- `GLOBAL OPTIONS`
- `COMMANDS` -- install, wizard, list, list-tools, list-features, select, rename, remove, purge, import, fix, run, shell, completions, discover
- `CONFIGURATION`, `EXAMPLES`, `OFFLINE INSTALLATION`, `CUSTOM REPOSITORIES`, `PRIVACY`
- `FILES`, `SEE ALSO`, `BUGS`, `AUTHOR`, `COPYRIGHT`

### Update Triggers

Update the man page when:
- New CLI commands are added
- New options are added to existing commands
- Default values change
- New features like offline installation are modified

### Testing

```bash
man ./man/eim.1
groff -man -Tascii man/eim.1 > /dev/null
```

---

## 10. APT Repository

### Hosted Location

- **URL:** https://dl.espressif.com/dl/eim/apt/
- **S3 Bucket:** `s3://espdldata/dl/eim/apt/`

### Automated Workflow

**File:** `.github/workflows/update-linux-repos.yml` (job: `update-apt-repo`)

### How the Workflow Works

1. Downloads `.deb` artifacts from the same workflow run
2. Syncs existing packages from S3
3. Signs packages with the repository GPG key
4. Generates APT metadata (`dpkg-scanpackages`, `apt-ftparchive release`)
5. Uploads to S3 with `public-read` ACL
6. Invalidates CloudFront cache

### Repository Structure

```
apt/
+-- pool/main/
|   +-- eim_{version}_amd64.deb
|   +-- eim_{version}_arm64.deb
|   +-- eim-gui_{version}_amd64.deb
|   +-- eim-gui_{version}_arm64.deb
+-- dists/stable/
    +-- Release
    +-- main/
        +-- binary-amd64/
        |   +-- Packages
        |   +-- Packages.gz
        +-- binary-arm64/
            +-- Packages
            +-- Packages.gz
```

### Manual Verification

```bash
echo "deb https://dl.espressif.com/dl/eim/apt stable main" | sudo tee /etc/apt/sources.list.d/eim.list
sudo apt update
apt-cache policy eim
sudo apt install eim
eim --version
```

---

## 11. RPM Repository

### Hosted Location

- **URL:** https://dl.espressif.com/dl/eim/rpm/
- **S3 Bucket:** `s3://espdldata/dl/eim/rpm/`

### Automated Workflow

**File:** `.github/workflows/update-linux-repos.yml` (job: `update-rpm-repo`)

### How the Workflow Works

1. Downloads `.rpm` artifacts from the same workflow run
2. Organizes by architecture (x86_64, aarch64)
3. Signs packages with the repository GPG key
4. Generates metadata using `createrepo_c`
5. Creates `eim.repo` configuration file
6. Uploads to S3

### Manual Verification

```bash
sudo wget -O /etc/yum.repos.d/eim.repo https://dl.espressif.com/dl/eim/rpm/eim.repo
sudo dnf check-update
sudo dnf install eim
eim --version
```

---

## 12. Pacman Repository (Arch Linux)

### Hosted Location

- **URL:** https://dl.espressif.com/dl/eim/pacman/
- **S3 Bucket:** `s3://espdldata/dl/eim/pacman/`

### Automated Workflow

**File:** `.github/workflows/update-linux-repos.yml` (job: `update-pacman-repo`)

### How the Workflow Works

1. Downloads `.pacman` artifacts from the same workflow run (x86_64, aarch64, armv7h)
2. Syncs existing packages from S3
3. Signs packages with the repository GPG key using `repo-add --sign`
4. Generates pacman database files
5. Uploads to S3 with separate architecture directories

### Repository Structure

```
pacman/
+-- x86_64/
|   +-- eim-{version}-x86_64.pkg.tar.zst
|   +-- eim.db
|   +-- eim.db.tar.gz
|   +-- eim.files
+-- aarch64/
|   +-- eim-{version}-aarch64.pkg.tar.zst
|   +-- ...
+-- armv7h/
    +-- eim-{version}-armv7h.pkg.tar.zst
    +-- ...
```

### Manual Verification

```bash
# Add to /etc/pacman.conf:
# [eim]
# SigLevel = Optional TrustAll
# Server = https://dl.espressif.com/dl/eim/pacman/$arch

sudo pacman -Syu
sudo pacman -S eim
eim --version
```

### Maintenance Notes

- Pacman support was added in v0.16.0
- The repository supports three architectures: x86_64, aarch64, armv7h
- Package signing is handled by the same GPG key as APT/RPM
- **Note:** Pacman is not yet covered by `test_pkg_managers.yml` (only APT, DNF, Homebrew, and WinGet have automated post-release verification). Manual testing on Arch Linux is recommended after each release.

---

## 13. WinGet

### Package Identifiers

| Package | Identifier |
|---------|------------|
| CLI | `Espressif.EIM-CLI` |
| GUI | `Espressif.eim` |

### Automated Workflow

**File:** `.github/workflows/update-windows-packages.yml`

### How the Workflow Works

1. Downloads Windows artifacts from the same workflow run
2. Generates Scoop manifests and uploads them to the release
3. Syncs the WinGet fork with upstream `microsoft/winget-pkgs`
4. Uses `vedantmgoyal9/winget-releaser@v2` to create PRs to `microsoft/winget-pkgs`

> **Note:** The WinGet workflow currently uses a personal fork for the winget-pkgs sync step. This should be migrated to an organization-owned fork when available.

### Manual Verification

```powershell
winget search Espressif
winget install Espressif.EIM-CLI
winget install Espressif.eim
eim --version
```

### Required Secret

**`WINGET_PAT`** -- Personal Access Token requirements:
- Scopes: `repo`, `workflow`

### Troubleshooting

If PRs are not being created:
1. Check if `WINGET_PAT` has expired
2. Verify fork sync succeeded
3. Check workflow run logs for errors
4. Ensure package identifier matches exactly

---

## 14. Scoop Distribution (Online)

### Current Status

**There is no dedicated Scoop repository at the moment.** The Scoop distribution works via release-hosted manifests only. Users install EIM via Scoop with the manifest URL from the GitHub release.

### How It Works

The `update-windows-packages.yml` workflow generates two Scoop manifests (`eim-cli.json`, `eim.json`) and uploads them as GitHub Release assets. The manifests contain version, SHA256 hashes, and download URLs pointing at the release.

### Manual Installation

```powershell
scoop install https://github.com/espressif/idf-im-ui/releases/latest/download/eim-cli.json
scoop install https://github.com/espressif/idf-im-ui/releases/latest/download/eim.json
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

## 15. Mirror Infrastructure

### Background

EIM supports installing ESP-IDF from alternative repository mirrors using `--mirror` and `--repo-stub` CLI flags. This is critical for users in regions with restricted network access.

### Mirror History

- **JihuLab mirror** (`jihulab.com/esp-mirror`): Previously the default mirror for China. Now deprecated due to authentication requirements that break submodule fetching. Replaced by Espressif's own mirror infrastructure (EIM-332).
- **Espressif mirror**: The current replacement. EIM includes mirror probing logic to test connectivity before use.
- **Offline archives**: The primary fallback for restricted network environments.

### Mirror-Related Code

- `src-tauri/src/lib/git_tools.rs` -- Contains `apply_github_mirror()`, `reverse_github_mirror()`, and submodule URL resolution logic
- Mirror probing runs at installation time to detect and prefer the fastest available mirror

### Custom Repository Configuration

```bash
# For GitHub repositories (only repo-stub needed):
eim install -i {{version}} --repo-stub my-github-user/my-custom-idf

# For completely custom repositories (GitLab, self-hosted, etc.):
eim install -i {{version}} --mirror https://gitlab.example.com --repo-stub my-gitlab-user/my-custom-idf
```

### Maintenance Notes

- When mirror URLs change, update `git_tools.rs` and associated tests
- The offline installer serves as the fallback for all mirror issues
- Monitor GitHub issues for mirror-related installation failures (common pattern: JihuLab auth errors)

---

## 16. CLI Features Impact on Maintenance

When new CLI commands or flags are added, multiple distribution components may need updates. Use this checklist:

### Commands Added Since v0.15.0

| Command / Flag | Version | Maintenance Impact |
|---------------|---------|-------------------|
| `eim shell` | v0.19.0 | Man page, TLDR, docs |
| `eim list-tools` | v0.17.0 | Man page, TLDR, docs |
| `eim list-features` | v0.17.0 | Man page, TLDR, docs |
| `eim fix --idf-features` / `--idf-tools` | v0.16.0 | Man page, docs |
| `eim install --cleanup` | v0.16.0 | Docker examples, CI docs |
| `eim install --skip-components-download` | v0.18.0 | Docker examples, CI docs |
| `eim install --use-local-archive` | v0.15.0 | Offline installation docs |
| Deactivation scripts | v0.15.0 | After-install docs, activation/deactivation reference |
| Telemetry | v0.19.0 | Privacy docs, man page |
| Installation status in config | v0.19.0 | Config file docs |

### What to Update for Each New Command/Flag

1. **Man page** (`man/eim.1`) -- Add command/option documentation
2. **TLDR page** -- Add example if the command is commonly used
3. **Documentation** (`docs/src/`) -- Update relevant docs pages
4. **GitHub Install Action** -- Update if the action's interface is affected
5. **Docker examples** -- Update if the flag is relevant to CI/Docker usage

---

## External Links Reference

| Component | Repository/URL |
|-----------|---------------|
| Main EIM Repository | https://github.com/espressif/idf-im-ui |
| Install Action | https://github.com/espressif/install-esp-idf-action |
| Homebrew Tap | https://github.com/espressif/homebrew-eim |
| TLDR Pages | https://github.com/tldr-pages/tldr |
| Scoop Main Bucket | https://github.com/ScoopInstaller/Main |
| Scoop Installer | https://github.com/ScoopInstaller/Install |
| WinGet Packages | https://github.com/microsoft/winget-pkgs |
| EIM Documentation | https://docs.espressif.com/projects/idf-im-ui |
| EIM Downloads | https://dl.espressif.com/dl/eim/ |
| Athena Sync Action | https://github.com/hahihula/sync-athena |
