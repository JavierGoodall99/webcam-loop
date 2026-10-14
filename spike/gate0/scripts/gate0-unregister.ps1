#requires -RunAsAdministrator
<#
THROWAWAY (Gate 0). Removes everything gate0-register.ps1 added: virtual cameras created from this media source,
the HKLM CLSID key, and C:\Program Files\Gate0VCam\.
Run the camera removal first (VirtualCamera_Installer.exe /Uninstall removes every camera made from this CLSID).
#>
$ErrorActionPreference = 'Continue'
$clsid = '{7B89B92E-FE71-42D0-8A41-E137D06EA184}'
$installer = Join-Path $PSScriptRoot '..\vcam-sample\VirtualCamera_Installer\x64\Release\VirtualCamera_Installer.exe'
if (Test-Path $installer) { & $installer /Uninstall } else { Write-Warning "Installer not found; cameras (if any) must be removed another way." }

Stop-Service FrameServer -Force -ErrorAction SilentlyContinue   # lets go of the DLL if a call app left it loaded
Remove-Item "HKLM:\SOFTWARE\Classes\CLSID\$clsid" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:ProgramFiles 'Gate0VCam') -Recurse -Force -ErrorAction SilentlyContinue
Start-Service FrameServer -ErrorAction SilentlyContinue

"CLSID key present: " + (Test-Path "HKLM:\SOFTWARE\Classes\CLSID\$clsid")
"Install dir present: " + (Test-Path (Join-Path $env:ProgramFiles 'Gate0VCam'))
Get-PnpDevice -Class Camera -ErrorAction SilentlyContinue | Select-Object Status, FriendlyName | Format-Table -AutoSize
