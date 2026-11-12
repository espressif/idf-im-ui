# Troubleshooting

When EIM fails, the last lines of the message say what to do and end with a code such as `EIM-E101`. Find that code below, or follow the link in the message. A warning that says EIM is continuing is not a failure. Act on it only if the command fails later.

Errors include a `Fix:` line. Warnings describe what to do only if the command later fails.

| Code | Problem |
|------|---------|
| [EIM-E101](#eim-e101) | Missing prerequisites |
| [EIM-E102](#eim-e102) | Prerequisite check could not run |
| [EIM-E103](#eim-e103) | Prerequisite installation failed |
| [EIM-E104](#eim-e104) | QEMU prerequisites |
| [EIM-E201](#eim-e201) | Python sanity check failed |
| [EIM-E202](#eim-e202) | Python installation failed or was refused |
| [EIM-E203](#eim-e203) | Virtual environment or requirements failed |
| [EIM-E204](#eim-e204) | Constraints file download failed |
| [EIM-E205](#eim-e205) | Offline wheels missing |
| [EIM-E301](#eim-e301) | Version list fetch failed |
| [EIM-E302](#eim-e302) | Tool download failed |
| [EIM-E303](#eim-e303) | PyPI download failed |
| [EIM-E401](#eim-e401) | Clone failed |
| [EIM-E402](#eim-e402) | Destination is not empty |
| [EIM-E403](#eim-e403) | Submodule update failed |
| [EIM-E501](#eim-e501) | No space left, including a small /tmp |
| [EIM-E502](#eim-e502) | Permission denied on a custom path |
| [EIM-E503](#eim-e503) | Running as administrator |
| [EIM-E601](#eim-e601) | No IDF version specified |
| [EIM-E602](#eim-e602) | Already installed |
| [EIM-E603](#eim-e603) | Fix failed or the directory is not an IDF checkout |
| [EIM-E701](#eim-e701) | Offline archive |
| [EIM-E801](#eim-e801) | PowerShell missing or blocked |
| [EIM-E802](#eim-e802) | Drivers need an administrator |
| [EIM-E900](#eim-e900) | Any other command failure |

## Logs

When a CLI command fails, it prints the path of the log file it wrote.

| File | When |
|------|------|
| `eim.log` | CLI. Default directory below. Override with `--log-file`. |
| `eim_gui.log` | GUI, same directory. |
| `offline_installer.log` | Offline archive builder, same directory. |

| Platform | Directory |
|----------|-----------|
| Linux | `$XDG_DATA_HOME/eim/logs` or `$HOME/.local/share/eim/logs` |
| macOS | `$HOME/Library/Application Support/eim/logs` |
| Windows | `%LOCALAPPDATA%\eim\logs` |

If EIM cannot find a data directory, the CLI writes `eim.log` in the current directory.

Increase detail with `-v` (repeat it for more).

## Prerequisites

### EIM-E101: Missing prerequisites {#eim-e101}

**What you see:** `Please install the missing prerequisites and try again.`

**Why:** A required package is missing. On Linux and macOS, EIM never installs prerequisites for you. On Windows it installs them only when you agree, or when `--install-all-prerequisites true` is set.

**What to do:** Install the packages named above with your system package manager, then rerun the same `eim` command. The names for your system are on the [Prerequisites](./prerequisites.md) page. On Windows, `eim install --install-all-prerequisites true` installs Git and Python. That flag does nothing on Linux or macOS. QEMU libraries are [EIM-E104](#eim-e104).

### EIM-E102: Prerequisite check could not run {#eim-e102}

**What you see:** one of

- `The installer is unable to verify the prerequisites. Please try again with the --skip-prerequisites-check flag.`
- `Basic shell execution failed on this system. Unable to verify prerequisites.`
- `A system error occurred while checking prerequisites:`

**Why:** EIM could not run its prerequisite check, because the shell failed or the check itself hit an error. This is not the same as a package being absent.

**What to do:** Restore a working shell (`bash` on Linux and macOS, `powershell.exe` or `pwsh.exe` on Windows), or skip the check only if the packages are already installed:

```bash
eim install --skip-prerequisites-check true
```

`--install-all-prerequisites` won't help here: it installs packages, and the problem is that the check itself can't run. On Windows, PowerShell missing or blocked is [EIM-E801](#eim-e801).

### EIM-E103: Prerequisite installation failed {#eim-e103}

**What you see:** `Prerequisite installation failed`, `Something went terribly wrong. These prerequisites were not installed:`, `Failed to install python:`, or `Failed to install git:`. A package-manager panic starts with `Failed to install`.

**Why:** EIM tried to install prerequisites and at least one package did not install.

**What to do:** Read the package-manager output, install the named packages yourself, then rerun `eim`. See [Missing prerequisites](#eim-e101). On Windows, `--install-all-prerequisites true` is what asks EIM to install them.

### EIM-E104: QEMU prerequisites {#eim-e104}

**What you see:** one of

- `The following prerequisites for QEMU are not satisfied`
- `Failed to check QEMU prerequisites.`
- `QEMU prerequisites are not satisfied and you selected QEMU for installation.`

**Why:** QEMU was selected and its libraries are missing. The message lists them after `Missing:`.

**What to do:** Install those packages, or rerun without QEMU. The names are on the [Prerequisites](./prerequisites.md) page. EIM does not install these libraries for you.

## Python

Supported interpreters for a normal install are Python **3.10, 3.11, 3.12, 3.13, and 3.14** (`>= 3.10.0` and `< 3.15.0`). Offline archives ship wheels for **3.11 to 3.14** on Linux, macOS, and Windows. See [Offline archive](#eim-e701).

### EIM-E201: Python sanity check failed {#eim-e201}

**What you see:** `Python sanity check failed. See the [FAIL] entries above for details.`

Each failed check is a `[FAIL]` line followed by `Hint:`.

**Why:** The selected Python is missing, too old, too new, or lacks pip, venv, the standard library, ctypes, or SSL. On Windows the Microsoft Store alias also fails this check.

**What to do:** Follow the `Hint:` line for each `[FAIL]`. Install Python 3.10 to 3.14 from [python.org](https://www.python.org/downloads/) when the hint says to reinstall.

If `[FAIL] SSL/HTTPS` appears only when a proxy is set, bypass localhost:

```bash
export NO_PROXY=localhost,127.0.0.1,::1
export no_proxy=localhost,127.0.0.1,::1
```

The sanity check opens a local HTTPS server. A proxy that captures `127.0.0.1` makes that check fail even when SSL works. To confirm the proxy is the cause, unset it and rerun:

```bash
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY
eim install
```

If that passes, exclude localhost rather than disabling the proxy.

On macOS, a GUI launch does not load your shell `PATH`. Finder starts EIM from `launchd`, which does not read `.zprofile` or `.zshrc`, so the GUI can see `/usr/bin/python3` (often 3.9) while a terminal sees Homebrew (`/opt/homebrew/bin` or `/usr/local/bin`) or MacPorts (`/opt/local/bin`). Compare `which python3` in the terminal with the version EIM reports. Start the GUI from that terminal with `eim gui`, or install Python from python.org so GUI launches can see it.

When several Pythons are installed, EIM uses the first `python3` or `python` on `PATH` that passes the sanity check. Prepend the directory you want, for this terminal only:

```bash
export PATH="/path/to/correct/python/folder:$PATH"
eim install
```

```powershell
$env:PATH = "C:\path\to\correct\python\folder;" + $env:PATH
eim install
```

The activation scripts then pin that interpreter.

### EIM-E202: Python installation failed or was refused {#eim-e202}

**What you see:** `Python installation failed` or `Please install Python3 with pip, venv, and SSL support and try again`

**Why:** EIM could not install Python, or you declined the install and no suitable interpreter is present.

**What to do:** Install Python 3.10 to 3.14 with pip, venv, and SSL from [python.org](https://www.python.org/downloads/), then rerun `eim`. On Windows, `eim install --install-all-prerequisites true` asks EIM to install Python.

### EIM-E203: Virtual environment or requirements failed {#eim-e203}

**What you see:** one of

- `failed to ensure venv path:`
- `failed to create venv:`
- `failed to install requirements from file`

**Why:** The tools directory is not usable, `python -m venv` failed, or `pip install -r` failed. A pip download problem is [PyPI download](#eim-e303). A missing offline wheel is [Offline wheels](#eim-e205).

**What to do:** Use Python 3.10 to 3.14 with the `venv` module and rerun `eim`. Point `--path` at a directory your user can write to. For a package-index failure, rerun with `--pypi-mirror` as in [PyPI download](#eim-e303).

### EIM-E204: Constraints file download failed {#eim-e204}

**What you see:** `Failed to download constraints file:`

**Why:** EIM downloads `espidf.constraints.<version>.txt` from `dl.espressif.com`. This warning does not stop the installation. `--mirror`, `--idf-mirror`, and `--pypi-mirror` do not replace that host.

**What to do:** Retry the same command when `dl.espressif.com` is reachable. For an air-gapped machine, use an offline archive ([EIM-E701](#eim-e701)); the archive copies this file instead of downloading it.

### EIM-E205: Offline wheels missing {#eim-e205}

**What you see:** `Could not find wheels for Python` or `No compatible wheel directory found for Python`

**Why:** The offline archive has no `wheels_py*` directory for the Python that was detected. Published archives include wheels for Python 3.11, 3.12, 3.13, and 3.14.

**What to do:** Install one of those Python versions and rerun against the same `.zst`, or rebuild the archive with the offline archive builder so it contains wheels for the version you need.

## Network

### EIM-E301: Version list fetch failed {#eim-e301}

**What you see:** `Error downloading IDF versions from` or, in the GUI, `Unable to fetch available ESP-IDF versions.` A broken bundled list says `Error parsing cached IDF versions JSON:`.

**Why:** `https://dl.espressif.com/dl/esp-idf/idf_versions.json` could not be downloaded. EIM then uses the version list built into the binary and the command continues. Mirror flags do not change this URL.

**What to do:** Retry later. You can still install a version you already know:

```bash
eim install -i v5.4.2
```

If the message is `Error parsing cached IDF versions JSON`, reinstall EIM. The copy shipped inside the binary could not be read.

### EIM-E302: Tool download failed {#eim-e302}

**What you see:** `Download attempt` followed by `failed for`.

**Why:** One download attempt failed. EIM retries on its own. The URL in the message is the host that failed.

**What to do:** Nothing while EIM is still retrying. If the command fails after the retries, rerun with a mirror closer to you.

Tools (`--mirror`), any of:

- `https://github.com`
- `https://dl.espressif.com/github_assets`
- `https://dl.espressif.cn/github_assets`

ESP-IDF git (`--idf-mirror`), any of:

- `https://github.com`
- `https://git.espressif.com.cn`

Example:

```bash
eim install -i v5.4.2 --mirror https://dl.espressif.com/github_assets --idf-mirror https://git.espressif.com.cn
```

Python package downloads use `--pypi-mirror` ([EIM-E303](#eim-e303)).

### EIM-E303: PyPI download failed {#eim-e303}

**What you see:** pip text inside `failed to install requirements from file` ([EIM-E203](#eim-e203)) about an index, a timeout, or a connection to a package host.

**Why:** pip could not download from the Espressif package index or the selected PyPI mirror.

**What to do:** Rerun with `--pypi-mirror` set to one of:

- `https://pypi.org/simple`
- `https://mirrors.aliyun.com/pypi/simple`
- `https://pypi.tuna.tsinghua.edu.cn/simple`
- `https://pypi.mirrors.ustc.edu.cn/simple`

```bash
eim install -i v5.4.2 --pypi-mirror https://pypi.tuna.tsinghua.edu.cn/simple
```

## Git

### EIM-E401: Clone failed {#eim-e401}

**What you see:** `git clone failed` or `Both gix and system git failed to clone ESP-IDF.`

**Why:** The ESP-IDF repository could not be cloned from the selected host. When both clone attempts fail, follow the git command's message. If that message is about a directory that already has files, see [Destination is not empty](#eim-e402).

**What to do:** Retry with another IDF mirror:

```bash
eim install -i v5.4.2 --idf-mirror https://git.espressif.com.cn
```

### EIM-E402: Destination is not empty {#eim-e402}

**What you see:** `gix clone failed and destination` / `is not empty`, or `Destination` / `exists and is not empty; refusing to clone over it`.

**Why:** The install path already has files. EIM will not delete them. A normal `eim install` continues into that directory, so act on this only if a later step fails.

**What to do:** Pass an empty `--path`, or move the existing directory aside, then rerun `eim`. If that directory is a finished ESP-IDF install, use `eim fix` ([EIM-E602](#eim-e602)) instead of cloning again.

### EIM-E403: Submodule update failed {#eim-e403}

**What you see:** `Submodule update failed:` or `git submodule update failed`

**Why:** The ESP-IDF clone succeeded and the submodule step did not.

**What to do:** Retry with `--idf-mirror`, the same flag as [Clone failed](#eim-e401).

## Filesystem

### EIM-E501: No space left, including a small /tmp {#eim-e501}

**What you see:** `No space left on device (os error 28)`, `Disk quota exceeded (os error 122)`, or a message that says the disk is full. The usual wrapper is `Failed to setup tools` during extraction. During offline installs it can also appear as one of:

- `Failed to create tools directory` / `Failed to create destination directory`
- `Failed to copy …` (git archive, python archive, IDF version, components)
- `Failed to execute robocopy`
- `Failed to create PowerShell profile:`

Those lines can also mean a permission problem, see [EIM-E502](#eim-e502).

**Why:** The disk is full, or `/tmp` is a small tmpfs (often 1–2 GB) while the home disk is not. EIM extracts tool archives in the temp directory and does not check its free space first.

**What to do:** Free disk space, or point the temp directory at a larger disk and rerun from that same terminal:

```bash
mkdir -p "$HOME/tmp_build"
export TMPDIR="$HOME/tmp_build"
df -h /tmp "$TMPDIR"
```

Remove `$HOME/tmp_build` after the install finishes. On Windows, clear space on the drive that holds the temp directory (`%TEMP%`).

### EIM-E502: Permission denied on a custom path {#eim-e502}

**What you see:** `Permission denied`, `Access is denied`, or `os error 13` / `os error 5` while creating the install directory, the tools directory, or `eim_idf.json`. The offline lines listed under [EIM-E501](#eim-e501) can also mean this.

**Why:** `--path`, the tools directory, or `--esp-idf-json-path` is not writable by this user.

**What to do:** Choose a directory your user can write to, and rerun. Do not fix this by running the whole install as root. Defaults are `~/.espressif` on Linux and macOS and `C:\Espressif` on Windows. Example:

```bash
eim install -i v5.4.2 --path "$HOME/esp/idf"
```

Running as an administrator is [EIM-E503](#eim-e503). Windows drivers that need an elevated process are [EIM-E802](#eim-e802).

### EIM-E503: Running as administrator {#eim-e503}

**What you see:** `Running as elevated user. This is not recommended.` On Windows the same warning adds that elevation is required to install drivers.

**Why:** The process is elevated. That is unnecessary for the install itself and can leave root-owned files.

**What to do:** Nothing if you meant to be elevated for a Windows driver install ([EIM-E802](#eim-e802)). Otherwise rerun as a normal user. The warning does not fail the command.

## Version management

### EIM-E601: No IDF version specified {#eim-e601}

**What you see:** `No IDF version specified and no version is selected. Please specify an IDF version.`

**Why:** `eim run` or `eim shell` was started with no installed version selected and no version argument. `eim install` does not select a version for these commands.

**What to do:**

```bash
eim list
eim select <name>
eim run "<command>" <name>
eim shell <name>
```

### EIM-E602: Already installed {#eim-e602}

**What you see:** `The IDF at path` / `is already installed` and `To fix the installation, run: eim fix --path <path>`

**Why:** That path already contains an ESP-IDF install recorded by EIM. The command finishes successfully and does not reinstall it.

**What to do:**

```bash
eim fix --path <path>
```

Use another `--path` when you want a second copy. `eim fix` keeps the original tools and features unless you pass new `--idf-tools` or `--idf-features`.

### EIM-E603: Fix failed or the directory is not an IDF checkout {#eim-e603}

**What you see:** `Invalid IDF directory:` or `Failed to fix IDF installation:`

**Why:** `--path` is not an ESP-IDF checkout, or the repair itself failed. The text after `Failed to fix IDF installation:` is the underlying error.

**What to do:** Pass the directory that contains the ESP-IDF repository. An empty directory is not enough:

```bash
eim fix --path <path-to-esp-idf>
```

Then follow the code of the underlying error ([EIM-E401](#eim-e401), [EIM-E302](#eim-e302), [EIM-E203](#eim-e203), and the others on this page).

## Offline

### EIM-E701: Offline archive {#eim-e701}

**What you see:** `Local archive path does not exist:`, `Failed to extract archive:`, or `Failed to load config from archive:`. `Git portable archive not found` and `Python archive not found` mean the archive has no file for this platform.

**Why:** The `.zst` path is wrong, the `.zst` was unpacked, or the archive has no file for this platform.

Unzip the downloaded `.zip`, but pass the `.zst` inside it to EIM as it is.

**What to do:** For a bad path or an extract failure, pass the compressed `.zst` file. If the message says the archive entry is missing, download the archive again.

```bash
eim install --use-local-archive /path/to/archive.zst
```

On Windows the offline installer can install Git and Python from the archive. That step always runs, and `--skip-prerequisites-check` does not skip it. On Linux and macOS those prerequisites must already be installed ([EIM-E101](#eim-e101)). Python for the wheels must be 3.11 to 3.14 ([EIM-E205](#eim-e205)).

**Not this one?**

- `Failed to create tools directory`, `Failed to copy`: disk space or permission ([EIM-E501](#eim-e501), [EIM-E502](#eim-e502)).
- `Failed to install git`: [EIM-E103](#eim-e103). `Failed to install python`: [EIM-E202](#eim-e202).
- Prerequisite check could not run: [EIM-E102](#eim-e102) or [EIM-E801](#eim-e801).

## Windows

### EIM-E801: PowerShell missing or blocked {#eim-e801}

**What you see:** one of

- `PowerShell script failed:` means PowerShell started and the script exited non-zero. The cause is in the `stderr:` line after it.
- `Unable to execute PowerShell to verify prerequisites` means neither `powershell.exe` nor `pwsh.exe` could be started.
- `running scripts is disabled on this system` when opening the IDF terminal, on an older build.

`Failed to create PowerShell profile:` is a file write, not a missing PowerShell. That one is [EIM-E501](#eim-e501) or [EIM-E502](#eim-e502).

**Why:** one of

- PowerShell is not on `PATH`, or cannot be started.
- The execution policy blocks scripts.

EIM is tested on Windows 11. Windows 10 is not a supported platform. If PowerShell cannot be detected there, installing PowerShell 7 usually fixes it.

**What to do:** Install PowerShell 7, put `pwsh.exe` on `PATH`, and rerun. On an older build, open the generated profile with execution policy bypass. The file name is `Microsoft.<version>.PowerShell_profile.ps1`:

```powershell
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -NoExit -File "<path-to>\Microsoft.<version>.PowerShell_profile.ps1"
```

Current EIM launches its own scripts with `-ExecutionPolicy Bypass`. The desktop shortcut and Windows Terminal profile do the same.

### EIM-E802: Drivers need an administrator {#eim-e802}

**What you see:** `Some drivers failed to install` or, in the GUI, `Insufficient permissions to install drivers. Please run the application as an administrator.` The log lines above the summary say which driver step failed.

**Why and what to do:**

- `Failed to install driver`: the install needs an elevated process. Start EIM as administrator and install the drivers again. Close it afterwards and use a normal user for `eim install` ([EIM-E503](#eim-e503)).
- `Failed to download driver`: a download failure. Follow [EIM-E301](#eim-e301).
- `Failed to create driver directory`: disk space or write permission ([EIM-E501](#eim-e501), [EIM-E502](#eim-e502)).

## Generic

### EIM-E900: Error executing CLI {#eim-e900}

**What you see:** A line that starts with `Error executing CLI:`, then exit code `1`.

**Why:** The command failed. The lines after it name the error, the log file, and this page.

**What to do:** Open the `EIM-E` code of the specific error, printed above the `Log:` line. `-v` prints more detail on the next run.
