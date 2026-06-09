# After Installing

## Windows

On Windows, the installer creates a desktop icon labeled **IDF\_PowerShell**. Clicking this icon will launch PowerShell with the environment set up, allowing you to start using ESP-IDF immediately. If you've installed multiple versions, you will have multiple icons, one for each version.

## macOS & Linux

In the installation directory you selected, there will be activation scripts for both **Bash** and **Fish** shells:

- **Bash**: `activate_idf_{version}.sh` - works in Sh, Bash, Dash and Zsh
- **Fish**: `activate_idf_{version}.fish` - for Fish shell users

If you've installed multiple versions of ESP-IDF, there will be separate scripts for each version.

> **Note**
> The script must be sourced and not executed\!

## Deactivating the Environment

Each activation script has a matching **deactivation** script in the same directory. Sourcing it undoes the activation: it unsets `IDF_PATH`, `IDF_TOOLS_PATH`, `IDF_PYTHON_ENV_PATH`, `IDF_COMPONENT_LOCAL_STORAGE_URL`, `ESP_ROM_ELF_DIR`, `OPENOCD_SCRIPTS`, `ESP_IDF_VERSION` and any toolchain variables the activation set; strips the IDF entries from `PATH`; deactivates the Python virtual environment; and removes the `idf.py`, `esptool`, `espefuse`, `espsecure`, `otatool`, `parttool` shell functions/aliases and the `idf.py` tab completion.

- **Bash / POSIX shell**:
    ```bash
    . /path/to/tools/deactivate_idf_{version}.sh
    ```
- **Fish**:
    ```fish
    source /path/to/tools/deactivate_idf_{version}.fish
    ```
- **PowerShell**:
    ```powershell
    . 'C:\Espressif\tools\Microsoft.{version}.PowerShell_deactivate.ps1'
    ```
- **CMD** (only when the matching `_profile.bat` was generated, e.g. when the offline installer was used):
    ```cmd
    call C:\Espressif\tools\Microsoft.{version}_deactivate.bat
    ```

> **Note**
> Like the activation script, the deactivation script must be sourced (or `call`-ed for CMD), not executed directly.

The deactivation script is removed automatically when you uninstall the matching IDF version with `eim remove {version}` (CLI) or the Version Management dashboard (GUI).
