# Gate 0: Virtual Camera Compatibility Spike — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decide, with measured evidence, whether a Windows 11 native virtual camera (`MFCreateVirtualCamera`) is accepted and stable in Zoom, Teams, Meet (Chrome and Edge) and Discord, and whether our frame-delivery and installation model works. Output: a pass/fail report and a fallback decision.

**Architecture:** Start from Microsoft's MIT-licensed `Samples/VirtualCamera` media source. Add (1) a test pattern with a machine-readable frame-counter strip, (2) a shared-memory reader with a "slate" fallback, (3) a tiny Rust producer, (4) a Python probe that measures what the camera actually delivers, and (5) an MSIX registration experiment. All of it is throwaway: nothing here is carried into the product, only the findings.

**Tech Stack:** C++ (MSVC, Windows 11 SDK) for the camera DLL spike; Rust 1.x and the `windows` crate for the producer; Python 3 with `numpy`, `opencv-python`, `pytest` for the probe (measurement only, never shipped).

**Spec:** `docs/superpowers/specs/2026-10-14-webcam-loop-design.md` (sections 2, 4.3, 4.4, 8, 9, 9a). Read it first.

This is **Plan 1 of 4**. Plans 2 (looper and compositor), 3 (desktop app and camera DLL for real) and 4 (licensing and web layer) are written only after this gate passes.

## Global Constraints

- Platform: Windows 11 only (`MFCreateVirtualCamera` requires build 22000 or later). Record the exact build tested.
- Test matrix, every app: **Zoom, Microsoft Teams, Google Meet in Chrome, Google Meet in Edge, Discord.**
- Gate passes only if, in every app: (1) camera discovered reliably (first launch, call-app restart, Windows restart); (2) streams continuously for **at least 30 minutes** with stable frames; (3) survives restarting the producer app and recovers cleanly if the producer is killed (slate shown, then normal video when it returns); (4) works at **at least 720p at 30 fps**.
- Stated stability thresholds (the spec says "a stated threshold"; these are it), measured by the probe on the LIVE phase: freeze events = 0; max gap between frames <= 0.5 s; dropped frames <= 1% of expected; undecodable frames = 0; average fps >= 27.
- The report must record: Windows versions and builds tested; installation requirements and any permissions or elevation needed to register and unregister the camera; whether the registration model works with the intended MSIX packaging; every failing app with the reason.
- Decision rule: if Zoom or Teams fail, the DirectShow filter fallback gets its own spike with the same criteria, and the spec is revised before any other work starts.
- Clean-room: do not read `haxybaxy/video-pedal` source (`loop_pedal.py`, `tests/`). Nothing in this plan comes from it.
- Licensing: every dependency and every Microsoft sample used is recorded in `DEPENDENCIES.md` (name, version, license, source). No GPL components. The OBS virtual camera and `pyvirtualcam` are not used.
- Spike code is throwaway. It lives under `spike/gate0/` and is labeled as such.
- No CSVs, signing keys (`*.pfx`), or recordings of real people are committed. Report tables are Markdown.
- Webcam frames never leave the machine. The spike makes no network calls except `git clone` and package installs.

## Review Focus

Failure modes the spec implies but a happy-path run would not exercise, most likely first. Each is pinned to the task that owns it.

1. **Frame Server process boundary.** The camera source runs inside the Windows Frame Server service, in a different session and account from the producer. A shared-memory section created with default permissions, or in the per-session `Local\` namespace, is invisible to it. Expected: camera shows the slate (never crashes, never hangs) and the report states exactly which naming and permission scheme works. (Task 6)
2. **Two consumers at once.** A call app plus the probe both opening the camera; or two call apps. Expected: both get frames, or the failure is recorded. (Task 7)
3. **Format and resolution negotiation.** Apps ask for sizes and pixel formats the source may not offer (for example 640x360 or NV12 vs. RGB32). Expected: every app in the matrix gets a usable picture at its chosen size, and the supported list is recorded. (Tasks 5 and 7)
4. **Producer sends the wrong frame size or garbage.** Header says 1920x1080 while the camera negotiated 1280x720, a truncated mapping, or a half-written frame. Expected: slate or last good frame behavior as specified, never a crash or torn picture. (Task 6, with unit tests)
5. **Leftover registrations.** Crash mid-test, registering twice, or un-registering while an app has the camera open. Expected: no phantom cameras in app pickers after cleanup; the cleanup procedure is written down. (Tasks 3, 8 and 9)

---

## File Structure

```
webcam-loop/
├── .gitignore
├── DEPENDENCIES.md                       # license + provenance log
├── docs/superpowers/{specs,plans}/       # spec (exists), this plan
└── spike/gate0/                          # THROWAWAY
    ├── README.md                         # "throwaway; do not ship"
    ├── REPORT.md                         # the deliverable
    ├── vcam-sample/                      # copy of microsoft/Windows-Camera Samples/VirtualCamera (MIT, LICENSE kept)
    │   └── gate0/                        # our additions
    │       ├── TestPattern.h             # test pattern + counter strip + BGRA->NV12
    │       ├── SharedFrame.h             # shared-memory contract + TryReadFrame
    │       ├── SharedSection.h           # open/create the named section
    │       └── SharedFrameTests.cpp      # console unit tests for TryReadFrame
    ├── producer/                         # Rust: writes frames into shared memory
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── probe/                            # Python: measures what the camera delivers
    │   ├── counter_codec.py
    │   ├── stats.py
    │   ├── vcam_probe.py
    │   ├── test_counter_codec.py
    │   └── test_stats.py
    └── msix/                             # packaging experiment
        └── AppxManifest.xml
```

---

### Task 1: Repo, toolchain and report skeleton

**Files:**
- Create: `D:\Coding Projects\webcam-loop\.gitignore`, `DEPENDENCIES.md`, `spike/gate0/README.md`, `spike/gate0/REPORT.md`

**Interfaces:**
- Produces: a git repo; `REPORT.md` with the section headings later tasks fill in; a verified build environment.

- [ ] **Step 1: Initialise the repo and ignore rules**

```powershell
Set-Location "D:\Coding Projects\webcam-loop"
git init -b main
@'
target/
__pycache__/
.pytest_cache/
.venv/
.vs/
x64/
Debug/
Release/
*.obj
*.pdb
*.ilk
*.exe
*.dll
*.lib
*.exp
*.pfx
*.cer
*.csv
*.mp4
*.mkv
*.msix
'@ | Set-Content .gitignore -Encoding utf8
git add .gitignore docs
git commit -m "chore: init repo with spec and Gate 0 plan"
```

Expected: a commit containing `.gitignore`, the spec and this plan.

- [ ] **Step 2: Verify the C++ toolchain and Windows SDK**

Run in a normal PowerShell:

```powershell
$vs = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools"
& "$vs\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -SkipAutomaticLocation
cl 2>&1 | Select-Object -First 2
msbuild -version
Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\Include" -Directory | Select-Object -ExpandProperty Name
Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makeappx.exe | Select-Object -First 1 -ExpandProperty FullName
```

Expected: `cl` prints a compiler version, `msbuild` prints a version, at least one SDK folder at `10.0.22000.0` or newer is listed, and `makeappx.exe` is found. If anything is missing, install via Visual Studio Installer → Build Tools 2022 → Modify: workload "Desktop development with C++" plus the "Windows 11 SDK (10.0.22621 or newer)". Note: the Windows build of this dev machine is 10.0.26300.

- [ ] **Step 3: Verify Python and install probe dependencies**

```powershell
py -3 --version
py -3 -m venv spike\gate0\probe\.venv
spike\gate0\probe\.venv\Scripts\python -m pip install numpy opencv-python pytest
spike\gate0\probe\.venv\Scripts\python -m pip freeze | Select-String "numpy|opencv|pytest"
```

Expected: Python 3.10 or newer, and the three packages with versions. Copy those versions into `DEPENDENCIES.md` in Step 5. (If `py` is missing, install Python 3.12 from python.org first.)

- [ ] **Step 4: Verify the call apps and get Zoom**

```powershell
foreach ($p in "$env:APPDATA\Zoom\bin\Zoom.exe","$env:LOCALAPPDATA\Discord\Update.exe","$env:ProgramFiles\Google\Chrome\Application\chrome.exe","${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe") { "{0}: {1}" -f $p,(Test-Path $p) }
Get-AppxPackage -Name '*Teams*' | Select-Object Name, Version
```

Expected before this task: Zoom `False`. Install Zoom from zoom.us and sign in (or use a free account). Record the version of each app. Teams must be the new Teams (`MSTeams`).

- [ ] **Step 5: Write `DEPENDENCIES.md`**

```markdown
# Dependency log

