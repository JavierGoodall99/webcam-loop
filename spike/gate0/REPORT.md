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
Question: can a normal-user app hand frames to a media source that runs inside the Frame Server service (a different account and session)?

**Experiment A: the app (producer) creates `Global\gate0-vcam-frames`, the DLL opens it.** Fails. A non-elevated process cannot create a `Global\` section: `Access is denied (0x80070005)`. (Normal users lack `SeCreateGlobalPrivilege`.)

**Experiment B: the DLL, running in the Frame Server with service privileges, creates the section; the normal-user app opens it.** Works, with no elevation. The DLL creates `Global\gate0-vcam-frames` with a DACL allowing Everyone full access (spike only; see security note), the Rust producer opens it with `OpenFileMappingW` and writes 1280x720 BGRA frames plus a heartbeat. With no producer the camera shows the slate; the output flips to LIVE 0.3 to 0.9 s after the producer attaches. **This is the IPC scheme for the product.**

Security note: the spike's DACL (Everyone: full access) is unacceptable for a product, because any local process could read the camera frames or write fake ones. The product needs a DACL limited to the interactive user and the service account, and the DLL (a service) must learn which user to allow. Open design item for Plan 3.

Reader behavior measured against the real camera (each is also covered by a unit or contract test):

| Case | Result |
|---|---|
| No producer | Slate (flat grey, strip marker SLATE) from the first frame |
| Producer killed | Slate 0.25 s after the last frame (stale window 250 ms). With the first 1000 ms window the output showed a frozen frame as live for up to 0.94 s. |
| Producer restarted | LIVE again within about 0.3 s, no re-selecting of the camera |
| Producer sends wrong size in header | Slate for the whole run; no crash, no garbage |
| Producer sends bad magic | Slate for the whole run |
| Writer dies mid-update (sequence stuck odd, heartbeat moving) | Live for about 2 s, then slate within 0.25 s, and it stays slate |
| Producer alive but no longer writing (hang) | Slate within 0.25 s |
| New producer attaches to a section a dead writer left with an odd sequence | **Bug found and fixed.** The section outlives the producer (the DLL holds it), so the odd value inverted the parity for every later reader and no read succeeded. The producer now makes the sequence even on attach (contract test `test_new_producer_recovers_a_section_left_with_an_odd_sequence`; RED before the fix). |

Design findings:
1. **Torn reads caused visible flicker.** The 3.7 MB frame write is long enough that a reader hits a mid-write state in 10 to 20% of reads. The first reader showed a one-frame grey slate every 0.3 to 1 s while live. Fix: hold the last good frame for up to 250 ms on a torn read (`LatestGoodFrame`, unit-tested), after which the 40 s timeline had no flicker. For the product, use double buffering so tears cannot happen.
2. **Do not block inside `RequestSample`.** Waiting for a fresh producer frame there (to pace the camera to the producer) made delivery fail: 3540 undecodable reads and a 1.0 s gap in 60 s.
3. **Frame repetition and skipping.** The Frame Server requests about 45 samples per second while 30 are delivered, and the producer writes at its own 30 fps clock. On the producer path the counter shows 3 to 15% skipped values and a similar number of repeats (49/1397 and 253/2056 in two runs). Fps, max gap and freezes were all within thresholds, but **this fails the plan's "dropped <= 1%" threshold** and would be visible as judder. Needs a better pacing design in Plan 3 (for example frame timestamps and a non-blocking, timestamp-based pick of the newest frame).
4. **Camera "stops working" episodes: explained (probe artifact).** A Session camera repeatedly stopped starting streams (`MF_E_HW_MFT_FAILED_START_STREAMING 0xC00D3704`) while I was running repeated OpenCV probe sessions. The cause is that the OpenCV probe opens the camera exclusively (see section 6), so overlapping or slowly released probe sessions locked each other out; recreating the camera released them. It is not a fault in the camera. The Frame Server did crash twice (`0xc0000409`, 16:26 and 16:30) while I was replacing the DLL; that is separate and still unexplained.
## 6. Compatibility matrix
Camera: producer variant (frames from the Rust producer through shared memory), non-elevated Session camera, Windows 11 build 26300. Evidence so far is from the screenshots you took during the session (19:34 to 20:0x); the 30-minute soak and the restart checks are still to do.

| App | Discovered | Picture shown | Alongside other clients | Restart call app | Restart Windows | 30-min soak | 720p30 | Producer kill -> slate -> recover | Notes |
|---|---|---|---|---|---|---|---|---|---|
| Zoom | not tested | | | | | | | | Not installed; deferred by the user. Gate cannot pass without it. |
| Teams (new) | yes, "Gate0 VCam (Windows Virtual Camera)" | **yes** (settings preview) | yes: worked while the browser probe and other apps held the camera | | | | requested NV12 1920x1080 first, then 1280x720 | | Showed BLACK while my OpenCV probe held the camera (see below). Self-view is mirrored (strip appears top-right). |
| Meet (Chrome) | yes | **yes** via the browser probe (a shared getUserMedia client) | yes (with Teams) | | | | 1280x720 @ ~29 fps delivered | | Probe: 28.7 to 29.1 fps, max gap 0.08 to 0.10 s, 0 freezes/slate/undecodable. Real Meet call not yet tried. |
| Meet (Edge) | probably (second browser, see notes) | yes if the second screenshot was Edge | yes | | | | | | To confirm which browser the second probe window was. |
| Discord | yes | **yes after re-selecting** the camera | yes | | | | | | Discord (web, discord.com in Chrome) first showed the real webcam although the dropdown said Gate0 VCam; after switching the dropdown away and back it showed our picture. Desktop app not yet tested. |

Multi-client finding (important for the product and for test method): **opening the camera through OpenCV/Media Foundation (`MFCreateDeviceSource`) takes exclusive control and locks out every other app.** While the OpenCV probe ran, Teams showed a black preview and a second OpenCV client got nothing (`0xC00D3704`). With no exclusive client running, Teams, Chrome, a second browser and Discord all show the camera at the same time. The shared-mode clients (Teams, browsers, Discord) coexist. The earlier "camera stops working after N sessions" episodes were most likely overlapping or slowly released exclusive OpenCV sessions, not a camera fault. All numeric measurements for the matrix therefore use the browser probe (`browser-probe/`), which is a shared client. Chrome pauses video callbacks for hidden or minimized tabs, so the probe window must stay visible.

## 7. Probe results (per soak)
All runs: Python probe through OpenCV/Media Foundation (index 1), 1280x720 requested, camera created non-elevated (Session lifetime). Full per-run output is in `probe-runs.md`. Gate thresholds: freeze events 0, max gap <= 0.5 s, dropped <= 1%, undecodable 0, fps >= 27.

| Run | Variant | Result |
|---|---|---|
| 2 min, 1280x720 | built-in pattern | 30.02 fps, max gap 0.10 s, 0 freezes, 0 undecodable. **PASS** (drop check off: the pattern's counter advances ~45/s per sample request) |
| 1 min, 640x360 (pipeline-scaled) | built-in pattern | 29.96 to 30.01 fps, max gap 0.13 to 0.20 s. **PASS** |
| 45 s, producer attached late | producer, first DLL | 30.05 fps, max gap 0.14 s, 0 freezes, 0 undecodable. Dropped 48/1397 (3.4%): **fails the drop threshold**; slate for the first 11 s as designed |
| 60 s, DLL paced to producer (blocking) | producer | **FAIL**: 3540 undecodable reads, max gap 1.0 s. Blocking in `RequestSample` is not viable. |
| 60 s, no pacing | producer | 30.02 fps, max gap 0.16 s, 0 freezes, 0 undecodable. Dropped 256/2056 (12%): **fails the drop threshold** |
| 40 s kill/restart timeline | producer + `LatestGoodFrame`, 250 ms stale window | No flicker; SLATE 0.25 s after the kill; LIVE again within 0.3 s of restart; longest identical-LIVE run 0.27 s |
| **30 min continuous soak (20:12 to 20:42), Chrome browser probe** | producer + `LatestGoodFrame` | 1800.5 s, 53,008 frames, **29.44 fps**, **0 freezes, 0 slate, 0 undecodable**. **Max gap 0.679 s (fails the 0.5 s limit, once)**. Counter gaps 7456 of 60,464 expected (12.3%) with 6446 repeats (**fails the 1% drop limit**). The probe tab was hidden for 1566 s (87% of the run) and delivery stayed at 30 fps. Other apps (Teams, Edge, Discord) held the camera during the run. |
| **10 min, tab visible, Chrome browser probe** | producer + `LatestGoodFrame` | 600.0 s, 17,663 frames, **29.44 fps**, **max gap 0.374 s (passes)**, 0 freezes, 0 slate, 0 undecodable, tab hidden 0.0 s. Three small hiccups: 0.32 s at 240 s, 0.34 s at 510 s, 0.37 s at 514 s. Counter gaps 2909 of 20,572 (14.1%) with 2572 repeats: **fails the 1% drop limit**, the only failing measure. |
| First soak attempt | producer | **Invalid, discarded.** 193 s in, a single 123 s gap and 3981 counter gaps because Chrome had paused the tab's video callbacks while hidden (the probe's old frame source). Fixed: the probe now reads frames with `MediaStreamTrackProcessor` and records hidden time. |

Reading the 30-minute soak: the camera ran for the full 30 minutes without a freeze, black frame, slate frame or undecodable frame, and averaged 29.44 fps while three other apps used it. Two of my own thresholds failed. The 10-minute re-run with the tab visible shows the max gap is 0.374 s (passes), so the 0.68 s gap was most likely Chrome throttling the hidden tab; only the drop rate is a confirmed failure. (1) One 0.68 s gap: with the tab hidden 87% of the time this could be Chrome's background throttling or a real hiccup in the camera path; the page only recorded the maximum at that point, so the cause is not known (the page now lists every gap over 0.25 s with its time, so the next run will say). (2) The 12% counter gaps and repeats match every earlier producer run (12 to 14%), so it is a property of the producer-to-camera sampling, not noise. Neither is a freeze, but together they mean the output is not smooth enough yet for a product (about 4 skipped and 4 repeated frames per second).

Probe calibration: the same read loop gets 29.0 fps from the physical webcam and 29.9 fps from the virtual camera, so the probe is not the limiting factor. Two earlier runs (about 21 and 25 fps) were discarded: one was limited by the probe itself (fixed test-first), one was judged on counter gaps that are meaningless for the built-in pattern.
## 8. MSIX packaging result
Package: `msix/AppxManifest.xml` (a full-trust app wrapping `Registrar.exe`, with an execution alias `gate0registrar.exe`). Built with `makeappx`, signed with a throwaway self-signed certificate (`CN=Gate0Spike`).

What worked:
- The package builds and signs once the manifest is valid. Two fixes were needed: no `--` inside an XML comment, and the `windows.appExecutionAlias` extension belongs to the `uap3` schema (not `uap5`).
- A self-signed package installs **without elevation, but only after its certificate is trusted in the machine's Trusted People store, which needs one elevated step.** (A product would use a real code-signing certificate or the Store.) After testing, the package, alias, machine trust and my personal copy of the certificate were all removed and verified gone.
- A packaged process really ran from `C:\Program Files\WindowsApps\Gate0.VCamSpike_...\Registrar.exe`, so the tests below had package identity.

Camera creation, all non-elevated, **identical whether packaged or not**:

| Lifetime / access | Unpackaged | Packaged |
|---|---|---|
| Session, current user | created | created |
| System, current user | created | created |
| Session, all users | `0x80070005` | `0x80070005` |
| System, all users | `0x80070005` | `0x80070005` |

So **package identity is not needed to create a camera** (the sample's "not running in app package" warning was misleading; the only thing that needed elevation was the sample installer's own extra bookkeeping). All-users cameras need elevation either way. A packaged app's camera streams through the HKLM-registered DLL and is removed cleanly.

Not tested (stated plainly): whether the DLL's COM registration can be declared inside the package and still be found by the Frame Server (E3). I did not run it because the Frame Server runs outside the package, and Microsoft's own sample deploys the DLL with an MSI while a packaged app creates the cameras. Treat Store-only distribution as **unproven**. The model that works today is: an elevated one-time installer step registers the DLL (HKLM), and a normal non-elevated app creates and feeds the camera. That fits the spec's "direct download first, Store later".
## 9. Cleanup / uninstall procedure
Verified so far:
1. Stop the camera cleanly: `registrar\Registrar.exe --stop` (Remove + Shutdown). The camera is gone from capture enumeration immediately (probe finds no virtual camera).
2. Replacing the media source DLL: **do not overwrite in place.** The Frame Server keeps the DLL locked, and overwriting still failed after stopping both `FrameServer` and `FrameServerMonitor` (cause unexplained; an old copy stays locked and cannot be deleted). Working method (`scripts/gate0-register.ps1`): copy the new build under a **new file name**, repoint `HKLM\...\CLSID\{...}\InProcServer32` at it, then restart `FrameServerMonitor` and `FrameServer`. The service restart briefly interrupts every camera app, and the script must always restart them (an earlier version aborted and left the camera services stopped).
3. Full removal: `scripts/gate0-unregister.ps1` (removes cameras via the sample installer, the CLSID key and `C:\Program Files\Gate0VCam`; restarts `FrameServer`). To be run and verified at the end of the spike.

Lifecycle results (all non-elevated, camera named "Gate0 VCam"):
| Case | Result |
|---|---|
| Abrupt kill (`Stop-Process -Force`) of the process that created a **Session** camera | The camera disappears immediately; nothing is left behind. |
| Abrupt kill of the process that created a **System** camera | The camera **persists** and keeps streaming (frames or slate). A new instance of the same tool (same name and source CLSID) can then remove it with `Remove()`, because the camera's identity is derived from them. |
| Second instance registering the same camera while the first is running | The second is refused (`Start failed 0xC00D36B2`) **and the first instance's camera is destroyed even though its process is alive** (confirmed: first alone streams; after the failed second attempt the camera is gone and the first's later `Remove` returns `0xC00D36D5`). The real app must enforce a single instance before touching the camera, and must notice a vanished camera and recreate it. |
| Camera removed while a client is streaming from it | The client's frames stop (about 3.6 s into its run) and it does not hang. |
| Replacing the DLL | See item 2 above (new file name, repoint HKLM, restart the camera services). |

Still to verify: behavior across logoff and a Windows restart (a System camera is documented to last until restart).
## 10. Failures and reasons
Open items at this point:
- **Zoom: untested** (not installed; deferred by the user). The gate cannot pass without it.
- **Drop threshold fails on the producer path** (3 to 15% counter gaps/repeats). Not a delivery failure by the other measures, but a real judder risk. See section 5, finding 3.
- **Frame Server crashes** (`0xc0000409`, twice) while the DLL was being replaced.
- **Elevation:** DLL registration is an elevated, installer-time step; the camera itself can be created without elevation.
- MSIX experiment (section 8) is done (package identity does not matter for camera creation; DLL registration inside the package untested). Call-app matrix (section 6): restart checks, Zoom, Discord desktop and Edge confirmation still open.
## 11. Decision and next step
