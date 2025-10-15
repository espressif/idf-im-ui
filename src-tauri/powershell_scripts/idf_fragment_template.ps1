# ESP-IDF PowerShell Profile Fragment
# Version: {{idf_version}}
# Auto-generated - Installed via ESP-IDF Manager

{{env_var_pairs}}

function Parse-CMakeVersion {
    $cmakeFile = "{{idf_path}}\tools\cmake\version.cmake"

    if (-not (Test-Path $cmakeFile)) {
        Write-Warning "CMake version file not found at: $cmakeFile"
        return "{{idf_version}}"
    }

    $major = $null
    $minor = $null

    try {
        $content = Get-Content $cmakeFile -ErrorAction Stop

        foreach ($line in $content) {
            $line = $line.Trim()

            if ($line -match '^set\(IDF_VERSION_MAJOR') {
                if ($line -match '\d+') {
                    $major = $matches[0]
                }
            }
            elseif ($line -match '^set\(IDF_VERSION_MINOR') {
                if ($line -match '\d+') {
                    $minor = $matches[0]
                }
            }
        }

        if ($null -eq $major -or $null -eq $minor) {
            Write-Warning "Could not parse version, using: {{idf_version}}"
            return "{{idf_version}}"
        }

        return "$major.$minor"
    }
    catch {
        Write-Warning "Failed to read CMake version file: $($_.Exception.Message)"
        return "{{idf_version}}"
    }
}

$IdfVersion = Parse-CMakeVersion

# Set environment variables
$env:ESP_IDF_VERSION = "$IdfVersion"
$env_var_pairs.GetEnumerator() | ForEach-Object {
    Set-Item -Path "env:$($_.Key)" -Value $_.Value -ErrorAction SilentlyContinue
}

# Prepend ESP-IDF tools to PATH (only if not already present)
$idfToolsPath = "{{add_paths_extras}}"
if ($env:PATH -notlike "*$idfToolsPath*") {
    $env:PATH = "$idfToolsPath;$env:PATH"
}

# Define ESP-IDF helper functions
function global:Invoke-idfpy {
    & "{{idf_python_env_path}}\Scripts\python.exe" "{{idf_path}}\tools\idf.py" @args
}

function global:esptool.py {
    & "{{idf_python_env_path}}\Scripts\python.exe" "{{idf_path}}\components\esptool_py\esptool\esptool.py" @args
}

function global:espefuse.py {
    & "{{idf_python_env_path}}\Scripts\python.exe" "{{idf_path}}\components\esptool_py\esptool\espefuse.py" @args
}

function global:espsecure.py {
    & "{{idf_python_env_path}}\Scripts\python.exe" "{{idf_path}}\components\esptool_py\esptool\espsecure.py" @args
}

function global:otatool.py {
    & "{{idf_python_env_path}}\Scripts\python.exe" "{{idf_path}}\components\app_update\otatool.py" @args
}

function global:parttool.py {
    & "{{idf_python_env_path}}\Scripts\python.exe" "{{idf_path}}\components\partition_table\parttool.py" @args
}

# Create alias for idf.py
New-Alias -Name idf.py -Value Invoke-idfpy -Force -Scope Global -ErrorAction SilentlyContinue

# Python venv activation
. "{{idf_python_env_path}}\Scripts\Activate.ps1"

Write-Host "ESP-IDF environment loaded (v$IdfVersion)" -ForegroundColor Green