Every third-party component, its license and where it came from. Updated whenever a dependency is added.

| Component | Version | License | Source | Used for | Ships in product? |
|---|---|---|---|---|---|
| microsoft/Windows-Camera `Samples/VirtualCamera` | commit SHA recorded in Task 2 | MIT (verify the LICENSE file in Task 2) | https://github.com/microsoft/Windows-Camera | Gate 0 spike base only | No (decision after the spike) |
| windows (windows-rs) | 0.58 | MIT OR Apache-2.0 (verify in `cargo metadata`) | crates.io | Spike producer | Undecided |
| numpy, opencv-python, pytest | versions from Task 1 Step 3 | BSD / Apache-2.0 / MIT (verify with `pip show`) | PyPI | Probe (measurement only) | No |
```

- [ ] **Step 6: Write `spike/gate0/README.md` and `REPORT.md` skeleton**

`README.md`:

```markdown
# Gate 0 spike — THROWAWAY

Nothing in this folder ships. Its only output is the findings in REPORT.md. See docs/superpowers/plans/2026-10-14-gate0-virtual-camera-spike.md.
```

`REPORT.md` (the headings later tasks fill in; each table cell is recorded data):

```markdown
# Gate 0 report

Decision: NOT YET DECIDED

## 1. Environment
| Item | Value |
|---|---|
| Windows version/build | |
| CPU / GPU | |
| Physical camera | |
| VS Build Tools / SDK | |
| Zoom / Teams / Chrome / Edge / Discord versions | |

## 2. Baseline (unmodified Microsoft sample)
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
```

Fill section 1 now from Steps 2–4 and `Get-CimInstance Win32_OperatingSystem`.

- [ ] **Step 7: Commit**

```powershell
git add DEPENDENCIES.md spike/gate0/README.md spike/gate0/REPORT.md
git commit -m "chore(gate0): dependency log and report skeleton"
```

---

### Task 2: Import and build the Microsoft sample, unchanged

**Files:**
- Create: `spike/gate0/vcam-sample/` (copy of the sample, with its LICENSE)
- Modify: `DEPENDENCIES.md`, `spike/gate0/REPORT.md`

**Interfaces:**
- Produces: a buildable baseline; the recorded commit SHA; knowledge of the sample's project/solution names, the registration command, and where it generates frames (used by Tasks 3, 5, 6).

- [ ] **Step 1: Sparse-clone the sample outside the repo and record its commit**

```powershell
$tmp = "$env:TEMP\winclone-src"      # NOT inside any repo
git clone --depth 1 --filter=blob:none --sparse https://github.com/microsoft/Windows-Camera.git $tmp
git -C $tmp sparse-checkout set Samples/VirtualCamera
git -C $tmp rev-parse HEAD
Get-Content "$tmp\LICENSE" -TotalCount 5
```

Expected: a 40-character SHA and a license text starting with "MIT License". Record both. If the license is not MIT, stop and report it: the spike would then need a different base.

- [ ] **Step 2: Copy the sample into the repo, keeping the license**

```powershell
Set-Location "D:\Coding Projects\webcam-loop"
Copy-Item "$tmp\Samples\VirtualCamera" "spike\gate0\vcam-sample" -Recurse
Copy-Item "$tmp\LICENSE" "spike\gate0\vcam-sample\LICENSE-microsoft-windows-camera"
Get-ChildItem spike\gate0\vcam-sample -Recurse -Include *.sln,*.vcxproj,*.csproj,README.md | Select-Object -ExpandProperty FullName
```

Expected: the list of solution/project files and the sample README. Update the `DEPENDENCIES.md` row with the real SHA and "MIT (confirmed)".

- [ ] **Step 3: Read the sample's README and record how it works**

Read `spike\gate0\vcam-sample\README.md` fully. In `REPORT.md` section 2 write: what it builds (DLL, config app names), how it registers the camera (command or app), whether it asks for elevation, what the camera shows, and the stream formats it advertises. Then locate where frames are produced:

```powershell
Select-String -Path "spike\gate0\vcam-sample\*" -Recurse -Pattern 'RequestSample|MFCreateSample|NV12|RGB32|ColorBar|GenerateSample|MF_MT_SUBTYPE' | Select-Object Path, LineNumber, Line | Format-Table -AutoSize
```

Record the file and function that fill the output buffer, and the pixel format. Tasks 5 and 6 plug in there.

- [ ] **Step 4: Build the unchanged sample**

```powershell
& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -SkipAutomaticLocation
$sln = (Get-ChildItem spike\gate0\vcam-sample -Recurse -Filter *.sln | Select-Object -First 1).FullName
msbuild $sln /p:Configuration=Release /p:Platform=x64 /restore /m
```

Expected: `Build succeeded.` If it fails because of a missing SDK version, retarget via the property `/p:WindowsTargetPlatformVersion=<installed SDK>` and record the fix. A build that cannot be made to work after a good-faith attempt is itself a Gate 0 finding: record it and stop.

- [ ] **Step 5: Commit**

```powershell
git add spike/gate0/vcam-sample DEPENDENCIES.md spike/gate0/REPORT.md
git commit -m "spike(gate0): import MIT-licensed Microsoft virtual camera sample unchanged"
```

---

### Task 3: Baseline registration, discovery and cleanup

**Files:**
- Modify: `spike/gate0/REPORT.md` (sections 2, 3, 9)

**Interfaces:**
- Consumes: the built baseline from Task 2.
- Produces: the exact register and unregister commands; whether elevation is needed; a verified cleanup procedure reused by every later task.

- [ ] **Step 1: Register the baseline camera as the sample's README describes**

Run the registration step exactly as documented in the sample, first from a **non-elevated** PowerShell. Record whether it succeeded. If it fails with an access error, rerun elevated and record that elevation is required, and for which step (COM registration, camera creation, or both).

- [ ] **Step 2: Confirm the OS sees the camera**

```powershell
Get-PnpDevice -Class Camera -ErrorAction SilentlyContinue | Select-Object Status, FriendlyName, InstanceId | Format-Table -AutoSize
```

Also open the Windows Camera app and switch cameras. Expected: a new device appears. Record its exact friendly name (the OS may append "Windows Virtual Camera" to the name given).

- [ ] **Step 3: See picture in one app**

Open Chrome → `https://webcamtests.com` or any camera test page, select the new camera, confirm the sample's picture shows. Record what the picture is and its resolution.

- [ ] **Step 4: Unregister and verify clean removal**

Run the sample's unregister path. Then:

```powershell
Get-PnpDevice -Class Camera | Select-Object FriendlyName
Get-ChildItem "HKCU:\Software\Classes\CLSID","HKLM:\SOFTWARE\Classes\CLSID" -ErrorAction SilentlyContinue | Where-Object { (Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue).'(default)' -match 'Virtual' }
```

Expected: the camera is gone from the device list and from the Windows Camera app's picker (reopen the app), and no leftover COM class entries for the sample. Write the cleanup procedure into `REPORT.md` section 9, including anything that needed a reboot or manual deletion. Pin for Review Focus 5: also register twice in a row and confirm you get one camera, or record what happens; then kill the registering process mid-run and check for a phantom camera.

- [ ] **Step 5: Commit**

```powershell
git add spike/gate0/REPORT.md
git commit -m "spike(gate0): record baseline registration, elevation and cleanup"
```

---

