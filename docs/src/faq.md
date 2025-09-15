# FAQ

## General Questions

### Where can I find more information about ESP-IDF?
- [ESP-IDF Documentation](https://docs.espressif.com/projects/esp-idf/en/latest/)
- [ESP-IDF Forum](https://www.esp32.com/viewforum.php?f=20)

### Should I run the installer 'as admin'?
No, the installer does not require elevated rights and should **not** be run as an administrator. Running the installer with admin privileges is unnecessary and could lead to unintended permission issues.

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

### How does offline installation work?

The offline installation allows you to install ESP-IDF without an internet connection. You need to download an offline installer artifact (a zip file) for your specific OS and ESP-IDF version. This artifact contains the installer and a `.zst` archive with all the necessary data. You then run the installer with the `--use-local-archive` flag, pointing to the `.zst` file. Remember **not** to unpack the `.zst` archive. Also, the offline installation currently requires **Python 3.11**. For detailed instructions, please see the [Offline Installation](./offline_installation.md) guide.

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
You can either:
- Click "Try again" after resolving any issues
- Switch to expert mode for more control over the installation process
- Check the logs folder for detailed information about the failure

## CLI-Specific Questions

### What if I want to install a specific version of IDF that is not listed?
You can install any tagged version of ESP-IDF using the `-i` or `--idf-version` flag:
```bash
eim install -i v4.4.1
```

### I am getting the error `/lib64/libm.so.6: version 'GLIBC_2.38' not found`. What should I do?
This error indicates that your Linux system is using an outdated version of the GNU C Library (glibc). However, since the CLI is statically linked, it does not depend on the system's glibc and should not encounter this issue. If you continue to experience problems, consider updating your Linux distribution to a newer version.


### How can I use EIM in CI/CD pipelines?
For GitHub Actions, use the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action). For other CI/CD systems, use the headless mode with appropriate configuration. See [Headless Usage](./headless_usage.md) for details.

## More Questions?

If you have additional questions, you can:
1. Visit the [ESP32 forum](https://esp32.com/)
2. Check the [EIM repository](https://github.com/espressif/idf-im-ui)
