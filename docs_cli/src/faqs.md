# FAQs

Here are some frequently asked questions about the ESP-IDF Installation Manager (EIM):

---

### **Should I run the installer 'as admin'?**

No, the installer does not require elevated rights and should **not** be run as an administrator. Running the installer with admin privileges is unnecessary and could lead to unintended permission issues. The installer is designed to work with standard user permissions.

---

### **What if I want to install a specific version of IDF that is not listed?**

The EIM allows you to install **any tagged version** of ESP-IDF, even if it is not listed in the default options. To install a specific version, simply specify the tag name using the `-i` or `--idf-version` flag followed by the tag name. For example:

```bash
./eim -i v4.4.1
```

This will install the ESP-IDF version tagged as v4.4.1. You can find all available tags in the [ESP-IDF GitHub repository](https://github.com/espressif/esp-idf/tags).

---

### I am getting the error `/lib64/libm.so.6: version 'GLIBC_2.38' not found`. What should I do?

This error indicates that your Linux system is using an outdated version of the GNU C Library (glibc). Unfortunately, the ESP-IDF Installation Manager does not support such old versions of glibc. To resolve this issue, you will need to update your Linux distribution to a newer version that includes a more recent glibc.

We apologize for any inconvenience this may cause, but supporting older versions of glibc is not feasible due to compatibility and security concerns.

---

### More Questions?

As of the release date of this documentation, no additional questions have been asked about the EIM. The next version of this documentation will include answers to all the questions asked before its release. If you have further questions, feel free to reach out to the [ESP-IDF community](https://esp32.com/) or check the [GitHub repository](https://github.com/espressif/idf-im-cli) for updates.