### Task 4: The probe (TDD)

**Files:**
- Create: `spike/gate0/probe/counter_codec.py`, `stats.py`, `vcam_probe.py`
- Test: `spike/gate0/probe/test_counter_codec.py`, `test_stats.py`

**Interfaces:**
- Produces:
  - `counter_codec.encode_strip(counter: int, marker: int = MARKER_LIVE) -> np.ndarray` (uint8 grayscale, shape `(BLOCK, BLOCK*BITS)`)
  - `counter_codec.decode_strip(image: np.ndarray) -> tuple[int, int] | None` returning `(marker, counter)`
  - constants `BLOCK = 8`, `BITS = 48`, `MARKER_LIVE = 0xA5C3`, `MARKER_SLATE = 0x5A3C`
  - `stats.summarize(samples, freeze_threshold_s=1.0) -> dict`
  - `stats.gate_failures(summary) -> list[str]`
- The C++ `TestPattern.h` (Task 5) and Rust producer (Task 6) must draw exactly this strip: 48 blocks of 8x8 px at the top-left, MSB first, 16 marker bits then 32 counter bits, white = 1, black = 0.

- [ ] **Step 1: Write the failing codec tests**

`spike/gate0/probe/test_counter_codec.py`:

```python
import numpy as np
import pytest

from counter_codec import (
    BITS, BLOCK, MARKER_LIVE, MARKER_SLATE, decode_strip, encode_strip,
)


def canvas(strip, w=1280, h=720, channels=3):
    img = np.full((h, w, channels), 90, dtype=np.uint8)
    img[: strip.shape[0], : strip.shape[1], :] = strip[:, :, None]
    return img


@pytest.mark.parametrize("counter", [0, 1, 255, 65535, 123456789, 0xFFFFFFFF])
def test_roundtrip_live(counter):
    assert decode_strip(canvas(encode_strip(counter))) == (MARKER_LIVE, counter)


def test_roundtrip_slate_marker():
    assert decode_strip(canvas(encode_strip(7, MARKER_SLATE))) == (MARKER_SLATE, 7)


def test_survives_noise_and_grayscale_input():
    rng = np.random.default_rng(1)
    img = canvas(encode_strip(4242)).astype(np.int16)
    img += rng.integers(-25, 26, img.shape)
    img = np.clip(img, 0, 255).astype(np.uint8)
    assert decode_strip(img) == (MARKER_LIVE, 4242)
    assert decode_strip(img[:, :, 0]) == (MARKER_LIVE, 4242)


def test_too_small_image_returns_none():
    assert decode_strip(np.zeros((BLOCK - 1, BLOCK * BITS, 3), np.uint8)) is None
    assert decode_strip(np.zeros((BLOCK, BLOCK * BITS - 1, 3), np.uint8)) is None


def test_unknown_marker_returns_none():
    rng = np.random.default_rng(2)
    noise = rng.integers(0, 256, (720, 1280, 3), dtype=np.uint8)
    assert decode_strip(noise) is None
```

- [ ] **Step 2: Run to verify failure**

```powershell
Set-Location "D:\Coding Projects\webcam-loop\spike\gate0\probe"
.\.venv\Scripts\python -m pytest test_counter_codec.py -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'counter_codec'`.

- [ ] **Step 3: Implement `counter_codec.py`**

```python
"""Machine-readable frame-counter strip shared by the test pattern, the producer and the probe.

Layout: BITS blocks of BLOCK x BLOCK pixels at the top-left, MSB first.
First 16 bits are a marker (LIVE or SLATE), the next 32 are a frame counter.
White block = 1, black block = 0.
"""
import numpy as np

BLOCK = 8
BITS = 48
MARKER_LIVE = 0xA5C3
MARKER_SLATE = 0x5A3C
_VALID_MARKERS = (MARKER_LIVE, MARKER_SLATE)


def encode_strip(counter: int, marker: int = MARKER_LIVE) -> np.ndarray:
    value = (marker << 32) | (counter & 0xFFFFFFFF)
    strip = np.zeros((BLOCK, BLOCK * BITS), dtype=np.uint8)
    for i in range(BITS):
        if (value >> (BITS - 1 - i)) & 1:
            strip[:, i * BLOCK:(i + 1) * BLOCK] = 255
    return strip


def decode_strip(image: np.ndarray):
    """Return (marker, counter) or None if the image is too small or the marker is unknown."""
    if image.ndim == 3:
        gray = image.mean(axis=2)
    else:
        gray = image.astype(np.float64)
    if gray.shape[0] < BLOCK or gray.shape[1] < BLOCK * BITS:
        return None
    row = BLOCK // 2
    value = 0
    for i in range(BITS):
        bit = 1 if gray[row, i * BLOCK + BLOCK // 2] >= 128 else 0
        value = (value << 1) | bit
    marker = value >> 32
    if marker not in _VALID_MARKERS:
        return None
    return marker, value & 0xFFFFFFFF
```

- [ ] **Step 4: Run to verify pass**

```powershell
.\.venv\Scripts\python -m pytest test_counter_codec.py -v
```

Expected: all PASS.

- [ ] **Step 5: Write the failing stats tests**

`spike/gate0/probe/test_stats.py`:

```python
from counter_codec import MARKER_LIVE, MARKER_SLATE
from stats import gate_failures, summarize

L = MARKER_LIVE


def steady(seconds=3, fps=30, start=0):
    return [(i / fps, L, start + i) for i in range(int(seconds * fps))]


def test_steady_stream_is_clean():
    s = summarize(steady())
    assert s["dropped"] == 0 and s["repeats"] == 0 and s["freeze_events"] == 0
    assert s["invalid_frames"] == 0
    assert 29 <= s["fps"] <= 31
    assert gate_failures(s) == []


def test_dropped_frames_are_counted_from_counter_jumps():
    samples = steady(1)
    samples = samples[:10] + [(t, m, c + 5) for (t, m, c) in samples[10:]]
    assert summarize(samples)["dropped"] == 5


def test_freeze_is_detected():
    base = steady(1)
    frozen = [(1.0 + i * 0.1, L, 29) for i in range(25)]   # same counter for 2.5 s
    s = summarize(base + frozen)
    assert s["freeze_events"] == 1
    assert "freeze" in " ".join(gate_failures(s))


def test_slate_and_invalid_frames_are_counted_separately():
    samples = steady(1) + [(1.0, MARKER_SLATE, 0), (1.03, None, None)]
    s = summarize(samples)
    assert s["slate_frames"] == 1 and s["invalid_frames"] == 1


def test_large_gap_fails_the_gate():
    samples = steady(1) + [(2.0, L, 30), (2.03, L, 31)]
    s = summarize(samples)
    assert s["max_gap_s"] > 0.5
    assert any("gap" in f for f in gate_failures(s))


def test_empty_input_is_safe():
    s = summarize([])
    assert s["frames"] == 0 and s["fps"] == 0.0
```

- [ ] **Step 6: Run to verify failure**

```powershell
.\.venv\Scripts\python -m pytest test_stats.py -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'stats'`.

- [ ] **Step 7: Implement `stats.py`**

