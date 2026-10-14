# Gate 0 spike — THROWAWAY

Nothing in this folder ships. Its only output is the findings in [REPORT.md](REPORT.md) (decision: **open**). See
`docs/superpowers/plans/2026-10-14-gate0-virtual-camera-spike.md` and spec section 11.

## What is here

| Folder | What it is |
|---|---|
| `vcam-sample/` | Microsoft's MIT `Samples/VirtualCamera` (licence kept), modified: more sizes, test pattern, shared-memory producer path. Our additions are in `vcam-sample/gate0/`. |
| `producer/` | Rust program that writes frames into the shared section (with fault-injection modes). |
| `registrar/` | Small C++ tool that creates the virtual camera (`MFCreateVirtualCamera`) and holds it until `--stop`. |
| `probe/` | Python/OpenCV probe. **Opens the camera exclusively, which locks every other app out. Use only when nothing else is using the camera.** |
| `browser-probe/` | Shared-mode probe page (`getUserMedia`) plus a local result server. Use this one while apps are using the camera. |
| `scripts/` | Elevated install and removal of the DLL registration. |
| `msix/` | Manifest for the throwaway MSIX experiment (the package and certificate were removed). |

## Reproduce

Needs Windows 11, Visual Studio Build Tools 2022 with the Windows 11 SDK, Rust, Python 3.10+, Node 20+.

```powershell
# 1. Build (from a Developer PowerShell). NuGet packages must be restored per project and junctioned; see REPORT.md section 2.
msbuild vcam-sample\VirtualCameraMediaSource\VirtualCameraMediaSource.vcxproj /p:Configuration=Release /p:Platform=x64
# producer variant (frames from the producer):  set CL=/DGATE0_USE_PRODUCER  and use /p:OutDir=build\producer\
cargo build --release --manifest-path producer\Cargo.toml
cl /std:c++17 /EHsc /DUNICODE /D_UNICODE registrar\Registrar.cpp

# 2. Register the DLL (ELEVATED, once; installs under a new file name, restarts the camera services)
.\scripts\gate0-register.ps1 -Variant producer

# 3. Run: camera, producer, and the shared-mode probe page
registrar\Registrar.exe --lifetime session --access user
producer\target\release\gate0-producer.exe --mode open
python browser-probe\server.py            # then open http://127.0.0.1:8765/ in Chrome or Edge

# 4. Remove everything (ELEVATED)
.\scripts\gate0-unregister.ps1
```

Run all automated tests: `./run-tests.sh` (needs the probe venv and the built executables).

## Hard-won rules

- Never test with OpenCV while an app is using the camera, and never run two `Registrar.exe` at once (the second destroys the first's camera).
- Never overwrite the installed DLL in place; use the register script (new file name, repoint, restart services).
- Keep the browser-probe tab's frame source on `MediaStreamTrackProcessor`; `requestVideoFrameCallback` stops when the tab is hidden.
