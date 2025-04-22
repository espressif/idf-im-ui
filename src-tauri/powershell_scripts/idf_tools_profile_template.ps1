param(
    [Parameter(Mandatory=$false)]
    [switch]$e
)

{{env_var_pairs}}


# Function to print environment variables
function Print-EnvVariables {
    "PATH={{add_paths_extras}}"
    "ESP_IDF_VERSION={{idf_version}}"
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
$env:ESP_IDF_VERSION = "{{idf_version}}"
$env_var_pairs.GetEnumerator() | ForEach-Object {
    Set-Item -Path "env:$($_.Key)" -Value $_.Value
}

# Set system path
$env:PATH += ";{{add_paths_extras}}"

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