```python
"""Summarise a stream of probe samples (t_seconds, marker_or_None, counter_or_None)."""
from counter_codec import MARKER_LIVE, MARKER_SLATE

MAX_GAP_S = 0.5
MAX_DROP_FRACTION = 0.01
MIN_FPS = 27.0


def summarize(samples, freeze_threshold_s=1.0):
    out = dict(frames=len(samples), duration_s=0.0, fps=0.0, max_gap_s=0.0,
               dropped=0, repeats=0, freeze_events=0, slate_frames=0,
               invalid_frames=0, counter_resets=0)
    if not samples:
        return out
    out["duration_s"] = samples[-1][0] - samples[0][0]
    if len(samples) > 1 and out["duration_s"] > 0:
        out["fps"] = (len(samples) - 1) / out["duration_s"]
        out["max_gap_s"] = max(b[0] - a[0] for a, b in zip(samples, samples[1:]))

    prev = None            # previous LIVE sample
    run_start_t = None     # start time of the current identical-counter run
    run_flagged = False
    for t, marker, counter in samples:
        if marker is None:
            out["invalid_frames"] += 1
            prev, run_start_t, run_flagged = None, None, False
            continue
        if marker == MARKER_SLATE:
            out["slate_frames"] += 1
            prev, run_start_t, run_flagged = None, None, False
            continue
        # LIVE frame
        if prev is not None:
            delta = counter - prev[2]
            if delta > 1:
                out["dropped"] += delta - 1
            elif delta == 0:
                out["repeats"] += 1
            elif delta < 0:
                out["counter_resets"] += 1
            if delta == 0:
                if run_start_t is None:
                    run_start_t = prev[0]
                if not run_flagged and t - run_start_t >= freeze_threshold_s:
                    out["freeze_events"] += 1
                    run_flagged = True
            else:
                run_start_t, run_flagged = None, False
        prev = (t, marker, counter)
    return out


def gate_failures(s):
    fails = []
    if s["freeze_events"] > 0:
        fails.append(f"freeze events: {s['freeze_events']}")
    if s["max_gap_s"] > MAX_GAP_S:
        fails.append(f"max gap {s['max_gap_s']:.2f}s > {MAX_GAP_S}s")
    expected = s["frames"] + s["dropped"]
    if expected and s["dropped"] / expected > MAX_DROP_FRACTION:
        fails.append(f"dropped {s['dropped']}/{expected} > {MAX_DROP_FRACTION:.0%}")
    if s["invalid_frames"] > 0:
        fails.append(f"undecodable frames: {s['invalid_frames']}")
    if s["frames"] > 1 and s["fps"] < MIN_FPS:
        fails.append(f"fps {s['fps']:.1f} < {MIN_FPS}")
    return fails
```

- [ ] **Step 8: Run to verify pass**

```powershell
.\.venv\Scripts\python -m pytest -v
```

Expected: all tests in both files PASS.

- [ ] **Step 9: Write `vcam_probe.py`** (no unit test: it is a thin hardware wrapper around the two tested modules)

```python
"""Read the virtual camera for N minutes and report what it actually delivered.

Usage:  python vcam_probe.py --minutes 30 --out ..\\probe-run-zoom.md
        python vcam_probe.py --list
"""
import argparse
import json
import time

import cv2

from counter_codec import decode_strip
from stats import gate_failures, summarize


def open_cam(index, w, h, fps):
    cap = cv2.VideoCapture(index, cv2.CAP_MSMF)
    cap.set(cv2.CAP_PROP_FRAME_WIDTH, w)
    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, h)
    cap.set(cv2.CAP_PROP_FPS, fps)
    return cap


def find_virtual_camera(w, h, fps, max_index=10):
    for i in range(max_index):
        cap = open_cam(i, w, h, fps)
        try:
            for _ in range(15):
                ok, frame = cap.read()
                if ok and decode_strip(frame) is not None:
                    return i
        finally:
            cap.release()
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--minutes", type=float, default=1.0)
    ap.add_argument("--index", type=int, default=None)
    ap.add_argument("--width", type=int, default=1280)
    ap.add_argument("--height", type=int, default=720)
    ap.add_argument("--fps", type=int, default=30)
    ap.add_argument("--out", default=None, help="append a Markdown summary to this file")
    ap.add_argument("--label", default="run")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    index = args.index if args.index is not None else find_virtual_camera(args.width, args.height, args.fps)
    if index is None:
        raise SystemExit("No camera index delivered a decodable counter strip.")
    if args.list:
        print(f"virtual camera index: {index}")
        return

    cap = open_cam(index, args.width, args.height, args.fps)
    actual = (int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)), int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)))
    samples = []
    t0 = time.monotonic()
    deadline = t0 + args.minutes * 60
    while time.monotonic() < deadline:
        ok, frame = cap.read()
        t = time.monotonic() - t0
        if not ok:
            samples.append((t, None, None))
            time.sleep(0.01)
            continue
        decoded = decode_strip(frame)
        samples.append((t, decoded[0], decoded[1]) if decoded else (t, None, None))
    cap.release()

    summary = summarize(samples)
    failures = gate_failures(summary)
    print(json.dumps({"label": args.label, "index": index, "size": actual, **summary, "failures": failures}, indent=2))
    if args.out:
        with open(args.out, "a", encoding="utf-8") as f:
            f.write(f"\n### {args.label}\n- camera index {index}, delivered size {actual[0]}x{actual[1]}\n")
            f.write("- " + ", ".join(f"{k}={v:.3f}" if isinstance(v, float) else f"{k}={v}" for k, v in summary.items()) + "\n")
            f.write(f"- gate: {'PASS' if not failures else 'FAIL — ' + '; '.join(failures)}\n")


if __name__ == "__main__":
    main()
```

- [ ] **Step 10: Commit**

```powershell
Set-Location "D:\Coding Projects\webcam-loop"
git add spike/gate0/probe
git commit -m "spike(gate0): counter-strip codec, stats and virtual camera probe with tests"
```

---

### Task 5: Test pattern with counter strip, multi-format output

**Files:**
- Create: `spike/gate0/vcam-sample/gate0/TestPattern.h`
- Modify: the sample's frame-generation function found in Task 2 Step 3
- Modify: `spike/gate0/REPORT.md` (section 4)

**Interfaces:**
- Consumes: the strip layout from Task 4; the frame-generation site and pixel format from Task 2.
- Produces: `gate0::RenderTestPattern(...)` and `gate0::BgraToNv12(...)` (used by Task 6); a camera that emits a moving, probe-readable pattern in the formats the sample advertises.

- [ ] **Step 1: Write `TestPattern.h`**

```cpp
// THROWAWAY (Gate 0). Test pattern with the machine-readable counter strip described in probe/counter_codec.py.
#pragma once
#include <cstdint>

namespace gate0 {

constexpr int kBlock = 8;
constexpr int kBits = 48;
constexpr uint16_t kMarkerLive = 0xA5C3;
constexpr uint16_t kMarkerSlate = 0x5A3C;

inline void PutPixel(uint8_t* row, int x, uint8_t r, uint8_t g, uint8_t b) {
    uint8_t* p = row + x * 4;
    p[0] = b; p[1] = g; p[2] = r; p[3] = 255;
}

// Draws 8 colour bars, a vertical sweep line that moves 8 px per frame, and the counter strip.
// A slate (marker == kMarkerSlate) is a flat dark grey frame with the strip, so it can never be mistaken for live video.
inline void RenderTestPattern(uint8_t* bgra, int w, int h, int stride, uint32_t frameIndex, uint16_t marker) {
    static const uint8_t bars[8][3] = {
        {255,255,255},{255,255,0},{0,255,255},{0,255,0},{255,0,255},{255,0,0},{0,0,255},{0,0,0}};
    const bool slate = (marker == kMarkerSlate);
    const int sweepX = static_cast<int>((static_cast<uint64_t>(frameIndex) * 8) % static_cast<uint32_t>(w));
    for (int y = 0; y < h; ++y) {
        uint8_t* row = bgra + static_cast<size_t>(y) * stride;
        for (int x = 0; x < w; ++x) {
            if (slate) { PutPixel(row, x, 40, 40, 40); continue; }
            const uint8_t* c = bars[x * 8 / w];
            if (y >= kBlock && x >= sweepX && x < sweepX + 6) PutPixel(row, x, 255, 255, 255);
            else PutPixel(row, x, c[0], c[1], c[2]);
        }
    }
    const uint64_t value = (static_cast<uint64_t>(marker) << 32) | frameIndex;
    for (int i = 0; i < kBits && (i + 1) * kBlock <= w; ++i) {
        const uint8_t v = ((value >> (kBits - 1 - i)) & 1) ? 255 : 0;
        for (int y = 0; y < kBlock && y < h; ++y) {
            uint8_t* row = bgra + static_cast<size_t>(y) * stride;
            for (int x = i * kBlock; x < (i + 1) * kBlock; ++x) PutPixel(row, x, v, v, v);
        }
    }
}

// BT.601 limited range. Output planes: Y (w*h, stride yStride) then interleaved UV (w*(h/2), same stride).
inline void BgraToNv12(const uint8_t* bgra, int w, int h, int srcStride, uint8_t* nv12, int yStride) {
    uint8_t* yPlane = nv12;
    uint8_t* uvPlane = nv12 + static_cast<size_t>(yStride) * h;
    for (int y = 0; y < h; ++y) {
        const uint8_t* s = bgra + static_cast<size_t>(y) * srcStride;
        for (int x = 0; x < w; ++x) {
            const int b = s[x*4], g = s[x*4+1], r = s[x*4+2];
            yPlane[static_cast<size_t>(y) * yStride + x] = static_cast<uint8_t>(((66*r + 129*g + 25*b + 128) >> 8) + 16);
            if ((y & 1) == 0 && (x & 1) == 0) {
                uint8_t* uv = uvPlane + static_cast<size_t>(y / 2) * yStride + x;
                uv[0] = static_cast<uint8_t>(((-38*r - 74*g + 112*b + 128) >> 8) + 128);
                uv[1] = static_cast<uint8_t>(((112*r - 94*g - 18*b + 128) >> 8) + 128);
            }
        }
    }
}

}  // namespace gate0
```

