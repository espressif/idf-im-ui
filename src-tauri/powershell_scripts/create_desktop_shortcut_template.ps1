$iconPath = "$env:USERPROFILE\Icons\eim.ico"
$desktop = [Environment]::GetFolderPath("Desktop")
$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("$desktop\IDF_{{name}}_Powershell.lnk")
$Shortcut.TargetPath = "powershell.exe"
$Shortcut.Arguments = "-NoExit -ExecutionPolicy Bypass -NoProfile -Command `"& {. '{{custom_profile_filename}}'}`""
$Shortcut.WorkingDirectory = $desktop
$Shortcut.IconLocation = $iconPath
$Shortcut.Save()

Write-Host "Shortcut created on the desktop: IDF_{{name}}_Powershell.lnk" -ForegroundColor Green
