# Version Management

The Version Management dashboard is a new GUI feature that gives you full control over your installed ESP-IDF environments. You can access it from the welcome page by clicking **Open Dashboard** when an installation is already present.

On this page, you can see a list of all your installed ESP-IDF versions. For each version, you can:

  * **Open IDF Terminal**: Open terminal with the appropriate IDF activated
  * **Rename**: Change the name of the installed version.
  * **Fix/Reinstall**: Rerun the installation process to repair a corrupted environment. This preserves the target, features and tools the version was originally installed with, so you don't lose any customization.
  * **Open Folder**: Open the installation directory in your file explorer.
  * **List Tools**: Open a modal showing every tool declared in the version's `tools.json`, its installed version(s), and whether it's up to date. See **Adding More Tools** below.
  * **Delete**: Uninstall the specific ESP-IDF version.

### Adding More Tools to an Existing Installation

Open **List Tools** for a version to see its full tool catalog, including optional tools that weren't installed. If any optional tools are available to add, an **Add more tools** button appears next to the IDF/tools paths at the top of the modal. Click it, check the tools you want, and confirm — this triggers a repair (the same mechanism as **Fix**) that reinstalls the version with the newly selected tools added on top of what's already there, without touching your existing configuration otherwise.

At the bottom of the page, you'll also find options to:

  * **Install New Version**: Launch a new installation wizard.
  * **Purge All**: Delete all installed ESP-IDF versions.

![Welcome - version already present](./screenshots/version_management.png)