- [ ] **Step 2: Plug it into the sample**

At the frame-generation site recorded in Task 2 Step 3: keep a monotonically increasing `uint32_t frameIndex` per stream (reset to 0 on stream start); render into a temporary BGRA buffer of the negotiated width and height; if the stream subtype is RGB32/ARGB32 copy rows into the sample buffer, if NV12 call `gate0::BgraToNv12`. Add `#include "gate0/TestPattern.h"` and guard the change with `#ifndef GATE0_USE_PRODUCER` so Task 6 can switch it. Keep the sample's own timestamping. Make the source also advertise **1280x720, 640x360 and 1920x1080 at 30 fps** if it only offers one size (add media types in the sample's type-list code); record every type offered.

- [ ] **Step 3: Rebuild, register, probe**

```powershell
msbuild $sln /p:Configuration=Release /p:Platform=x64 /m
# register using the exact command recorded in REPORT.md section 3, then:
Set-Location spike\gate0\probe
.\.venv\Scripts\python vcam_probe.py --minutes 2 --label "pattern-2min" --out ..\REPORT.md
.\.venv\Scripts\python vcam_probe.py --minutes 1 --width 640 --height 360 --label "pattern-640x360" --out ..\REPORT.md
```

Expected: `gate: PASS` for 1280x720 and for 640x360 with `invalid_frames=0`. If NV12 conversion shifts the strip colors out of tolerance, the probe will show `invalid_frames > 0`; fix the conversion rather than the threshold. Pin for Review Focus 3: also open the camera in Chrome's test page at 640x360 and 1920x1080 and record which sizes each shows.

- [ ] **Step 4: Record formats**

In `REPORT.md` section 4, list every (subtype, width, height, fps) the source offers and which sizes/formats each app chose (filled in as Task 7 progresses).

- [ ] **Step 5: Unregister, then commit**

```powershell
# run the unregister procedure from REPORT.md section 9
git add spike/gate0
git commit -m "spike(gate0): moving test pattern with counter strip in all offered formats"
```

---

### Task 6: Shared memory across the Frame Server boundary (reader tests, producer, slate)

**Files:**
- Create: `spike/gate0/vcam-sample/gate0/SharedFrame.h`, `SharedSection.h`, `SharedFrameTests.cpp`
- Create: `spike/gate0/producer/Cargo.toml`, `spike/gate0/producer/src/main.rs`
- Modify: the sample's frame-generation site; `spike/gate0/REPORT.md` (section 5)

**Interfaces:**
- Consumes: `RenderTestPattern`, `kMarkerSlate` from Task 5.
- Produces:
  - Shared-memory contract, little-endian, 64-byte header then pixels. Offsets: 0 `u32 magic = 0x504C4357`; 4 `u32 version = 1`; 8 `u32 width`; 12 `u32 height`; 16 `u32 stride`; 20 `u32 format = 1` (BGRA32); 24 `u32 seq` (odd while writing); 28 reserved; 32 `u64 frame_counter`; 40 `u64 heartbeat_ms` (`GetTickCount64` at last write); 48..63 reserved; 64.. pixels (`stride * height` bytes).
  - `gate0::ReadStatus TryReadFrame(const uint8_t* base, size_t mappedSize, uint64_t nowMs, uint32_t expectW, uint32_t expectH, uint8_t* dst, uint32_t dstStride, uint64_t* frameCounter)`
  - `gate0::SharedSection::EnsureOpen(const wchar_t* name, size_t len, bool createIfMissing)` and `base()`
  - A finding: the section name and DACL scheme that works from user session to Frame Server.

- [ ] **Step 1: Write the failing reader tests** — `SharedFrameTests.cpp`

```cpp
#include <cassert>
#include <cstdio>
#include <cstring>
#include <vector>
#include <atomic>
#include "SharedFrame.h"
using namespace gate0;

static std::vector<uint8_t> MakeBuffer(uint32_t w, uint32_t h, uint64_t counter, uint64_t heartbeat, uint32_t seq = 2) {
    std::vector<uint8_t> b(64 + static_cast<size_t>(w) * 4 * h, 0);
    auto put32 = [&](size_t o, uint32_t v) { std::memcpy(&b[o], &v, 4); };
    auto put64 = [&](size_t o, uint64_t v) { std::memcpy(&b[o], &v, 8); };
    put32(0, kMagic); put32(4, 1); put32(8, w); put32(12, h); put32(16, w * 4); put32(20, 1); put32(24, seq);
    put64(32, counter); put64(40, heartbeat);
    for (size_t i = 64; i < b.size(); ++i) b[i] = static_cast<uint8_t>(i);
    return b;
}

int main() {
    std::vector<uint8_t> dst(4 * 4 * 2);
    uint64_t counter = 0;

    { auto b = MakeBuffer(4, 2, 77, 1000);
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::Ok);
      assert(counter == 77 && dst[0] == 64 && dst[31] == 95); }

    { auto b = MakeBuffer(4, 2, 1, 1000); b[0] ^= 0xFF;
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::BadHeader); }

    { auto b = MakeBuffer(8, 2, 1, 1000);
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::SizeMismatch); }

    { auto b = MakeBuffer(4, 2, 1, 1000);
      assert(TryReadFrame(b.data(), b.size(), 2501, 4, 2, dst.data(), 16, &counter) == ReadStatus::Stale); }

    { auto b = MakeBuffer(4, 2, 1, 1000, /*seq=*/3);   // odd = writer mid-update
      assert(TryReadFrame(b.data(), b.size(), 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::Torn); }

    { auto b = MakeBuffer(4, 2, 1, 1000);               // mapping shorter than the header claims
      assert(TryReadFrame(b.data(), b.size() - 8, 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::BadHeader); }

    { assert(TryReadFrame(nullptr, 0, 1200, 4, 2, dst.data(), 16, &counter) == ReadStatus::BadHeader); }

    { auto b = MakeBuffer(4, 2, 1, 5000);                // heartbeat in the future must not underflow into "stale"
      assert(TryReadFrame(b.data(), b.size(), 1000, 4, 2, dst.data(), 16, &counter) == ReadStatus::Ok); }

    std::puts("SharedFrameTests: all passed");
    return 0;
}
```

- [ ] **Step 2: Run to verify failure**

```powershell
Set-Location "D:\Coding Projects\webcam-loop\spike\gate0\vcam-sample\gate0"
cl /nologo /std:c++17 /EHsc SharedFrameTests.cpp
```

Expected: FAIL with `Cannot open include file: 'SharedFrame.h'`.

- [ ] **Step 3: Implement `SharedFrame.h`**

