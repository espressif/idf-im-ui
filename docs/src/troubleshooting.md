# Troubleshooting

A failure keeps its original sentence, then says what to do and names an `EIM-E` number. Open that section below. A warning that says the command is continuing is not a failure.

## Logs

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

If the data directory cannot be created, the CLI falls back to `eim.log` in the current directory.

Increase detail with `-v` (repeat it for more).

## Prerequisites

### Missing prerequisites {#eim-e101}

**Symptom:** `Please install the missing prerequisites and try again.`

**Cause:** A required package is missing. On Linux and macOS, EIM never installs prerequisites for you. On Windows it installs them only when you agree, or when `--install-all-prerequisites true` is set.

**Fix:** Install the packages named above with your system package manager, then rerun the same `eim` command. The names for your system are on the [Prerequisites](./prerequisites.md) page. On Windows, `eim install --install-all-prerequisites true` installs Git and Python. That flag does nothing on Linux or macOS. QEMU libraries are [EIM-E104](#eim-e104).

### Prerequisite check could not run {#eim-e102}

**Symptom:** one of

- `The installer is unable to verify the prerequisites. Please try again with the --skip-prerequisites-check flag.`
- `Basic shell execution failed on this system. Unable to verify prerequisites.`
- `A system error occurred while checking prerequisites:`

**Cause:** EIM could not execute its prerequisite probe (shell failure, or an error while checking). This is not the same as a package being absent.

**Fix:** Restore a working shell (`bash` on Linux and macOS, `powershell.exe` or `pwsh.exe` on Windows), or skip the check only if the packages are already installed:

```bash
eim install --skip-prerequisites-check true
```

Do not use `--install-all-prerequisites` for this ID. That flag installs packages. It does not repair a probe that cannot run. It also does not skip the Windows offline prerequisite step, which always runs first ([Offline archive](#eim-e701)). On Windows, PowerShell missing or blocked is `EIM-E801`.

### Prerequisite installation failed {#eim-e103}

**Symptom:** `Prerequisite installation failed`, `Something went terribly wrong. These prerequisites were not installed:`, `Failed to install python:`, or `Failed to install git:`. A package-manager panic starts with `Failed to install`.

**Cause:** EIM tried to install prerequisites and at least one package did not install.

**Fix:** Read the package-manager output, install the named packages yourself, then rerun `eim`. See [Missing prerequisites](#eim-e101). On Windows, `--install-all-prerequisites true` is what asks EIM to install them.

### QEMU prerequisites {#eim-e104}

**Symptom:** one of

- `The following prerequisites for QEMU are not satisfied`
- `Failed to check QEMU prerequisites.`
- `QEMU prerequisites are not satisfied and you selected QEMU for installation.`

**Cause:** QEMU was selected and its libraries are missing. The message lists them after `Missing:`.

**Fix:** Install those packages, or rerun without QEMU. The names are on the [Prerequisites](./prerequisites.md) page. EIM does not install these libraries for you.

## Python

Supported interpreters for a normal install are Python **3.10, 3.11, 3.12, 3.13, and 3.14** (`>= 3.10.0` and `< 3.15.0`). Offline archives ship wheels for **3.11 to 3.14** on Linux, macOS, and Windows. See [Offline install](#eim-e701).

### Python sanity check failed {#eim-e201}

**Symptom:** `Python sanity check failed. See the [FAIL] entries above for details.`

Each failed check is a `[FAIL]` line followed by `Hint:`.

**Cause:** The selected Python is missing, too old, too new, or lacks pip, venv, the standard library, ctypes, or SSL. On Windows the Microsoft Store alias also fails this check.

**Fix:** Follow the `Hint:` line for each `[FAIL]`. Install Python 3.10 to 3.14 from [python.org](https://www.python.org/downloads/) when the hint says to reinstall.

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

### Python installation failed or was refused {#eim-e202}

**Symptom:** `Python installation failed` or `Please install Python3 with pip, venv, and SSL support and try again`

**Cause:** EIM could not install Python, or you declined the install and no suitable interpreter is present.

**Fix:** Install Python 3.10 to 3.14 with pip, venv, and SSL from [python.org](https://www.python.org/downloads/), then rerun `eim`. On Windows, `eim install --install-all-prerequisites true` asks EIM to install Python.

### Virtual environment or requirements failed {#eim-e203}

**Symptom:** one of

- `failed to ensure venv path:`
- `failed to create venv:`
- `failed to install requirements from file`

**Cause:** The tools directory is not usable, `python -m venv` failed, or `pip install -r` failed. A pip download problem is [PyPI download](#eim-e303). A missing offline wheel is [Offline wheels](#eim-e205).

**Fix:** Use Python 3.10 to 3.14 with the `venv` module and rerun `eim`. Point `--path` at a directory your user can write to. For a package-index failure, rerun with `--pypi-mirror` as in [PyPI download](#eim-e303).

### Constraints file download failed {#eim-e204}

**Symptom:** `Failed to download constraints file:`

**Cause:** EIM downloads `espidf.constraints.<version>.txt` from `dl.espressif.com`. This warning does not stop the installation. `--mirror`, `--idf-mirror`, and `--pypi-mirror` do not replace that host.

**Fix:** Retry the same command when `dl.espressif.com` is reachable. For an air-gapped machine, use an offline archive (`EIM-E701`); the archive copies this file instead of downloading it.

### Offline wheels missing {#eim-e205}

**Symptom:** `Could not find wheels for Python` or `No compatible wheel directory found for Python`

**Cause:** The offline archive has no `wheels_py*` directory for the Python that was detected. Published archives include wheels for Python 3.11, 3.12, 3.13, and 3.14.

**Fix:** Install one of those Python versions and rerun against the same `.zst`, or rebuild the archive with the offline archive builder so it contains wheels for the version you need.

## Network

### Version list fetch failed {#eim-e301}

**Symptom:** `Error downloading IDF versions from` or, in the GUI, `Unable to fetch available ESP-IDF versions.` A broken bundled list says `Error parsing cached IDF versions JSON:`.

**Cause:** `https://dl.espressif.com/dl/esp-idf/idf_versions.json` could not be downloaded. EIM then uses the version list built into the binary and the command continues. Mirror flags do not change this URL. The warning is not a failed run.

**Fix:** Retry later. You can still install a version you already know:

```bash
eim install -i v5.4.2
```

If the message is `Error parsing cached IDF versions JSON`, reinstall EIM. The copy shipped inside the binary could not be read.

### Tool download failed {#eim-e302}

**Symptom:** `Download attempt` followed by `failed for`.

**Cause:** One download attempt failed. EIM retries. The URL in the message is the host that failed. Change mirrors only if the command then exits `1`.

**Fix:** If the run fails after the retries, rerun with a mirror closer to you. Do not restart on the first warning.

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

Python package downloads are `--pypi-mirror` (`EIM-E303`).

### PyPI download failed {#eim-e303}

**Symptom:** pip text inside `failed to install requirements from file` (`EIM-E203`) about an index, a timeout, or a connection to a package host.

**Cause:** pip could not download from the Espressif package index or the selected PyPI mirror.

**Fix:** Rerun with `--pypi-mirror` set to one of:

- `https://pypi.org/simple`
- `https://mirrors.aliyun.com/pypi/simple`
- `https://pypi.tuna.tsinghua.edu.cn/simple`
- `https://pypi.mirrors.ustc.edu.cn/simple`

```bash
eim install -i v5.4.2 --pypi-mirror https://pypi.tuna.tsinghua.edu.cn/simple
```

## Git

### Clone failed {#eim-e401}

**Symptom:** `git clone failed` or `Both gix and system git failed to clone ESP-IDF.`

**Cause:** The ESP-IDF repository could not be cloned from the selected host. When both clone attempts fail, follow the git command's message. If that message is about a directory that already has files, see [Destination is not empty](#eim-e402).

**Fix:** Retry with another IDF mirror:

```bash
eim install -i v5.4.2 --idf-mirror https://git.espressif.com.cn
```

### Destination is not empty {#eim-e402}

**Symptom:** `gix clone failed and destination` / `is not empty`, or `Destination` / `exists and is not empty; refusing to clone over it`.

**Cause:** The install path already has files. EIM will not delete them. A normal `eim install` continues into that directory. Choose another path, or run `eim fix`, only if a later step fails.

**Fix:** Pass an empty `--path`, or move the existing directory aside, then rerun `eim`. If that directory is a finished ESP-IDF install, use `eim fix` (`EIM-E602`) instead of cloning again.

### Submodule update failed {#eim-e403}

**Symptom:** `Submodule update failed:` or `git submodule update failed`

**Cause:** The ESP-IDF clone succeeded and the submodule step did not.

**Fix:** Retry with `--idf-mirror`, the same flag as [Clone failed](#eim-e401).

## Filesystem

### No space left, including a small /tmp {#eim-e501}

**Symptom:** `No space left on device (os error 28)`, `Disk quota exceeded (os error 122)`, or a message that says the disk is full. The usual wrapper is `Failed to setup tools` during extraction. Offline copy failures that can also be a full disk are `Failed to create tools directory`, `Failed to create destination directory`, `Failed to copy git archive`, `Failed to copy python archive`, `Failed to copy IDF version`, `Failed to copy some IDF versions`, `Failed to copy components from offline archive`, `Failed to copy Root Managed Components`, and `Failed to execute robocopy`. `Failed to create PowerShell profile:` is a file write and belongs here too. Those lines point at E501 or E502.

**Cause:** The disk is full, or `/tmp` is a small tmpfs (often 1–2 GB) while the home disk is not. EIM extracts tool archives in the temp directory and does not check its free space first.

**Fix:** Free disk space, or point the temp directory at a larger disk and rerun from that same terminal:

```bash
mkdir -p "$HOME/tmp_build"
export TMPDIR="$HOME/tmp_build"
df -h /tmp "$TMPDIR"
```

Remove `$HOME/tmp_build` after the install finishes. On Windows, clear space on the drive that holds the temp directory (`%TEMP%`).

### Permission denied on a custom path {#eim-e502}

**Symptom:** `Permission denied`, `Access is denied`, or `os error 13` / `os error 5` while creating the install directory, the tools directory, or `eim_idf.json`. The same offline copy lines listed under [EIM-E501](#eim-e501) (`Failed to create tools directory`, `Failed to copy`, `Failed to execute robocopy`) also point here when the path is not writable. `Failed to create PowerShell profile:` is the same kind of file write.

**Cause:** `--path`, the tools directory, or `--esp-idf-json-path` is not writable by this user.

**Fix:** Choose a directory your user can write to, and rerun. Do not fix this by running the whole install as root. Defaults are `~/.espressif` on Linux and macOS and `C:\Espressif` on Windows. Example:

```bash
eim install -i v5.4.2 --path "$HOME/esp/idf"
```

Running as an administrator is `EIM-E503`. Windows drivers that need an elevated process are `EIM-E802`.

### Running as administrator {#eim-e503}

**Symptom:** `Running as elevated user. This is not recommended.` On Windows the same warning adds that elevation is required to install drivers.

**Cause:** The process is elevated. That is unnecessary for the install itself and can leave root-owned files.

**Fix:** No action if you meant to be elevated for a Windows driver install (`EIM-E802`). Otherwise stop and rerun as a normal user. The warning does not fail the command.

## Version management

### No IDF version specified {#eim-e601}

**Symptom:** `No IDF version specified and no version is selected. Please specify an IDF version.`

**Cause:** `eim run` or `eim shell` was started with no installed version selected and no version argument. `eim install` does not select a version for these commands.

**Fix:**

```bash
eim list
eim select <name>
eim run "<command>" <name>
eim shell <name>
```

### Already installed {#eim-e602}

**Symptom:** `The IDF at path` / `is already installed` and `To fix the installation, run: eim fix --path <path>`

**Cause:** That path already contains an ESP-IDF install recorded by EIM. The command finishes successfully and does not reinstall it.

**Fix:**

```bash
eim fix --path <path>
```

Use another `--path` when you want a second copy. `eim fix` keeps the original tools and features unless you pass new `--idf-tools` or `--idf-features`.

### Fix failed or the directory is not an IDF checkout {#eim-e603}

**Symptom:** `Invalid IDF directory:` or `Failed to fix IDF installation:`

**Cause:** `--path` is not an ESP-IDF checkout, or the repair itself failed. The text after `Failed to fix IDF installation:` is the underlying error.

**Fix:** Pass the directory that contains the ESP-IDF repository. An empty directory is not enough:

```bash
eim fix --path <path-to-esp-idf>
```

Then follow the ID of the underlying error (`EIM-E401`, `EIM-E302`, `EIM-E203`, and the others on this page).

## Offline

### Offline archive {#eim-e701}

**Symptom:** `Local archive path does not exist:`, `Failed to extract archive:`, or `Failed to load config from archive:`. `Git portable archive not found` and `Python archive not found` mean the archive has no file for this platform.

**Cause:** The `.zst` path is wrong, the archive was unpacked, or the archive is missing the file this platform needs. Unzip the downloaded zip. Do not unpack the `.zst` inside it. On Windows the offline prerequisite step always runs, and `--skip-prerequisites-check` does not skip it. A copy or directory failure (`Failed to create tools directory`, `Failed to copy`) is disk space or write permission (`EIM-E501`, `EIM-E502`), not this ID. A probe that cannot run is `EIM-E102` or `EIM-E801`. `Failed to install git` is `EIM-E103`. `Failed to install python` is `EIM-E202`.

**Fix:** For a bad path or an extract failure, pass the compressed `.zst` file. Do not unpack it. If the message says the archive entry is missing, download the archive again.

```bash
eim install --use-local-archive /path/to/archive.zst
```

On Windows the offline installer can install Git and Python from the archive. On Linux and macOS those prerequisites must already be installed (`EIM-E101`). Python for the wheels must be 3.11 to 3.14 (`EIM-E205`).

## Windows

### PowerShell missing or blocked {#eim-e801}

**Symptom:** `PowerShell script failed:` means PowerShell started and the script exited non-zero. The cause is in the `stderr:` line that follows that message. `Unable to execute PowerShell to verify prerequisites` means `powershell.exe` and `pwsh.exe` could not be started. An older build can also show `running scripts is disabled on this system` when opening the IDF terminal. `Failed to create PowerShell profile:` is a file write, not a missing PowerShell. That one is [EIM-E501](#eim-e501) or [EIM-E502](#eim-e502).

**Cause:** PowerShell is not on `PATH`, or the execution policy blocks scripts. EIM is tested on Windows 11. Windows 10 is not a supported platform. If PowerShell cannot be detected there, installing PowerShell 7 and putting `pwsh.exe` on `PATH` is what usually makes the probe succeed. Windows 10 ships with Windows PowerShell 5.1, and that is not enough when the probe cannot see it.

**Fix:** Install PowerShell 7, put `pwsh.exe` on `PATH`, and rerun. On an older build, open the generated profile with execution policy bypass. The file name is `Microsoft.<version>.PowerShell_profile.ps1`:

```powershell
powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -NoExit -File "<path-to>\Microsoft.<version>.PowerShell_profile.ps1"
```

Current EIM launches its own scripts with `-ExecutionPolicy Bypass`. The desktop shortcut and Windows Terminal profile do the same.

### Drivers need an administrator {#eim-e802}

**Symptom:** `Some drivers failed to install` or, in the GUI, `Insufficient permissions to install drivers. Please run the application as an administrator.`

**Cause:** `Failed to install driver` needs an elevated process. `Failed to download driver` is a download failure. Use [EIM-E301](#eim-e301) for that line, not this one. `Failed to create driver directory` is disk space or write permission ([EIM-E501](#eim-e501), [EIM-E502](#eim-e502)). The summary `Some drivers failed to install` tells you which of those lines to read. `Failed to install drivers:` does not add a second fix.

**Fix:** If the line is `Failed to install driver`, start EIM as administrator and install the drivers again. Close it afterwards and use a normal user for `eim install` (`EIM-E503`). If the line is `Failed to download driver`, follow [EIM-E301](#eim-e301).

## Generic

### Error executing CLI {#eim-e900}

**Symptom:** A line that starts with `Error executing CLI:`, then exit code `1`.

**Cause:** The command failed. The message names the error and the log for this operating system.

**Fix:** Open the `EIM-E` number in the message. `-v` prints more detail on the next run.

