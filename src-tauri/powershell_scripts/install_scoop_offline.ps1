# Issue Tracker: https://github.com/ScoopInstaller/Install/issues
# Unlicense License:
#
# This is free and unencumbered software released into the public domain.
#
# Anyone is free to copy, modify, publish, use, compile, sell, or
# distribute this software, either in source code form or as a compiled
# binary, for any purpose, commercial or non-commercial, and by any
# means.
#
# In jurisdictions that recognize copyright laws, the author or authors
# of this software dedicate any and all copyright interest in the
# software to the public domain. We make this dedication for the benefit
# of the public at large and to the detriment of our heirs and
# successors. We intend this dedication to be an overt act of
# relinquishment in perpetuity of all present and future rights to this
# software under copyright law.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
# EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
# MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
# IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
# OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
# ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
# OTHER DEALINGS IN THE SOFTWARE.
#
# For more information, please refer to <http://unlicense.org/>

<#
.SYNOPSIS
    Offline Scoop installer.
.DESCRIPTION
    The offline installer of Scoop. Requires pre-downloaded files in the specified offline directory.
.PARAMETER ScoopDir
    Specifies Scoop root path.
    If not specified, Scoop will be installed to '$env:USERPROFILE\scoop'.
.PARAMETER ScoopGlobalDir
    Specifies directory to store global apps.
    If not specified, global apps will be installed to '$env:ProgramData\scoop'.
.PARAMETER ScoopCacheDir
    Specifies cache directory.
    If not specified, caches will be downloaded to '$ScoopDir\cache'.
.PARAMETER OfflineDir
    Specifies the directory containing pre-downloaded Scoop files.
    If not specified, looks for files in the same directory as this script.
.PARAMETER RunAsAdmin
    Force to run the installer as administrator.
.LINK
    https://scoop.sh
.LINK
    https://github.com/ScoopInstaller/Scoop/wiki
#>
param(
    [String] $ScoopDir,
    [String] $ScoopGlobalDir,
    [String] $ScoopCacheDir,
    [String] $OfflineDir,
    [Switch] $RunAsAdmin
)

# Disable StrictMode in this script
Set-StrictMode -Off

function Write-InstallInfo {
    param(
        [Parameter(Mandatory = $True, Position = 0)]
        [String] $String,
        [Parameter(Mandatory = $False, Position = 1)]
        [System.ConsoleColor] $ForegroundColor = $host.UI.RawUI.ForegroundColor
    )

    $backup = $host.UI.RawUI.ForegroundColor

    if ($ForegroundColor -ne $host.UI.RawUI.ForegroundColor) {
        $host.UI.RawUI.ForegroundColor = $ForegroundColor
    }

    Write-Output "$String"

    $host.UI.RawUI.ForegroundColor = $backup
}

function Deny-Install {
    param(
        [String] $message,
        [Int] $errorCode = 1
    )

    Write-InstallInfo -String $message -ForegroundColor DarkRed
    Write-InstallInfo 'Abort.'

    # Don't abort if invoked with iex that would close the PS session
    if ($IS_EXECUTED_FROM_IEX) {
        break
    } else {
        exit $errorCode
    }
}

function Test-LanguageMode {
    if ($ExecutionContext.SessionState.LanguageMode -ne 'FullLanguage') {
        Write-Output 'Scoop requires PowerShell FullLanguage mode to run, current PowerShell environment is restricted.'
        Write-Output 'Abort.'

        if ($IS_EXECUTED_FROM_IEX) {
            break
        } else {
            exit $errorCode
        }
    }
}

function Test-OfflineFiles {
    param(
        [String] $OfflineDirectory
    )

    Write-InstallInfo "Checking offline files in: $OfflineDirectory"

    $requiredFiles = @(
        'scoop-master.zip',
        'main-master.zip'
    )

    $missingFiles = @()
    foreach ($file in $requiredFiles) {
        $filePath = Join-Path $OfflineDirectory $file
        if (!(Test-Path $filePath)) {
            $missingFiles += $file
        } else {
            Write-Verbose "Found: $file"
        }
    }

    if ($missingFiles.Count -gt 0) {
        Deny-Install "Missing required offline files: $($missingFiles -join ', '). Please ensure all required files are in the offline directory: $OfflineDirectory"
    }

    Write-InstallInfo "All required offline files found."
}