```cpp
// THROWAWAY (Gate 0). Shared-memory frame contract + tear-safe reader.
#pragma once
#include <atomic>
#include <cstdint>
#include <cstring>

namespace gate0 {

constexpr uint32_t kMagic = 0x504C4357;      // "WCLP"
constexpr size_t kHeaderBytes = 64;
constexpr uint64_t kStaleAfterMs = 1000;

enum class ReadStatus { Ok, BadHeader, SizeMismatch, Stale, Torn };

inline ReadStatus TryReadFrame(const uint8_t* base, size_t mappedSize, uint64_t nowMs,
                               uint32_t expectW, uint32_t expectH,
                               uint8_t* dst, uint32_t dstStride, uint64_t* frameCounter) {
    if (!base || mappedSize < kHeaderBytes) return ReadStatus::BadHeader;
    auto rd32 = [&](size_t o) { uint32_t v; std::memcpy(&v, base + o, 4); return v; };
    auto rd64 = [&](size_t o) { uint64_t v; std::memcpy(&v, base + o, 8); return v; };
    const auto* seqPtr = reinterpret_cast<const std::atomic<uint32_t>*>(base + 24);

    const uint32_t seq1 = seqPtr->load(std::memory_order_acquire);
    if (seq1 & 1u) return ReadStatus::Torn;

    if (rd32(0) != kMagic || rd32(4) != 1 || rd32(20) != 1) return ReadStatus::BadHeader;
    const uint32_t w = rd32(8), h = rd32(12), stride = rd32(16);
    const uint64_t counter = rd64(32), heartbeat = rd64(40);
    if (w != expectW || h != expectH) return ReadStatus::SizeMismatch;
    if (stride < w * 4u || kHeaderBytes + static_cast<size_t>(stride) * h > mappedSize) return ReadStatus::BadHeader;
    if (nowMs > heartbeat && nowMs - heartbeat > kStaleAfterMs) return ReadStatus::Stale;

    const uint32_t rowBytes = w * 4u;
    for (uint32_t y = 0; y < h; ++y)
        std::memcpy(dst + static_cast<size_t>(y) * dstStride, base + kHeaderBytes + static_cast<size_t>(y) * stride, rowBytes);

    std::atomic_thread_fence(std::memory_order_acquire);
    if (seqPtr->load(std::memory_order_acquire) != seq1) return ReadStatus::Torn;
    *frameCounter = counter;
    return ReadStatus::Ok;
}

}  // namespace gate0
```

- [ ] **Step 4: Run to verify pass**

```powershell
cl /nologo /std:c++17 /EHsc SharedFrameTests.cpp ; .\SharedFrameTests.exe
```

Expected: `SharedFrameTests: all passed`.

- [ ] **Step 5: Implement `SharedSection.h`**

```cpp
// THROWAWAY (Gate 0). Opens (or creates) the named section; retries are throttled so a missing producer costs nothing.
#pragma once
#include <windows.h>
#include <sddl.h>
#include <cstdint>

namespace gate0 {

class SharedSection {
public:
    ~SharedSection() { Close(); }

    bool EnsureOpen(const wchar_t* name, size_t len, bool createIfMissing) {
        if (m_base) return true;
        const ULONGLONG now = GetTickCount64();
        if (m_lastAttempt && now - m_lastAttempt < 500) return false;
        m_lastAttempt = now;

        m_handle = OpenFileMappingW(FILE_MAP_READ, FALSE, name);
        if (!m_handle && createIfMissing) {
            PSECURITY_DESCRIPTOR sd = nullptr;
            // Everyone: generic all. Spike only; the product tightens this.
            if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"D:(A;;GA;;;WD)", SDDL_REVISION_1, &sd, nullptr)) return false;
            SECURITY_ATTRIBUTES sa{ sizeof(sa), sd, FALSE };
            m_handle = CreateFileMappingW(INVALID_HANDLE_VALUE, &sa, PAGE_READWRITE, 0, static_cast<DWORD>(len), name);
            LocalFree(sd);
        }
        if (!m_handle) { m_lastError = GetLastError(); return false; }
        m_base = static_cast<const uint8_t*>(MapViewOfFile(m_handle, FILE_MAP_READ, 0, 0, len));
        if (!m_base) { m_lastError = GetLastError(); CloseHandle(m_handle); m_handle = nullptr; return false; }
        m_len = len;
        return true;
    }
    const uint8_t* base() const { return m_base; }
    size_t size() const { return m_len; }
    DWORD lastError() const { return m_lastError; }

    void Close() {
        if (m_base) { UnmapViewOfFile(m_base); m_base = nullptr; }
        if (m_handle) { CloseHandle(m_handle); m_handle = nullptr; }
    }

private:
    HANDLE m_handle = nullptr;
    const uint8_t* m_base = nullptr;
    size_t m_len = 0;
    ULONGLONG m_lastAttempt = 0;
    DWORD m_lastError = 0;
};

}  // namespace gate0
```

- [ ] **Step 6: Plug the reader into the sample behind `GATE0_USE_PRODUCER`**

At the same generation site: `static gate0::SharedSection s_section;` then each frame

```cpp
#ifdef GATE0_USE_PRODUCER
    constexpr size_t kMapBytes = gate0::kHeaderBytes + 1280u * 720u * 4u;
    uint64_t producerCounter = 0;
    gate0::ReadStatus st = gate0::ReadStatus::BadHeader;
    if (s_section.EnsureOpen(L"Global\\gate0-vcam-frames", kMapBytes, /*createIfMissing=*/false))
        st = gate0::TryReadFrame(s_section.base(), s_section.size(), GetTickCount64(), width, height, bgraBuf, stride, &producerCounter);
    if (st == gate0::ReadStatus::Ok) {
        // bgraBuf now holds the producer's frame (its own counter strip, marker LIVE)
    } else if (st != gate0::ReadStatus::Torn) {
        gate0::RenderTestPattern(bgraBuf, width, height, stride, slateIndex++, gate0::kMarkerSlate);   // never a frozen "live" frame
    }   // Torn: keep the previous bgraBuf content for this one tick only
#else
    gate0::RenderTestPattern(bgraBuf, width, height, stride, frameIndex++, gate0::kMarkerLive);
#endif
```

Build twice: once normally, once with `/p:ExternalCompilerOptions=/DGATE0_USE_PRODUCER` (or define it in the project). Unit-test coverage for the reader's rules is in Step 4; Review Focus 4 (wrong size, truncated mapping, half-written frame) is pinned there by the `SizeMismatch`, short-mapping and odd-`seq` cases.

- [ ] **Step 7: Write the Rust producer** — `producer/Cargo.toml`

```toml
[package]
name = "gate0-producer"
version = "0.0.1"
edition = "2021"

[dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_Security_Authorization",
    "Win32_System_Memory",
    "Win32_System_SystemInformation",
] }
```

`producer/src/main.rs` (written against `windows` 0.58; if a signature differs in the installed version, adjust the call, not the behavior):

