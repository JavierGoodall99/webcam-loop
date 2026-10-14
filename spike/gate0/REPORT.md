# Gate 0 report

Decision: NOT YET DECIDED

## 1. Environment
| Item | Value |
|---|---|
| Windows version/build | Windows 11 Pro 10.0.26300 (build 26300) |
| CPU / GPU | Intel Core Ultra 9 185H / Intel Arc Graphics |
| Physical camera | Intel(R) AVStream Camera (built-in) |
| VS Build Tools / SDK | Build Tools 2022, cl 19.44.35220, MSBuild 17.14.23; Windows SDK 10.0.26100.0 |
| Python | 3.13.2 |
| Zoom | NOT INSTALLED yet (needed for Task 7) |
| Teams | MSTeams 26260.1701.5139.3736 (new Teams) |
| Chrome / Edge | 155.0.8059.39 / 155.0.4283.45 |
| Discord | 1.0.9237 |

## 2. Baseline (unmodified Microsoft sample)
Source: microsoft/Windows-Camera `Samples/VirtualCamera`, commit `626f8b19c5f367602f2e89c6b314573d3776c9df`, MIT (LICENSE retained as `LICENSE-microsoft-windows-camera`).

What it contains: `VirtualCameraMediaSource` (the COM media source DLL; three modes: Simple/synthetic, HW passthrough, Augmented), `VirtualCamera_Installer` (console app that creates and removes virtual cameras via `MFCreateVirtualCamera`), `VirtualCamera_MSI` (VS Installer Projects MSI that writes the HKLM COM registration), a UWP manager app plus WinRT wrapper and tray app, and `VirtualCameraTest` (GoogleTest).

Findings that differ from the plan's assumptions:
- Registration is two parts: (1) the COM class must be registered under `HKLM\Software\Classes\CLSID\{CLSID}\InProcServer32` (value = DLL path, `ThreadingModel=Both`) so the Frame Server service can load it (the MSI does this; the MSI project cannot be built without the VS Installer Projects extension, so the spike writes the same keys by hand); (2) `VirtualCamera_Installer.exe` (interactive console) calls `MFCreateVirtualCamera`. Camera lifetime (System or Session) and access (CurrentUser or AllUsers) are chosen at creation; the README says AllUsers needs admin.
- The Simple (synthetic) source offers only 640x480 NV12 and RGB32 at 30 fps (`NUM_IMAGE_COLS`/`NUM_IMAGE_ROWS` in `SimpleMediaStream.cpp`). Task 5 must add 1280x720 and other sizes.
- Frames are produced by `SimpleFrameGenerator::_CreateRGB32Frame` (grey gradient scrolling once per second); NV12 is produced by converting that RGB32 frame with the sample's own `RGB32ToNV12Frame`. Task 5 hooks in there and reuses that converter.
- The README says a custom media source runs in the `FrameServer` service once an app uses it (and in `FrameServerMonitor` during creation).
- Build recipe that works: NuGet packages are restored per project (`msbuild <proj> /t:Restore /p:RestorePackagesConfig=true`) and then placed in `vcam-sample\packages` with directory junctions from each project's `packages` folder (the project mixes `..\packages` and `packages` paths). The solution file cannot be restored because it references `Samples/Shared/CameraKsPropertyHelper`, which is outside the sparse checkout and not needed (UWP app only).
- Built successfully, Release x64: `VirtualCameraMediaSource\x64\Release\VirtualCameraMediaSource.dll` and `VirtualCamera_Installer\x64\Release\VirtualCamera_Installer.exe` (MSVC 19.44, SDK 10.0.26100.0, platform toolset v143).
## 3. Registration and elevation
## 4. Formats and resolutions offered
## 5. Shared-memory / Frame Server findings
## 6. Compatibility matrix
| App | Discovered | Restart call app | Restart Windows | 30-min soak | 720p30 | Producer restart | Producer kill → slate → recover | Notes |
|---|---|---|---|---|---|---|---|---|
| Zoom | | | | | | | | |
| Teams | | | | | | | | |
| Meet (Chrome) | | | | | | | | |
| Meet (Edge) | | | | | | | | |
| Discord | | | | | | | | |
## 7. Probe results (per soak)
## 8. MSIX packaging result
## 9. Cleanup / uninstall procedure
## 10. Failures and reasons
## 11. Decision and next step
