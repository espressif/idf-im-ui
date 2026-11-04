# FAQ

## General Questions

### What about my privacy? Do you collect any data?

To help us improve the ESP-IDF Installation Manager, we collect some anonymous usage data. We are transparent about what we collect and you can opt-out at any time.

We collect:
- **Environment / System Info**: OS & version, distribution, architecture, kernel version, and EIM version.
- **Install context**: Which install flow was used (wizard / simple / offline / fix / CLI), whether the GUI or CLI was used, requested ESP-IDF version(s), and counts of selected features, tools, and targets.
- **Outcome**: Whether the install succeeded or failed and how long it took.
- **Error & Failure Tracking**: A coarse error kind (network, filesystem, git, python, dependency, configuration, user-cancelled, unknown), a 16-character SHA-256 fingerprint of the error message, and the error message with any filesystem paths, home-directory segments, and email addresses automatically redacted.

We **do not** collect filesystem paths, command-line arguments, environment variables, raw unredacted error messages, or any other personally identifiable information.

**How to disable it:**
- **GUI**: Uncheck the telemetry checkbox on the welcome screen.
- **CLI**: Use the `--do-not-track true` flag. The GUI subcommand (`eim gui`) honours this flag as well.

For more details, see the "Privacy and Data Collection" section in our documentation.

