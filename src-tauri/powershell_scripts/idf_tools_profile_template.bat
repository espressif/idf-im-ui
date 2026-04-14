@echo off

REM Capture eim path early (before PATH changes)
set _EimBinPath=
for /f "delims=" %%i in ('where eim 2^>nul') do set "_EimBinPath=%%i" & goto :eim_found
:eim_found

{{env_var_pairs}}

REM Parse IDF version from CMake
set IdfVersion=
for /f "usebackq tokens=1,2 delims==#" %%a in ("{{idf_path}}\tools\cmake\version.cmake") do (
    if "%%a"=="set(IDF_VERSION_MAJOR" call set "IdfVersionMajor=%%b"
    if "%%a"=="set(IDF_VERSION_MINOR" call set "IdfVersionMinor=%%b"
)
if defined IdfVersionMajor if defined IdfVersionMinor set IdfVersion=%IdfVersionMajor%.%IdfVersionMinor%

REM If -e parameter is provided, print variables and exit
if "%~1"=="-e" goto print_env

REM Set environment variables
set ESP_IDF_VERSION=%IdfVersion%

REM Set system PATH - add all IDF toolchain directories
set PATH={{add_paths_extras}};{{idf_path}}\tools;%PATH%

REM Activate Python environment (call the activate script)
if exist "{{idf_python_env_path}}\Scripts\activate.bat" call "{{idf_python_env_path}}\Scripts\activate.bat"

REM Re-prepend IDF tools after venv activation (venv modifies PATH)
set PATH={{add_paths_extras}};{{idf_path}}\tools;%PATH%

REM Create command aliases (CMD equivalent of PowerShell functions)
doskey idf.py={{python_bin_path}} {{idf_path}}\tools\idf.py $*
doskey esptool.py={{python_bin_path}} {{idf_path}}\components\esptool_py\esptool\esptool.py $*
doskey esptool={{python_bin_path}} {{idf_path}}\components\esptool_py\esptool\esptool.py $*
doskey espefuse.py={{python_bin_path}} {{idf_path}}\components\esptool_py\esptool\espefuse.py $*
doskey espefuse={{python_bin_path}} {{idf_path}}\components\esptool_py\esptool\espefuse.py $*
doskey espsecure.py={{python_bin_path}} {{idf_path}}\components\esptool_py\esptool\espsecure.py $*
doskey espsecure={{python_bin_path}} {{idf_path}}\components\esptool_py\esptool\espsecure.py $*
doskey otatool.py={{python_bin_path}} {{idf_path}}\components\app_update\otatool.py $*
doskey parttool.py={{python_bin_path}} {{idf_path}}\components\partition_table\parttool.py $*

REM Display setup information
echo IDF Command Prompt Environment
echo ---------------------------------
echo Environment variables set:
echo IDF_PATH: %IDF_PATH%
echo IDF_TOOLS_PATH: %IDF_TOOLS_PATH%
echo IDF_PYTHON_ENV_PATH: %IDF_PYTHON_ENV_PATH%
echo.
echo Custom commands available:
echo idf.py - Use this to run IDF commands (e.g., idf.py build)
echo esptool.py
echo espefuse.py
echo espsecure.py
echo otatool.py
echo parttool.py
echo.
echo Python environment activated.
echo You can now use IDF commands and Python tools.

REM Sync selection with eim_idf.json for IDEs (silent on failure)
if defined _EimBinPath (
    call %_EimBinPath% select "{{idf_version}}" 2>nul
)

goto :end

:print_env
echo PATH={{add_paths_extras}};{{idf_path}}\tools;%PATH%
echo ESP_IDF_VERSION=%IdfVersion%
echo SYSTEM_PATH={{current_system_path}}
{{env_var_pairs_print}}
goto :end

:end
REM Note: We intentionally don't use endlocal to allow environment changes to persist
REM in the parent CMD session when the batch file is called with 'call'
