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
Measured on Windows 11 build 26300, non-elevated user account (member of Administrators, UAC on).

| Step | Elevation needed? | Result |
|---|---|---|
| Copy DLL to `C:\Program Files\Gate0VCam\` and write `HKLM\SOFTWARE\Classes\CLSID\{7B89B92E-...}\InProcServer32` (the MSI's job) | **Yes** (HKLM write) | Done once via a UAC prompt (`scripts/gate0-register.ps1`). The Frame Server can find the media source only through this HKLM key. |
| `MFCreateVirtualCamera` + `Start`, lifetime **Session**, access **CurrentUser** (`registrar/Registrar.cpp`) | **No** | Works. Camera exists while the creating process runs; `--stop` removes it cleanly and it disappears from capture enumeration. |
| Sample installer with lifetime **System**, access CurrentUser | Fails non-elevated: `E_ACCESSDENIED 0x80070005`, "Please run application in elevated mode" | The failing call is the sample's own `AddProperty(DEVPKEY_DeviceInterface_VCamCreate_SourceId...)` bookkeeping, after `MFCreateVirtualCamera` already succeeded. The sample also logs "not running in app package", which suggests System-lifetime cameras are meant to be created by a packaged app or an elevated process. Not yet tested whether an MSIX-packaged app can create a System-lifetime camera (Task 9). |
| Replacing the installed DLL while it has been loaded by the Frame Server | **Yes**, and it needs a `FrameServer` service restart | The DLL stays locked after the camera is removed. The register script retries after `Stop-Service FrameServer` (briefly interrupts all camera apps). Matters for upgrades (Task 8). |

Implication for the product: the camera DLL registration is an installer-time, elevated step. Creating the camera itself can be done by the normal-user app (Session lifetime) while the app runs.
## 4. Formats and resolutions offered
Source offers (after our change to the sample): NV12 and RGB32 at 1280x720, 640x360, 1920x1080 and 640x480, all 30 fps.

Observed through OpenCV/Media Foundation (index 1, `probe`), requested size -> delivered size -> strip block size (8 px = native render at 1280x720; other values mean the picture was scaled):

| Requested | Delivered | Strip block | Reading |
|---|---|---|---|
| 1280x720 | 1280x720 | 8 px | native |
| 1920x1080 | 1920x1080 | 12 px | scaled 1.5x from the 720p render (not a native 1080p stream) |
| 640x360 | 640x360 | 4 px | scaled 0.5x from the 720p render |
| 320x240 | 640x360 | 4 px | falls back to 640x360, scaled |
| 640x480, 1024x576 | 640x480 | strip not found | 4:3 output; strip is cropped or letterboxed, picture otherwise fine |

Finding: the Frame Server renders one native size and scales it for clients that ask for another. The product should offer the sizes it can actually render and not rely on per-size negotiation. Remaining per-app choices are recorded in the matrix (Task 7).
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
