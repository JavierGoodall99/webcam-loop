#requires -RunAsAdministrator
<#
THROWAWAY (Gate 0). Registers the spike's media source DLL so the Windows Frame Server can load it.
This is the part of the sample's MSI that matters: copy the DLL somewhere any service can read it, and write the COM keys
  HKLM\SOFTWARE\Classes\CLSID\{CLSID}\InProcServer32  (default = DLL path, ThreadingModel = Both)

It changes: C:\Program Files\Gate0VCam\ and one HKLM CLSID key. gate0-unregister.ps1 removes both.

  .\gate0-register.ps1 -Variant pattern               # built-in test pattern, no producer needed
  .\gate0-register.ps1 -Variant producer              # reads frames from Global\gate0-vcam-frames (producer creates it)
  .\gate0-register.ps1 -Variant producer-dllcreates   # the DLL creates the section, the producer opens it
#>
param(
    [ValidateSet('pattern', 'producer', 'producer-dllcreates')]
    [string]$Variant = 'pattern'
)
$ErrorActionPreference = 'Stop'
$clsid = '{7B89B92E-FE71-42D0-8A41-E137D06EA184}'
$src = Join-Path $PSScriptRoot "..\build\$Variant\VirtualCameraMediaSource.dll"
if (-not (Test-Path $src)) { throw "Missing $src - build that variant first." }

$dest = Join-Path $env:ProgramFiles 'Gate0VCam'
New-Item -ItemType Directory $dest -Force | Out-Null
Copy-Item $src (Join-Path $dest 'VirtualCameraMediaSource.dll') -Force

$key = "HKLM:\SOFTWARE\Classes\CLSID\$clsid"
New-Item "$key\InProcServer32" -Force | Out-Null
Set-ItemProperty $key -Name '(default)' -Value 'Gate0 VCam Media Source'
Set-ItemProperty "$key\InProcServer32" -Name '(default)' -Value (Join-Path $dest 'VirtualCameraMediaSource.dll')
Set-ItemProperty "$key\InProcServer32" -Name 'ThreadingModel' -Value 'Both'

"Registered variant '$Variant':"
Get-ItemProperty "$key\InProcServer32" | Select-Object '(default)', ThreadingModel | Format-List
(Get-Item (Join-Path $dest 'VirtualCameraMediaSource.dll')).Length | ForEach-Object { "DLL size: $_ bytes" }
