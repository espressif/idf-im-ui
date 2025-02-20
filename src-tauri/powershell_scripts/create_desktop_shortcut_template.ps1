$iconPath = "$env:USERPROFILE\Icons\eim.ico"
$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("$env:USERPROFILE\Desktop\IDF_{{name}}_Powershell.lnk")
$Shortcut.TargetPath = "powershell.exe"
$Shortcut.Arguments = "-NoExit -ExecutionPolicy Bypass -NoProfile -Command `"& {. '{{custom_profile_filename}}'}`""
$Shortcut.WorkingDirectory = "$env:USERPROFILE\Desktop"
$Shortcut.IconLocation = $iconPath
$Shortcut.Save()

Write-Host "Shortcut created on the desktop: IDF_Powershell.lnk" -ForegroundColor Green