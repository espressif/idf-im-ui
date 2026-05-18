$NewDir = '{{new_dir}}'
$oldPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($null -eq $oldPath) { $oldPath = '' }
$parts = $oldPath.Split(';') | Where-Object { $_ -ne '' }
$exists = $false
foreach ($p in $parts) {
    if ($p.TrimEnd('\').Equals($NewDir.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
        $exists = $true
        break
    }
}
if (-not $exists) {
    if ($oldPath -eq '') {
        $newPath = $NewDir
    } else {
        $newPath = $NewDir + ';' + $oldPath
    }
    [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')

    # Broadcast WM_SETTINGCHANGE so Explorer and other processes pick up the new PATH
    if (-not ([System.Management.Automation.PSTypeName]'Win32.NativeMethods').Type) {
        Add-Type -Namespace Win32 -Name NativeMethods -MemberDefinition @'
            [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
            public static extern IntPtr SendMessageTimeout(
                IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
                uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
    }
    $HWND_BROADCAST = [IntPtr]0xffff
    $WM_SETTINGCHANGE = 0x1a
    $result = [UIntPtr]::Zero
    [Win32.NativeMethods]::SendMessageTimeout(
        $HWND_BROADCAST, $WM_SETTINGCHANGE, [UIntPtr]::Zero, 'Environment',
        2, 5000, [ref]$result) | Out-Null

    Write-Host "Added: $NewDir"
} else {
    Write-Host "Already present: $NewDir"
}
