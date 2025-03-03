# FAQ

## General Questions

### Where can I find more information about ESP-IDF?
- [ESP-IDF Documentation](https://docs.espressif.com/projects/esp-idf/en/latest/)
- [ESP-IDF Forum](https://www.esp32.com/viewforum.php?f=20)

### Should I run the installer 'as admin'?
No, the installer does not require elevated rights and should **not** be run as an administrator. Running the installer with admin privileges is unnecessary and could lead to unintended permission issues.

## GUI-Specific Questions

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
eim -i v4.4.1
```

### I am getting the error `/lib64/libm.so.6: version 'GLIBC_2.38' not found`. What should I do?
This error indicates that your Linux system is using an outdated version of the GNU C Library (glibc). You will need to update your Linux distribution to a newer version that includes a more recent glibc.

### How can I use EIM in CI/CD pipelines?
For GitHub Actions, use the [install-esp-idf-action](https://github.com/espressif/install-esp-idf-action). For other CI/CD systems, use the headless mode with appropriate configuration. See [Headless Usage](./headless_usage.md) for details.

## More Questions?

If you have additional questions, you can:
1. Visit the [ESP32 forum](https://esp32.com/)
2. Check the [EIM repository](https://github.com/espressif/idf-im-ui)
