#requires -RunAsAdministrator
<#
THROWAWAY (Gate 0). Registers the spike's media source DLL so the Windows Frame Server can load it.
This is the part of the sample's MSI that matters: copy the DLL somewhere any service can read it, and write the COM keys
  HKLM\SOFTWARE\Classes\CLSID\{CLSID}\InProcServer32  (default = DLL path, ThreadingModel = Both)

It changes: C:\Program Files\Gate0VCam\, C:\ProgramData\Gate0VCam\ (run-time flag files) and one HKLM CLSID key.
gate0-unregister.ps1 removes all of it.

Each install uses a NEW file name (the Frame Server keeps a loaded DLL locked, so overwriting in place fails) and then
restarts FrameServerMonitor and FrameServer so the new file is the one that gets loaded. That briefly interrupts every
camera app; the services are always started again, even if something fails.

  .\gate0-register.ps1 -Variant pattern               # built-in test pattern, no producer needed
  .\gate0-register.ps1 -Variant producer              # reads frames from Global\gate0-vcam-frames (see dllcreates.flag)
#>
param(
    [ValidateSet('pattern', 'producer')]
    [string]$Variant = 'pattern'
)
$ErrorActionPreference = 'Stop'
$clsid = '{7B89B92E-FE71-42D0-8A41-E137D06EA184}'
$src = Join-Path $PSScriptRoot "..\build\$Variant\VirtualCameraMediaSource.dll"
if (-not (Test-Path $src)) { throw "Missing $src - build that variant first." }

$dest = Join-Path $env:ProgramFiles 'Gate0VCam'
New-Item -ItemType Directory $dest -Force | Out-Null
$target = Join-Path $dest ("VirtualCameraMediaSource-{0}-{1}.dll" -f $Variant, (Get-Date -Format 'yyyyMMddHHmmss'))
Copy-Item $src $target -Force

# Run-time flag directory for the experiments: a normal user may create/delete dllcreates.flag here.
$flagDir = Join-Path $env:ProgramData 'Gate0VCam'
New-Item -ItemType Directory $flagDir -Force | Out-Null
icacls $flagDir /grant '*S-1-5-32-545:(OI)(CI)M' | Out-Null     # BUILTIN\Users: modify

$key = "HKLM:\SOFTWARE\Classes\CLSID\$clsid"
New-Item "$key\InProcServer32" -Force | Out-Null
Set-ItemProperty $key -Name '(default)' -Value 'Gate0 VCam Media Source'
Set-ItemProperty "$key\InProcServer32" -Name '(default)' -Value $target
Set-ItemProperty "$key\InProcServer32" -Name 'ThreadingModel' -Value 'Both'

try {
    Stop-Service FrameServer -Force -ErrorAction SilentlyContinue
    Stop-Service FrameServerMonitor -Force -ErrorAction SilentlyContinue
}
finally {
    # Always bring the services back: other camera apps depend on them.
    Start-Service FrameServerMonitor -ErrorAction SilentlyContinue
    Start-Service FrameServer -ErrorAction SilentlyContinue
}

# Best-effort removal of older copies (a copy still loaded somewhere simply stays until the next install).
Get-ChildItem $dest -Filter 'VirtualCameraMediaSource*.dll' | Where-Object { $_.FullName -ne $target } |
    Remove-Item -Force -ErrorAction SilentlyContinue

"Registered variant '$Variant':"
Get-ItemProperty "$key\InProcServer32" | Select-Object '(default)', ThreadingModel | Format-List
"DLL size: $((Get-Item $target).Length) bytes"
Get-Service FrameServer, FrameServerMonitor | Select-Object Name, Status | Format-Table -AutoSize | Out-String
