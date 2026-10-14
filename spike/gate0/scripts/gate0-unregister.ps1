#requires -RunAsAdministrator
<#
THROWAWAY (Gate 0). Removes everything gate0-register.ps1 added:
  - the running spike processes (Registrar, gate0-producer); a Session camera disappears with its creator,
  - the HKLM CLSID key,
  - C:\Program Files\Gate0VCam\ (the DLL copies) and C:\ProgramData\Gate0VCam\ (the flag files).

The camera services (FrameServerMonitor, FrameServer) are stopped so the DLL is released, and are ALWAYS started again,
even if a step fails: other camera apps depend on them. Stopping them briefly interrupts every camera app.

A System-lifetime camera (not used by the final tests) would persist until removed; if one was left behind, run
Registrar.exe --lifetime system once and then Registrar.exe --stop, which removes it.
#>
$ErrorActionPreference = 'Continue'
$clsid = '{7B89B92E-FE71-42D0-8A41-E137D06EA184}'
$installDir = Join-Path $env:ProgramFiles 'Gate0VCam'
$flagDir = Join-Path $env:ProgramData 'Gate0VCam'

Stop-Process -Name Registrar, gate0-producer -Force -ErrorAction SilentlyContinue

try {
    Stop-Service FrameServer -Force -ErrorAction SilentlyContinue
    Stop-Service FrameServerMonitor -Force -ErrorAction SilentlyContinue
    Remove-Item "HKLM:\SOFTWARE\Classes\CLSID\$clsid" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $installDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $flagDir -Recurse -Force -ErrorAction SilentlyContinue
}
finally {
    Start-Service FrameServerMonitor -ErrorAction SilentlyContinue
    Start-Service FrameServer -ErrorAction SilentlyContinue
}

"CLSID key present:      " + (Test-Path "HKLM:\SOFTWARE\Classes\CLSID\$clsid")
"Install dir present:    " + (Test-Path $installDir)
if (Test-Path $installDir) { "  leftover (a loaded DLL can stay locked until the next restart): " + ((Get-ChildItem $installDir -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name) -join ', ') }
"Flag dir present:       " + (Test-Path $flagDir)
Get-Service FrameServer, FrameServerMonitor | Select-Object Name, Status | Format-Table -AutoSize | Out-String