```rust
//! THROWAWAY (Gate 0). Writes solid-colour frames with the counter strip into the shared section.
//! Usage: gate0-producer [--name Global\gate0-vcam-frames] [--mode create|open] [--fps 30]
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use std::{env, thread};

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::System::Memory::{CreateFileMappingW, MapViewOfFile, OpenFileMappingW, FILE_MAP_ALL_ACCESS, PAGE_READWRITE};
use windows::Win32::System::SystemInformation::GetTickCount64;

const W: u32 = 1280;
const H: u32 = 720;
const HEADER: usize = 64;
const LEN: usize = HEADER + (W * H * 4) as usize;
const MAGIC: u32 = 0x504C_4357;
const BLOCK: usize = 8;
const BITS: usize = 48;
const MARKER_LIVE: u64 = 0xA5C3;

struct Section { _handle: HANDLE, base: *mut u8 }

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned().unwrap_or_else(|| default.to_string())
}

fn create_section(name: &HSTRING) -> windows::core::Result<Section> {
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // Everyone: generic all. Spike only.
        ConvertStringSecurityDescriptorToSecurityDescriptorW(w!("D:(A;;GA;;;WD)"), SDDL_REVISION_1, &mut sd, None)?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: false.into(),
        };
        let handle = CreateFileMappingW(INVALID_HANDLE_VALUE, Some(&sa as *const _), PAGE_READWRITE, 0, LEN as u32, name)?;
        map(handle)
    }
}

fn open_section(name: &HSTRING) -> windows::core::Result<Section> {
    unsafe { map(OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, name)?) }
}

unsafe fn map(handle: HANDLE) -> windows::core::Result<Section> {
    let view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, LEN);
    if view.Value.is_null() {
        return Err(windows::core::Error::from_win32());
    }
    Ok(Section { _handle: handle, base: view.Value as *mut u8 })
}

unsafe fn publish(s: &Section, counter: u64) {
    let seq = &*(s.base.add(24) as *const AtomicU32);
    seq.fetch_add(1, Ordering::AcqRel);                       // odd: writing
    let put32 = |o: usize, v: u32| (s.base.add(o) as *mut u32).write_unaligned(v);
    put32(0, MAGIC); put32(4, 1); put32(8, W); put32(12, H); put32(16, W * 4); put32(20, 1);
    (s.base.add(32) as *mut u64).write_unaligned(counter);
    (s.base.add(40) as *mut u64).write_unaligned(GetTickCount64());

    let pixels = std::slice::from_raw_parts_mut(s.base.add(HEADER), (W * H * 4) as usize);
    let hue = (counter % 256) as u8;
    for px in pixels.chunks_exact_mut(4) {
        px[0] = hue; px[1] = 255 - hue; px[2] = 128; px[3] = 255;       // B, G, R, A
    }
    let value: u64 = (MARKER_LIVE << 32) | (counter & 0xFFFF_FFFF);
    for i in 0..BITS {
        let v = if (value >> (BITS - 1 - i)) & 1 == 1 { 255u8 } else { 0u8 };
        for y in 0..BLOCK {
            for x in (i * BLOCK)..((i + 1) * BLOCK) {
                let o = (y * W as usize + x) * 4;
                pixels[o] = v; pixels[o + 1] = v; pixels[o + 2] = v; pixels[o + 3] = 255;
            }
        }
    }
    seq.fetch_add(1, Ordering::Release);                      // even: done
}

fn main() {
    let name = HSTRING::from(arg("--name", "Global\\gate0-vcam-frames"));
    let mode = arg("--mode", "create");
    let fps: u64 = arg("--fps", "30").parse().unwrap_or(30);

    let section = loop {
        let r = if mode == "open" { open_section(&name) } else { create_section(&name) };
        match r {
            Ok(s) => break s,
            Err(e) if mode == "open" => { eprintln!("waiting for section: {e}"); thread::sleep(Duration::from_millis(250)); }
            Err(e) => { eprintln!("cannot create section {name}: {e}"); std::process::exit(1); }
        }
    };
    eprintln!("producer running, mode={mode}, {W}x{H} @ {fps} fps");

    let interval = Duration::from_micros(1_000_000 / fps);
    let mut next = Instant::now();
    let mut counter: u64 = 0;
    loop {
        unsafe { publish(&section, counter) };
        counter += 1;
        next += interval;
        if let Some(d) = next.checked_duration_since(Instant::now()) { thread::sleep(d); }
    }
}
```

- [ ] **Step 8: Build the producer and run the boundary experiments**

```powershell
Set-Location "D:\Coding Projects\webcam-loop\spike\gate0\producer"
cargo build --release
```

Expected: builds. Then, with the camera built in `GATE0_USE_PRODUCER` mode and registered:

1. **Experiment A — producer creates, non-elevated:** run `target\release\gate0-producer.exe` from a normal (non-elevated) PowerShell. If it prints `cannot create section ... Access is denied`, that is the finding that a normal user cannot create a `Global\` section; record the exact error.
2. **Experiment A2 — producer creates, elevated:** run it from an elevated PowerShell. Run the probe: expect `marker LIVE` frames. If the probe sees only SLATE, the Frame Server cannot open the section (permissions or namespace). Record it.
3. **Experiment B — the camera DLL creates the section** (`createIfMissing=true` in the DLL call, which runs with service privileges) and the producer attaches with `--mode open` from a normal PowerShell. Record whether a normal-user producer can open and write it.
4. If neither works, try `Local\` and the session-specific `\Sessions\<n>\BaseNamedObjects` forms, and a named-pipe transport as a third option. Do not spend more than half a day here: write down what you tried.

In `REPORT.md` section 5 record the working scheme (name, who creates, DACL, elevation) or state that none works. **If none works, Gate 0 fails on the frame-delivery model even if apps accept the camera; stop and escalate.**

- [ ] **Step 9: Slate behavior and cleanup**

With the working scheme: start the producer, confirm the probe reports LIVE; `Stop-Process -Name gate0-producer -Force`; within 1.5 s the probe must report SLATE frames (and never repeated LIVE counters, i.e. `freeze_events=0`); restart the producer and confirm LIVE resumes. Run the probe for 2 minutes across this sequence with `--label "kill-and-recover"`. Then unregister the camera and verify cleanup per section 9.

- [ ] **Step 10: Commit**

```powershell
Set-Location "D:\Coding Projects\webcam-loop"
git add spike/gate0
git commit -m "spike(gate0): shared-memory reader with tests, Rust producer, boundary findings"
```

---

### Task 7: Compatibility matrix and 30-minute soaks

**Files:**
- Modify: `spike/gate0/REPORT.md` (sections 4, 6, 7, 10)

**Interfaces:**
- Consumes: the working camera + producer from Task 6, the probe from Task 4.
- Produces: the filled compatibility matrix; per-app probe results.

Camera name in each app's picker is the friendly name recorded in Task 3 Step 2.

- [ ] **Step 1: Discovery, per app**

For each of Zoom, Teams (new), Google Meet in Chrome, Google Meet in Edge, Discord: start the producer first, then launch the app, open its video settings (Zoom: Settings → Video; Teams: Settings → Devices; Meet: `https://meet.new` → Settings → Video; Discord: User Settings → Voice & Video), and pick the virtual camera. Record in the matrix: appears yes/no, picture correct yes/no, size the app chose (from the app's stats if shown), and any errors.

- [ ] **Step 2: Restart discovery, per app**

(a) Quit and relaunch the app: the camera must still be listed and selected state recorded. (b) Restart Windows, start the producer, launch the app: still listed. Record both.

- [ ] **Step 3: 30-minute soak, per app (observed in-call)**

Start a call with self-view (Zoom new meeting; Teams "Meet now"; Meet via meet.new; Discord video call to yourself via a test server voice channel or Settings → Video Test) with the virtual camera selected at the highest resolution the app offers. Keep it up 30 minutes. In the same time window run:

```powershell
Set-Location "D:\Coding Projects\webcam-loop\spike\gate0\probe"
.\.venv\Scripts\python vcam_probe.py --minutes 30 --label "soak-<app>" --out ..\REPORT.md
```

as a second consumer. If opening the camera in the probe fails while the app has it open, record that (Review Focus 2) and instead run two separate 30-minute passes: probe alone, then app alone. Watch the self-view: the sweep line must move smoothly; the counter strip must keep changing. Record freezes or stutters you see, with times.

- [ ] **Step 4: 720p30 check**