### Where can I find more information about ESP-IDF?
- [ESP-IDF Documentation](https://docs.espressif.com/projects/esp-idf/en/latest/)
- [ESP-IDF Forum](https://www.esp32.com/viewforum.php?f=20)

### Should I run the installer 'as admin'?

No. See [EIM-E503](./troubleshooting.md#eim-e503). Windows driver installation is the exception ([EIM-E802](./troubleshooting.md#eim-e802)).

### Does EIM support Windows on ARM?

No, EIM does not currently support Windows on ARM. This is because ESP-IDF itself does not support Windows ARM, so there is no ARM-native version of ESP-IDF available.

**Workaround**: You can run the Linux ARM64 version of EIM inside WSL (Windows Subsystem for Linux). We recommend using the CLI variant for this setup, as running the GUI from WSL adds complexity.

On Ubuntu-based WSL (which is the most common), you can install EIM CLI via APT:

```bash
# Install the Espressif signing key
sudo install -m 0755 -d /etc/apt/keyrings
sudo curl -fsSL https://dl.espressif.com/dl/eim/eim.gpg -o /etc/apt/keyrings/eim.gpg
sudo chmod 0644 /etc/apt/keyrings/eim.gpg

# Add the Espressif APT repository (deb822 with signed-by)
sudo curl -fsSL https://dl.espressif.com/dl/eim/eim.sources -o /etc/apt/sources.list.d/espressif.sources

# Update package lists
sudo apt update

# Install EIM CLI
sudo apt install eim-cli

# Install IDF itself (with defaults)
eim install
```

With this setup, ESP-IDF must be used from inside WSL.

### Can I install ESP-IDF to a custom location or a different hard drive?

Yes, you can install ESP-IDF to any location where you have full write permissions, including a different hard drive. However, on Windows (especially corporate machines with restricted permissions), simply using the `-p` parameter may not be enough.

EIM writes to multiple directories beyond just the IDF path. If any of these directories point to locations where you don't have full write permissions, the installation will fail with an "Access is denied" error.

**GUI users:** the Expert wizard's **Select Installation Path** step lets you rename the **Tool download** (default `dist`) and **Tool install** (default `tools`) folders via the *Use custom tool download / install folder locations (advanced)* option. The folders are created under the path you choose at the top of the step. There is also a *Delete temporary installation files after the install completes* option that mirrors the CLI's `--cleanup` flag.

To install to a completely custom location from the CLI, configure **all** relevant paths:

```bash
eim install -p D:\YourCustomPath\.espressif \
  --tool-install-folder-name D:\YourCustomPath\tools \
  --tool-download-folder-name D:\YourCustomPath\dist \
  --activation-script-path-override D:\YourCustomPath\tools \
  --esp-idf-json-path D:\YourCustomPath\tools
```

If you use `--esp-idf-json-path` during installation, you must also pass it to subsequent version management commands so they can find your `eim_idf.json`:

```bash
eim list --esp-idf-json-path D:\YourCustomPath\tools
eim select --esp-idf-json-path D:\YourCustomPath\tools
```

For a full list of configurable paths, see the [CLI Configuration](./cli_configuration.md) documentation.

> **Note:** If you change the location of the `eim_idf.json` configuration file, you may lose automatic IDE integration. Both VS Code and Espressif IDE expect this file to be in a known default location.

### Can I use an existing ESP-IDF Git repository with EIM?
Yes, simply run:
```bash
eim install -p PATH_TO_REPO
```
where PATH_TO_REPO is path to your IDF git repository on your local machine.


EIM is designed to work with existing ESP-IDF Git repositories on your filesystem. During the installation process (both GUI and CLI), when prompted for the installation path, you can specify the path to your existing ESP-IDF repository.

If EIM detects a valid ESP-IDF Git repository at the selected path, it will:
- **Utilize that existing repository**: It will not download a new copy or overwrite your existing files.
- **Ignore selected ESP-IDF versions**: Any specific ESP-IDF version you may have chosen in the GUI or via CLI arguments will be disregarded, as EIM will work with the version already present in your existing repository.

### Can I use EIM while bisecting ESP-IDF?

Yes, but you have to clone ESP-IDF yourself first. EIM's own clones are shallow and single-reference, so they contain no history to bisect. Clone the repository with its full history, install the toolchain with `eim install -p <repo> --version-name bisect`, and run `eim fix -p <repo>` at each bisect step to re-sync the tools and the Python environment with the newly checked-out revision. The full workflow, including a ready-made `git bisect run` script, is on the [Using EIM with git bisect](./git_bisect.md) page.

### How do I add more tools or features to an existing installation?

You don't need to reinstall from scratch to add a tool or optional feature (e.g. `docs`, `pytest`) you skipped the first time around.

**GUI:** Open the **Version Management** dashboard, then click **List Tools** or **List Features** on the version you want to change. The modal shows everything declared in that version's `tools.json`/`requirements.json`, including optional ones that aren't installed yet. If there are any available to add, an **Add more tools** / **Add more features** button appears next to the paths at the top of the modal — click it, check what you want, and confirm. This reinstalls the version with the newly selected tools/features added on top of what's already there; everything else is left untouched.

**CLI:** Use `eim fix` with `--idf-tools` and/or `--idf-features`, pointing at the existing installation:

```bash
eim fix -p /path/to/existing/esp-idf --idf-tools cmake,openocd --idf-features docs,pytest
```

You can also inspect what's currently installed first with [`eim list-tools`](./cli_commands.md#list-tools-command) and [`eim list-features`](./cli_commands.md#list-features-command).

`eim fix` preserves the configuration the version was originally installed with and only overrides what you explicitly pass, so any tools/features you don't mention stay as they were. See [CLI Commands](./cli_commands.md#fix-command) for details.

### How does offline installation work?

The offline installation allows you to install ESP-IDF without an internet connection. You need to download an offline installer artifact (a zip file) for your specific OS and ESP-IDF version. This artifact contains the installer and a `.zst` archive with all the necessary data. You then run the installer with the `--use-local-archive` flag, pointing to the `.zst` file. Remember **not** to unpack the `.zst` archive. Also, the offline installation currently requires **Python 3.11 to 3.14**. Python 3.14 is now fully supported on Linux, macOS, and Windows. For detailed instructions, please see the [Offline Installation](./offline_installation.md) guide.

### What is the offline_installer_builder-* binary for?

The `offline_installer_builder-*` binary is **not** required for performing offline installations. It is a separate tool that allows you to create custom offline installation archives for specific or all ESP-IDF versions. These archives contain everything needed for offline installation — ESP-IDF source, tools, Python wheels, and prerequisites — making them ideal for air-gapped environments, enterprise deployment, or ensuring your whole team has the same offline installable environment. You only need this tool if you want to create your own offline installer archives; for regular offline installation, you only need the pre-built offline installer artifacts available from the [Espressif Download Portal](https://dl.espressif.com/dl/eim/?tab=offline).

### "No space left on device" error despite having free disk space

`No space left on device` or `Disk quota exceeded` during tool extraction is [EIM-E501](./troubleshooting.md#eim-e501). A small `/tmp` is the usual cause. The page has the `TMPDIR` workaround.


### The installer says prerequisites are missing, but they are already installed

See [EIM-E101](./troubleshooting.md#eim-e101) when packages are missing, and [EIM-E102](./troubleshooting.md#eim-e102) when the check itself cannot run. `--skip-prerequisites-check true` is only for the second case, and only when the packages really are installed.

### "Failed to check prerequisites: Unable to execute PowerShell" error on Windows

`Unable to execute PowerShell` means PowerShell is missing or could not be started. See [EIM-E801](./troubleshooting.md#eim-e801). EIM is tested on Windows 11.

### Python SSL sanity check fails when using a system-wide proxy

`[FAIL] SSL/HTTPS` while a proxy is set is [EIM-E201](./troubleshooting.md#eim-e201). Bypass `localhost` and `127.0.0.1`.

### Why does EIM find the wrong Python on macOS?

A Finder launch can see Apple's Python while the terminal sees another one. See [EIM-E201](./troubleshooting.md#eim-e201).

### EIM detected the wrong Python — multiple Python installations found

EIM uses the first `python3` or `python` on `PATH` that passes the sanity check. See [EIM-E201](./troubleshooting.md#eim-e201).

## GUI-Specific Questions

### What is the new Report Issue button for?

The **Report Issue** button in the footer opens a modal that helps you quickly report bugs. It gathers key system information and directs you to the GitHub issue page where you can submit a detailed bug report, including your logs.

### Can I manage multiple installed ESP-IDF versions?

Yes. If you have an existing installation, the welcome screen will give you the option to **Manage Installations**, which takes you to the **Version Management** dashboard. From there, you can view, rename, reinstall, and delete your different ESP-IDF environments.

### Why do I see different welcome screens when I launch the installer?

The installer's welcome screen is dynamic. It will adapt to your environment:

  * If no installation is present, it will offer a **New Installation**.
  * If an offline archive is detected in the same directory, it will offer an **Offline Installation**.
  * If one or more versions are already installed, it will offer a **Manage Installations** button.

### Running Installer in Windows Sandbox

If you intend to run the ESP-IDF installer within a Windows Sandbox environment, you might encounter issues if the 'WebView2 Runtime' is not present in the sandbox. The installer relies on WebView2 for its graphical user interface.

To successfully run the installer in Windows Sandbox, you will need to first install the 'WebView2 Runtime' within the sandbox environment. You can obtain the installer for the WebView2 Runtime from the following link:

[https://developer.microsoft.com/](https://developer.microsoft.com/en-us/microsoft-edge/webview2/?form=MA13LH#download)

Download and run the appropriate WebView2 Runtime installer inside the Windows Sandbox before attempting to run the ESP-IDF installer. This will ensure that the necessary components for the GUI are available, allowing the ESP-IDF installer to function correctly.

### Can I return to the main page of the installer?
Yes, you can return to the main page by clicking on the Espressif logo in the top-left corner. However, this is not possible during the final steps of installation.

### What if the installation fails in simplified mode?

Open the log (`eim_gui.log`, see [Reporting an Issue](./issue_reporting.md)) and match the `EIM-E` ID in [Troubleshooting](./troubleshooting.md). You can retry after that, or switch to expert mode.

## CLI-Specific Questions

### What if I want to install a specific version of IDF that is not listed?
You can install any tagged version of ESP-IDF using the `-i` or `--idf-versions` flag:
```bash
eim install -i v5.3.1
```

### I am getting the error `/lib64/libm.so.6: version 'GLIBC_2.38' not found`. What should I do?
This error indicates that your Linux system is using an outdated version of the GNU C Library (glibc). However, since the CLI is statically linked, it does not depend on the system's glibc and should not encounter this issue. If you continue to experience problems, consider updating your Linux distribution to a newer version.


### How can I use EIM in CI/CD pipelines?
For GitHub Actions, use the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action). For other CI/CD systems, use the headless mode with appropriate configuration. See [Headless Usage](./headless_usage.md) for details.

### The installer crashes on startup / appears to crash immediately after I double-click it

If you downloaded the CLI-only version (e.g., `eim-cli-*.exe`) and double-clicked the executable, this behavior is expected. CLI releases must be run from a terminal/command prompt rather than double-clicking. When you double-click the executable, it opens a terminal window briefly, displays help information, and then closes immediately, which may appear as if the installer crashed. Instead, open PowerShell or Command Prompt, navigate to the file location, and run it with `.\eim-cli-*.exe --help`.

### What shells are supported for activation scripts?

On Linux and macOS, EIM creates activation scripts for both **Bash** and **Fish** shells:
- **Bash**: `activate_idf_{version}.sh` - works in Sh, Bash, Dash and Zsh
- **Fish**: `activate_idf_{version}.fish` - for Fish shell users

On Windows, EIM creates a **PowerShell** profile script (`Microsoft.{version}.PowerShell_profile.ps1`). Additionally, you can optionally create a CMD batch file by setting `create_bat_activation_script = true` in your configuration.

### Should I use CMD (Command Prompt) batch files?

**No.** We strongly recommend using **PowerShell** instead of CMD (Command Prompt) on Windows. The batch file (`.bat`) support exists only for backward compatibility and will be abandoned in a future release.

Reasons to use PowerShell:
- Better environment variable handling
- More modern syntax and features
- Active development and support
- Required for many modern tools

To use PowerShell, simply run the generated PowerShell profile script - it is created automatically during installation.

### Open IDF Terminal fails with "running scripts is disabled on this system" on Windows

`running scripts is disabled on this system` is a PowerShell execution-policy block. See [EIM-E801](./troubleshooting.md#eim-e801).

## More Questions?

If you have additional questions, you can:
1. Visit the [ESP32 forum](https://esp32.com/)
2. Check the [EIM repository](https://github.com/espressif/idf-im-ui)
