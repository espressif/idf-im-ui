param(
    [Parameter(Mandatory=$false)]
    [switch]$e
)

# Capture absolute eim path early (before PATH changes)
$_EimBinPath = (Get-Command eim -ErrorAction SilentlyContinue).Source

{{env_var_pairs}}

function Parse-CMakeVersion {

    $cmakeFile = "{{idf_path}}\tools\cmake\version.cmake"

    # Check if file exists
    if (-not (Test-Path $cmakeFile)) {
        Write-Error "CMake version file not found at: $cmakeFile"
        return $null
    }

    $major = $null
    $minor = $null

    try {
        # Read the file content
        $content = Get-Content $cmakeFile -ErrorAction Stop

        foreach ($line in $content) {
            $line = $line.Trim()

            if ($line -match '^set\(IDF_VERSION_MAJOR') {
                # Extract first number from the line
                if ($line -match '\d+') {
                    $major = $matches[0]
                }
            }
            elseif ($line -match '^set\(IDF_VERSION_MINOR') {
                # Extract first number from the line
                if ($line -match '\d+') {
                    $minor = $matches[0]
                }
            }
        }

        # Check if both versions were found
        if ($null -eq $major -or $null -eq $minor) {
            Write-Error "Could not find both major and minor version numbers"
            return $null
        }

        # Return the version
        return "$major.$minor"
    }
    catch {
        Write-Error "Failed to read CMake version file: $($_.Exception.Message)"
        return $null
    }
}

$IdfVersion = Parse-CMakeVersion

# Function to print environment variables
function Print-EnvVariables {
    "PATH={{add_paths_extras}}"
    "ESP_IDF_VERSION=$IdfVersion"
    "SYSTEM_PATH={{current_system_path}}"
    $env_var_pairs.GetEnumerator() | ForEach-Object {
        Write-Host "$($_.Key)=$($_.Value)"
    }
}

# If -e parameter is provided, print variables and exit
if ($e) {
    Print-EnvVariables
    return
}

# Set environment variables
$env:ESP_IDF_VERSION = "$IdfVersion"
$env_var_pairs.GetEnumerator() | ForEach-Object {
    Set-Item -Path "env:$($_.Key)" -Value $_.Value
}

# Set system path
$env:PATH = "{{add_paths_extras}};$env:PATH"

# Define the Invoke-idfpy function
function global:Invoke-idfpy {
    {{idf_python_env_path}}\Scripts\python.exe {{idf_path}}\tools\idf.py @args
}

function global:esptool.py {
  {{idf_python_env_path}}\Scripts\python.exe {{idf_path}}\components\esptool_py\esptool\esptool.py @args
}

function global:espefuse.py {
  {{idf_python_env_path}}\Scripts\python.exe {{idf_path}}\components\esptool_py\esptool\espefuse.py @args
}

function global:espsecure.py {
  {{idf_python_env_path}}\Scripts\python.exe {{idf_path}}\components\esptool_py\esptool\espsecure.py @args
}

function global:otatool.py {
  {{idf_python_env_path}}\Scripts\python.exe {{idf_path}}\components\app_update\otatool.py @args
}

function global:parttool.py {
  {{idf_python_env_path}}\Scripts\python.exe {{idf_path}}\components\partition_table\parttool.py @args
}

# Create an alias for the function
New-Alias -Name idf.py -Value Invoke-idfpy -Force -Scope Global

# Activate your Python environment
. "{{idf_python_env_path}}\Scripts\Activate.ps1"

# Display setup information
Write-Host 'IDF PowerShell Environment'
Write-Host '-------------------------'
Write-Host 'Environment variables set:'
Write-Host "IDF_PATH: $env:IDF_PATH"
Write-Host "IDF_TOOLS_PATH: $env:IDF_TOOLS_PATH"
Write-Host "IDF_PYTHON_ENV_PATH: $env:IDF_PYTHON_ENV_PATH"
Write-Host ''
Write-Host 'Custom commands available:'
Write-Host 'idf.py - Use this to run IDF commands (e.g., idf.py build)'
Write-Host 'esptool.py'
Write-Host 'espefuse.py'
Write-Host 'espsecure.py'
Write-Host 'otatool.py'
Write-Host 'parttool.py'
Write-Host ''
Write-Host 'Python environment activated.'
Write-Host 'You can now use IDF commands and Python tools.'

# Sync selection with eim_idf.json for IDEs (silent on failure)
if ($_EimBinPath) {
    try {
        & $_EimBinPath select "{{idf_version}}" 2>$null
        if ($LASTEXITCODE -eq 0) { Write-Host "eim select {{idf_version}}" }
    } catch {}
}