For each app confirm it is actually receiving 1280x720 at about 30 fps (app stats panel: Zoom "Statistics → Video"; Teams "Call health"; Meet "Troubleshooting → Video"; Discord `Ctrl+Shift+I` is not needed, use the probe's reported size as the camera-side figure). Record.

- [ ] **Step 5: Producer restart and kill, in-call**

During a call in each app: (a) `Stop-Process -Name gate0-producer -Force`, confirm the app shows the flat grey slate (not a frozen colored frame), restart the producer, confirm normal video returns without re-selecting the camera. Record any app that needs the camera re-selected or drops to black.

- [ ] **Step 6: Fill the matrix and failures**

Complete `REPORT.md` section 6 (every cell a result, not blank) and section 10 (each failure: app, step, symptom, suspected cause).

- [ ] **Step 7: Commit**

```powershell
git add spike/gate0/REPORT.md
git commit -m "spike(gate0): compatibility matrix and soak results"
```

---

### Task 8: Camera lifecycle and cleanup under stress

**Files:**
- Modify: `spike/gate0/REPORT.md` (sections 3, 9)

**Interfaces:**
- Produces: a verified lifecycle list that Plan 3 turns into installer requirements.

- [ ] **Step 1: Crash-leftover test (Review Focus 5)**

Register the camera, then `Stop-Process` the registering process (or close its window abruptly) while the camera is active, and check whether the camera is still present and still works. Check after logoff/logon and after a reboot. Record whether the camera disappears with the session or persists, and what registration lifetime the sample uses (session vs. system).

- [ ] **Step 2: Double registration and unregister-while-in-use**

Register twice; list cameras. Unregister while Chrome's camera test page is streaming; record what the page shows afterward and whether the camera lingers in pickers.

- [ ] **Step 3: Upgrade simulation**

With the camera registered, rebuild the DLL with a visibly different pattern (change one bar color) and reinstall over the top following the sample's procedure. Record whether the new DLL is picked up without a reboot and whether in-use files block replacement.

- [ ] **Step 4: Write the final cleanup/uninstall procedure**

Merge all findings into `REPORT.md` section 9 as a numbered, tested procedure (including any registry keys or files to delete and any reboot).

- [ ] **Step 5: Commit**

```powershell
git add spike/gate0/REPORT.md
git commit -m "spike(gate0): lifecycle, double registration, upgrade and cleanup findings"
```

---

### Task 9: MSIX registration experiment

**Files:**
- Create: `spike/gate0/msix/AppxManifest.xml`, `spike/gate0/msix/Assets/logo.png`
- Modify: `spike/gate0/REPORT.md` (section 8)

**Interfaces:**
- Produces: a verdict on which MSIX registration model works, answering the spec's "whether the registration model works with the intended MSIX packaging."

- [ ] **Step 1: Create a minimal packaged-app manifest** — `msix/AppxManifest.xml`

```xml
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities">
  <Identity Name="Gate0.VCamSpike" Publisher="CN=Gate0Spike" Version="0.0.1.0" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>Gate0 VCam Spike</DisplayName>
    <PublisherDisplayName>Gate0</PublisherDisplayName>
    <Logo>Assets\logo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.22000.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources><Resource Language="en-us" /></Resources>
  <Applications>
    <Application Id="Registrar" Executable="registrar.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="Gate0 VCam Spike" Description="Gate 0 spike" BackgroundColor="transparent"
                          Square150x150Logo="Assets\logo.png" Square44x44Logo="Assets\logo.png" />
    </Application>
  </Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
```

`registrar.exe` is the sample's registration/config executable recorded in Task 2 (rename the `Executable` to match). Create the logo:

```powershell
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory spike\gate0\msix\Assets -Force | Out-Null
$bmp = New-Object System.Drawing.Bitmap 150,150
([System.Drawing.Graphics]::FromImage($bmp)).Clear([System.Drawing.Color]::DarkSlateBlue)
$bmp.Save("spike\gate0\msix\Assets\logo.png"); $bmp.Dispose()
```

- [ ] **Step 2: Create a throwaway test certificate and package** (do **not** reuse any Cloakly signing key; keys stay out of git)

```powershell
$cert = New-SelfSignedCertificate -Type Custom -Subject "CN=Gate0Spike" -KeyUsage DigitalSignature -FriendlyName "Gate0 spike" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3","2.5.29.19={text}")
$stage = "$env:TEMP\gate0-msix-stage"; Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue; New-Item -ItemType Directory $stage | Out-Null
Copy-Item spike\gate0\msix\AppxManifest.xml, spike\gate0\msix\Assets $stage -Recurse
# copy the built sample DLL(s) and registrar.exe into $stage (paths from Task 2 Step 4)
$mk = (Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makeappx.exe | Where-Object FullName -match 'x64' | Select-Object -First 1).FullName
$sg = (Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe | Where-Object FullName -match 'x64' | Select-Object -First 1).FullName
& $mk pack /d $stage /p "$env:TEMP\gate0.msix" /nv
& $sg sign /fd SHA256 /sha1 $cert.Thumbprint "$env:TEMP\gate0.msix"
Export-Certificate -Cert $cert -FilePath "$env:TEMP\gate0.cer"
```

Install the cert as trusted (elevated): `Import-Certificate -FilePath "$env:TEMP\gate0.cer" -CertStoreLocation Cert:\LocalMachine\TrustedPeople`. Then `Add-AppxPackage "$env:TEMP\gate0.msix"`.

- [ ] **Step 3: Run the three models and record each**

1. **Model P (fully packaged):** launch the packaged app and register the camera from inside the package. Check Windows Camera app and the probe: does Frame Server load the packaged DLL? MSIX filesystem and registry virtualization can hide the COM registration from Frame Server, so expect this may fail.
2. **Model H (packaged app + external helper):** the packaged app launches a helper that does the COM registration outside package virtualization (for example `regsvr32` elevated, or an installer step), then calls the camera creation. Record whether that works and what elevation it requires.
3. **Model N (not MSIX):** the classic installer from Task 3 (already known). This is the baseline for comparison.

Record for each: works / fails, elevation prompts, and what the user would experience at install and uninstall. Then uninstall: `Get-AppxPackage Gate0.VCamSpike | Remove-AppxPackage` and verify the camera is gone (cleanup procedure in section 9).

- [ ] **Step 4: Write the verdict**

In `REPORT.md` section 8: which model(s) work, and the implication for Plan 3: (a) fully packaged is fine, (b) MSIX app plus a one-time elevated helper, or (c) the product needs a non-MSIX installer for the camera component. If no model works, say so; that is a Gate 0 finding about distribution (spec section 8) and not necessarily about compatibility.

- [ ] **Step 5: Commit** (the manifest and logo only; no `.msix`, `.cer`, `.pfx`)

```powershell
git add spike/gate0/msix spike/gate0/REPORT.md
git commit -m "spike(gate0): MSIX registration experiment and verdict"
```

---

### Task 10: Report, decision, and spec follow-ups

**Files:**
- Modify: `spike/gate0/REPORT.md` (sections 11 and header), `docs/superpowers/specs/2026-10-14-webcam-loop-design.md` (only if findings require it)

- [ ] **Step 1: Evaluate against the gate**

Mark each of the four gate criteria PASS/FAIL per app from section 6, and check the stated thresholds against the section 7 probe results. The gate passes only if **every** cell for **every** app passes, the frame-delivery model works (section 5), and registration/uninstall is repeatable (sections 3, 9).

- [ ] **Step 2: Write the decision**

Set the first line of `REPORT.md` to one of:
- `Decision: PASS — proceed to Plan 2 (looper and compositor).` List the chosen camera DLL language recommendation with reasons (Rust vs. C++ based on how the sample and Task 6 went), the required IPC scheme, the registration/elevation model and the MSIX verdict.
- `Decision: FAIL — Zoom/Teams rejected the native virtual camera.` Per the decision rule: scope a DirectShow filter spike with the same criteria and revise the spec before any other work.
- `Decision: FAIL — frame delivery or distribution model.` Name the failing piece and the options (alternate IPC; non-MSIX installer).

- [ ] **Step 3: Update the spec if findings require it**

The spec currently assumes shared memory works across the Frame Server boundary (section 4.2) and that MSIX can carry the registration (sections 8, 9a). If Task 6 or Task 9 contradicted either, edit those sections to match the measured facts and add a line to the spec's open items. Do not widen scope.

- [ ] **Step 4: Commit**

```powershell
git add spike/gate0/REPORT.md docs
git commit -m "spike(gate0): final report and decision"
```

- [ ] **Step 5: Hand back**

Tell the user the decision and ask whether to write Plan 2. If the decision is FAIL, write the fallback spike plan instead. Do not start any non-spike work.