function Test-IsAdministrator {
    return ([Security.Principal.WindowsPrincipal]`
            [Security.Principal.WindowsIdentity]::GetCurrent()`
    ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Test-Prerequisite {
    # Scoop requires PowerShell 5 at least
    if (($PSVersionTable.PSVersion.Major) -lt 5) {
        Deny-Install 'PowerShell 5 or later is required to run Scoop. Go to https://microsoft.com/powershell to get the latest version of PowerShell.'
    }

    # Scoop requires TLS 1.2 SecurityProtocol, which exists in .NET Framework 4.5+
    if ([System.Enum]::GetNames([System.Net.SecurityProtocolType]) -notcontains 'Tls12') {
        Deny-Install 'Scoop requires .NET Framework 4.5+ to work. Go to https://microsoft.com/net/download to get the latest version of .NET Framework.'
    }

    # Ensure Robocopy.exe is accessible
    if (!(Test-CommandAvailable('robocopy'))) {
        Deny-Install "Scoop requires 'C:\Windows\System32\Robocopy.exe' to work. Please make sure 'C:\Windows\System32' is in your PATH."
    }

    # Test if scoop is installed, by checking if scoop command exists.
    if (Test-CommandAvailable('scoop')) {
        Deny-Install "Scoop is already installed. Run 'scoop update' to get the latest version."
    }
}

function Test-isFileLocked {
    param(
        [String] $path
    )

    $file = New-Object System.IO.FileInfo $path

    if (!(Test-Path $path)) {
        return $false
    }

    try {
        $stream = $file.Open(
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::ReadWrite,
            [System.IO.FileShare]::None
        )
        if ($stream) {
            $stream.Close()
        }
        return $false
    } catch {
        # The file is locked by a process.
        return $true
    }
}

function Expand-ZipArchive {
    param(
        [String] $path,
        [String] $to
    )

    if (!(Test-Path $path)) {
        Deny-Install "Unzip failed: can't find $path to unzip."
    }

    # Check if the zip file is locked, by antivirus software for example
    $retries = 0
    while ($retries -le 10) {
        if ($retries -eq 10) {
            Deny-Install "Unzip failed: can't unzip because a process is locking the file."
        }
        if (Test-isFileLocked $path) {
            Write-InstallInfo "Waiting for $path to be unlocked by another process... ($retries/10)"
            $retries++
            Start-Sleep -Seconds 2
        } else {
            break
        }
    }

    # Workaround to suspend Expand-Archive verbose output,
    # upstream issue: https://github.com/PowerShell/Microsoft.PowerShell.Archive/issues/98
    $oldVerbosePreference = $VerbosePreference
    $global:VerbosePreference = 'SilentlyContinue'

    # Disable progress bar to gain performance
    $oldProgressPreference = $ProgressPreference
    $global:ProgressPreference = 'SilentlyContinue'

    # PowerShell 5+: use Expand-Archive to extract zip files
    Microsoft.PowerShell.Archive\Expand-Archive -Path $path -DestinationPath $to -Force
    $global:VerbosePreference = $oldVerbosePreference
    $global:ProgressPreference = $oldProgressPreference
}

function Out-UTF8File {
    param(
        [Parameter(Mandatory = $True, Position = 0)]
        [Alias('Path')]
        [String] $FilePath,
        [Switch] $Append,
        [Switch] $NoNewLine,
        [Parameter(ValueFromPipeline = $True)]
        [PSObject] $InputObject
    )
    process {
        if ($Append) {
            [System.IO.File]::AppendAllText($FilePath, $InputObject)
        } else {
            if (!$NoNewLine) {
                # Ref: https://stackoverflow.com/questions/5596982
                # Performance Note: `WriteAllLines` throttles memory usage while
                # `WriteAllText` needs to keep the complete string in memory.
                [System.IO.File]::WriteAllLines($FilePath, $InputObject)
            } else {
                # However `WriteAllText` does not add ending newline.
                [System.IO.File]::WriteAllText($FilePath, $InputObject)
            }
        }
    }
}

function Import-ScoopShim {
    Write-InstallInfo 'Creating shim...'
    # The scoop executable
    $path = "$SCOOP_APP_DIR\bin\scoop.ps1"

    if (!(Test-Path $SCOOP_SHIMS_DIR)) {
        New-Item -Type Directory $SCOOP_SHIMS_DIR | Out-Null
    }

    # The scoop shim
    $shim = "$SCOOP_SHIMS_DIR\scoop"

    # Convert to relative path
    Push-Location $SCOOP_SHIMS_DIR
    $relativePath = Resolve-Path -Relative $path
    Pop-Location
    $absolutePath = Resolve-Path $path

    # if $path points to another drive resolve-path prepends .\ which could break shims
    $ps1text = if ($relativePath -match '^(\.\\)?\w:.*$') {
        @(
            "# $absolutePath",
            "`$path = `"$path`"",
            "if (`$MyInvocation.ExpectingInput) { `$input | & `$path $arg @args } else { & `$path $arg @args }",
            "exit `$LASTEXITCODE"
        )
    } else {
        @(
            "# $absolutePath",
            "`$path = Join-Path `$PSScriptRoot `"$relativePath`"",
            "if (`$MyInvocation.ExpectingInput) { `$input | & `$path $arg @args } else { & `$path $arg @args }",
            "exit `$LASTEXITCODE"
        )
    }
    $ps1text -join "`r`n" | Out-UTF8File "$shim.ps1"

    # make ps1 accessible from cmd.exe
    @(
        "@rem $absolutePath",
        '@echo off',
        'setlocal enabledelayedexpansion',
        'set args=%*',
        ':: replace problem characters in arguments',
        "set args=%args:`"='%",
        "set args=%args:(=``(%",
        "set args=%args:)=``)%",
        "set invalid=`"='",
        'if !args! == !invalid! ( set args= )',
        'where /q pwsh.exe',
        'if %errorlevel% equ 0 (',
        "    pwsh -noprofile -ex unrestricted -file `"$absolutePath`" $arg %args%",
        ') else (',
        "    powershell -noprofile -ex unrestricted -file `"$absolutePath`" $arg %args%",
        ')'
    ) -join "`r`n" | Out-UTF8File "$shim.cmd"

    @(
        '#!/bin/sh',
        "# $absolutePath",
        'if command -v pwsh.exe > /dev/null 2>&1; then',
        "    pwsh.exe -noprofile -ex unrestricted -file `"$absolutePath`" $arg `"$@`"",
        'else',
        "    powershell.exe -noprofile -ex unrestricted -file `"$absolutePath`" $arg `"$@`"",
        'fi'
    ) -join "`n" | Out-UTF8File $shim -NoNewLine
}

function Get-Env {
    param(
        [String] $name,
        [Switch] $global
    )

    $RegisterKey = if ($global) {
        Get-Item -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager'
    } else {
        Get-Item -Path 'HKCU:'
    }

    $EnvRegisterKey = $RegisterKey.OpenSubKey('Environment')
    $RegistryValueOption = [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
    $EnvRegisterKey.GetValue($name, $null, $RegistryValueOption)
}

function Publish-Env {
    if (-not ('Win32.NativeMethods' -as [Type])) {
        Add-Type -Namespace Win32 -Name NativeMethods -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(
    IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
    uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
    }

    $HWND_BROADCAST = [IntPtr] 0xffff
    $WM_SETTINGCHANGE = 0x1a
    $result = [UIntPtr]::Zero

    [Win32.Nativemethods]::SendMessageTimeout($HWND_BROADCAST,
        $WM_SETTINGCHANGE,
        [UIntPtr]::Zero,
        'Environment',
        2,
        5000,
        [ref] $result
    ) | Out-Null
}

function Write-Env {
    param(
        [String] $name,
        [String] $val,
        [Switch] $global
    )

    $RegisterKey = if ($global) {
        Get-Item -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager'
    } else {
        Get-Item -Path 'HKCU:'
    }

    $EnvRegisterKey = $RegisterKey.OpenSubKey('Environment', $true)
    if ($val -eq $null) {
        $EnvRegisterKey.DeleteValue($name)
    } else {
        $RegistryValueKind = if ($val.Contains('%')) {
            [Microsoft.Win32.RegistryValueKind]::ExpandString
        } elseif ($EnvRegisterKey.GetValue($name)) {
            $EnvRegisterKey.GetValueKind($name)
        } else {
            [Microsoft.Win32.RegistryValueKind]::String
        }
        $EnvRegisterKey.SetValue($name, $val, $RegistryValueKind)
    }
    Publish-Env
}

function Add-ShimsDirToPath {
    # Get $env:PATH of current user
    $userEnvPath = Get-Env 'PATH'

    if ($userEnvPath -notmatch [Regex]::Escape($SCOOP_SHIMS_DIR)) {
        $h = (Get-PSProvider 'FileSystem').Home
        if (!$h.EndsWith('\')) {
            $h += '\'
        }

        if (!($h -eq '\')) {
            $friendlyPath = "$SCOOP_SHIMS_DIR" -Replace ([Regex]::Escape($h)), '~\'
            Write-InstallInfo "Adding $friendlyPath to your path."
        } else {
            Write-InstallInfo "Adding $SCOOP_SHIMS_DIR to your path."
        }

        # For future sessions
        Write-Env 'PATH' "$SCOOP_SHIMS_DIR;$userEnvPath"
        # For current session
        $env:PATH = "$SCOOP_SHIMS_DIR;$env:PATH"
    }
}

function Use-Config {
    if (!(Test-Path $SCOOP_CONFIG_FILE)) {
        return $null
    }

    try {
        return (Get-Content $SCOOP_CONFIG_FILE -Raw | ConvertFrom-Json -ErrorAction Stop)
    } catch {
        Deny-Install "ERROR loading $SCOOP_CONFIG_FILE`: $($_.Exception.Message)"
    }
}

function Add-Config {
    param (
        [Parameter(Mandatory = $True, Position = 0)]
        [String] $Name,
        [Parameter(Mandatory = $True, Position = 1)]
        [String] $Value
    )

    $scoopConfig = Use-Config

    if ($scoopConfig -is [System.Management.Automation.PSObject]) {
        if ($Value -eq [bool]::TrueString -or $Value -eq [bool]::FalseString) {
            $Value = [System.Convert]::ToBoolean($Value)
        }
        if ($null -eq $scoopConfig.$Name) {
            $scoopConfig | Add-Member -MemberType NoteProperty -Name $Name -Value $Value
        } else {
            $scoopConfig.$Name = $Value
        }
    } else {
        $baseDir = Split-Path -Path $SCOOP_CONFIG_FILE
        if (!(Test-Path $baseDir)) {
            New-Item -Type Directory $baseDir | Out-Null
        }

        $scoopConfig = New-Object PSObject
        $scoopConfig | Add-Member -MemberType NoteProperty -Name $Name -Value $Value
    }

    if ($null -eq $Value) {
        $scoopConfig.PSObject.Properties.Remove($Name)
    }

    ConvertTo-Json $scoopConfig | Set-Content $SCOOP_CONFIG_FILE -Encoding ASCII
    return $scoopConfig
}

function Add-DefaultConfig {
    # If user-level SCOOP env not defined, save to root_path
    if (!(Get-Env 'SCOOP')) {
        if ($SCOOP_DIR -ne "$env:USERPROFILE\scoop") {
            Write-Verbose "Adding config root_path: $SCOOP_DIR"
            Add-Config -Name 'root_path' -Value $SCOOP_DIR | Out-Null
        }
    }

    # Use system SCOOP_GLOBAL, or set system SCOOP_GLOBAL
    # with $env:SCOOP_GLOBAL if RunAsAdmin, otherwise save to global_path
    if (!(Get-Env 'SCOOP_GLOBAL' -global)) {
        if ((Test-IsAdministrator) -and $env:SCOOP_GLOBAL) {
            Write-Verbose "Setting System Environment Variable SCOOP_GLOBAL: $env:SCOOP_GLOBAL"
            [Environment]::SetEnvironmentVariable('SCOOP_GLOBAL', $env:SCOOP_GLOBAL, 'Machine')
        } else {
            if ($SCOOP_GLOBAL_DIR -ne "$env:ProgramData\scoop") {
                Write-Verbose "Adding config global_path: $SCOOP_GLOBAL_DIR"
                Add-Config -Name 'global_path' -Value $SCOOP_GLOBAL_DIR | Out-Null
            }
        }
    }

    # Use system SCOOP_CACHE, or set system SCOOP_CACHE
    # with $env:SCOOP_CACHE if RunAsAdmin, otherwise save to cache_path
    if (!(Get-Env 'SCOOP_CACHE' -global)) {
        if ((Test-IsAdministrator) -and $env:SCOOP_CACHE) {
            Write-Verbose "Setting System Environment Variable SCOOP_CACHE: $env:SCOOP_CACHE"
            [Environment]::SetEnvironmentVariable('SCOOP_CACHE', $env:SCOOP_CACHE, 'Machine')
        } else {
            if ($SCOOP_CACHE_DIR -ne "$SCOOP_DIR\cache") {
                Write-Verbose "Adding config cache_path: $SCOOP_CACHE_DIR"
                Add-Config -Name 'cache_path' -Value $SCOOP_CACHE_DIR | Out-Null
            }
        }
    }

    # save current datatime to last_update
    Add-Config -Name 'last_update' -Value ([System.DateTime]::Now.ToString('o')) | Out-Null
}

function Test-CommandAvailable {
    param (
        [Parameter(Mandatory = $True, Position = 0)]
        [String] $Command
    )
    return [Boolean](Get-Command $Command -ErrorAction SilentlyContinue)
}

function Install-ScoopOffline {
    param(
        [String] $OfflineDirectory
    )

    Write-InstallInfo 'Initializing offline installation...'

    # Check prerequisites
    Test-Prerequisite

    # Verify offline files exist
    Test-OfflineFiles -OfflineDirectory $OfflineDirectory

    # Install from local files
    Write-InstallInfo 'Installing from offline files...'

    # 1. Extract scoop from local zip
    $scoopZipfile = Join-Path $OfflineDirectory 'scoop-master.zip'
    if (!(Test-Path $SCOOP_APP_DIR)) {
        New-Item -Type Directory $SCOOP_APP_DIR | Out-Null
    }

    Write-Verbose "Extracting $scoopZipfile to $SCOOP_APP_DIR"
    $scoopUnzipTempDir = "$SCOOP_APP_DIR\_tmp"
    Expand-ZipArchive $scoopZipfile $scoopUnzipTempDir
    Copy-Item "$scoopUnzipTempDir\Scoop-*\*" $SCOOP_APP_DIR -Recurse -Force

    # 2. Extract scoop main bucket from local zip
    $scoopMainZipfile = Join-Path $OfflineDirectory 'main-master.zip'
    if (!(Test-Path $SCOOP_MAIN_BUCKET_DIR)) {
        New-Item -Type Directory $SCOOP_MAIN_BUCKET_DIR | Out-Null
    }

    Write-Verbose "Extracting $scoopMainZipfile to $SCOOP_MAIN_BUCKET_DIR"
    $scoopMainUnzipTempDir = "$SCOOP_MAIN_BUCKET_DIR\_tmp"
    Expand-ZipArchive $scoopMainZipfile $scoopMainUnzipTempDir
    Copy-Item "$scoopMainUnzipTempDir\Main-*\*" $SCOOP_MAIN_BUCKET_DIR -Recurse -Force

    # Cleanup temporary directories
    Remove-Item $scoopUnzipTempDir -Recurse -Force
    Remove-Item $scoopMainUnzipTempDir -Recurse -Force

    # Create the scoop shim
    Import-ScoopShim
    # Finally ensure scoop shims is in the PATH
    Add-ShimsDirToPath
    # Setup initial configuration of Scoop
    Add-DefaultConfig

    Write-InstallInfo 'Scoop was installed successfully!' -ForegroundColor DarkGreen
    Write-InstallInfo "Type 'scoop help' for instructions."
}

function Write-DebugInfo {
    param($BoundArgs)

    Write-Verbose '-------- PSBoundParameters --------'
    $BoundArgs.GetEnumerator() | ForEach-Object { Write-Verbose $_ }
    Write-Verbose '-------- Environment Variables --------'
    Write-Verbose "`$env:USERPROFILE: $env:USERPROFILE"
    Write-Verbose "`$env:ProgramData: $env:ProgramData"
    Write-Verbose "`$env:SCOOP: $env:SCOOP"
    Write-Verbose "`$env:SCOOP_CACHE: $SCOOP_CACHE"
    Write-Verbose "`$env:SCOOP_GLOBAL: $env:SCOOP_GLOBAL"
    Write-Verbose '-------- Selected Variables --------'
    Write-Verbose "SCOOP_DIR: $SCOOP_DIR"
    Write-Verbose "SCOOP_CACHE_DIR: $SCOOP_CACHE_DIR"
    Write-Verbose "SCOOP_GLOBAL_DIR: $SCOOP_GLOBAL_DIR"
    Write-Verbose "SCOOP_CONFIG_HOME: $SCOOP_CONFIG_HOME"
    Write-Verbose "OFFLINE_DIR: $OFFLINE_DIR"
}

# Prepare variables
$IS_EXECUTED_FROM_IEX = ($null -eq $MyInvocation.MyCommand.Path)

# Abort when the language mode is restricted
Test-LanguageMode

# Scoop root directory
$SCOOP_DIR = $ScoopDir, $env:SCOOP, "$env:USERPROFILE\scoop" | Where-Object { -not [String]::IsNullOrEmpty($_) } | Select-Object -First 1
# Scoop global apps directory
$SCOOP_GLOBAL_DIR = $ScoopGlobalDir, $env:SCOOP_GLOBAL, "$env:ProgramData\scoop" | Where-Object { -not [String]::IsNullOrEmpty($_) } | Select-Object -First 1
# Scoop cache directory
$SCOOP_CACHE_DIR = $ScoopCacheDir, $env:SCOOP_CACHE, "$SCOOP_DIR\cache" | Where-Object { -not [String]::IsNullOrEmpty($_) } | Select-Object -First 1
# Scoop shims directory
$SCOOP_SHIMS_DIR = "$SCOOP_DIR\shims"
# Scoop itself directory
$SCOOP_APP_DIR = "$SCOOP_DIR\apps\scoop\current"
# Scoop main bucket directory
$SCOOP_MAIN_BUCKET_DIR = "$SCOOP_DIR\buckets\main"
# Scoop config file location
$SCOOP_CONFIG_HOME = $env:XDG_CONFIG_HOME, "$env:USERPROFILE\.config" | Select-Object -First 1
$SCOOP_CONFIG_FILE = "$SCOOP_CONFIG_HOME\scoop\config.json"

# Offline files directory - defaults to script directory if not specified
$OFFLINE_DIR = if ($OfflineDir) {
    $OfflineDir
} else {
    if ($IS_EXECUTED_FROM_IEX) {
        $PWD.Path
    } else {
        Split-Path -Parent $MyInvocation.MyCommand.Path
    }
}

# Quit if anything goes wrong
$oldErrorActionPreference = $ErrorActionPreference
$ErrorActionPreference = 'Stop'

# Logging debug info
Write-DebugInfo $PSBoundParameters
# Bootstrap function for offline installation
Install-ScoopOffline -OfflineDirectory $OFFLINE_DIR

# Reset $ErrorActionPreference to original value
$ErrorActionPreference = $oldErrorActionPreference
