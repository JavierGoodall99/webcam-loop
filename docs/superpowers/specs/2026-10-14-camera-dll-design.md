# Camera DLL and frame protocol (Plan 3a) — Design

Status: draft for review. Date: 2026-10-14. Parent spec: `2026-10-14-webcam-loop-design.md` (sections 4.2, 4.3, 4.4 and 11). Evidence: `spike/gate0/REPORT.md`.

## 1. Summary

Turn the Gate 0 spike camera into the real camera. It gets a proper frame protocol between the desktop app and the camera DLL, a pacing design that delivers every frame once, a slate when the app goes quiet, restricted access to the frames, and tested install and removal scripts. It is the first part of Plan 3; capture, JPEG, hotkeys and UI (3b, 3c) and licensing (Plan 4) are not here.

## 2. Decisions already made with the owner

| Decision | Choice |
|---|---|
| Smoothness target | **At most 1%** of delivered frames skipped or repeated, measured as in Gate 0. If no approach reaches it, stop and ask the owner before building anything on a weaker design. |
| Zoom | Included in 3a's acceptance check (the owner will install it). |
| Order of attack | Pacing approach A first; B only if A misses 1%; then C; if none reach 1%, stop and ask. |
| Who may open the frame memory | The logged-in console user and the system only. If the DLL cannot resolve that user from inside the camera service, fall back to "any interactively logged-in user" and tell the owner. Never "Everyone". |

Assumed (not yet confirmed by the owner): the DLL stays C++, based on Microsoft's MIT-licensed `Samples/VirtualCamera` (licence notice kept); Rust would mean writing the COM media source from scratch, which is unproven.

## 3. Components and where they live

All in the existing `webcam-loop` repo for now (no new repos until the product name is decided).

| Component | Path | Language | What it is |
|---|---|---|---|
| Frame protocol, reference writer | `crates/vcam-protocol/` | Rust | Layout constants, header validation, the writer algorithm over plain memory (testable without Windows). Reused by Plan 3b. A small Windows test producer binary lives in `examples/`. |
| Frame protocol, reader | `camera-dll/src/vcam_ring.h` | C++ | The reader algorithm over a pointer; no Windows types. |
| Relay | `camera-dll/src/relay.h` | C++ | Pure logic: which frame goes to which waiting request, and when to show the slate. No Windows code, so it is tested by simulation. |
| Media source | `camera-dll/src/` | C++ | Microsoft's sample with the relay and a worker thread wired in. |
| Install tooling | `camera-dll/scripts/` | PowerShell | Elevated one-time registration (new file name, repoint, restart camera services), removal, and verification. Productized from the Gate 0 scripts. |
| Measuring tools | `tools/browser-probe/` | JS + Python | The shared-mode browser probe promoted from the spike (with its tests). |

## 4. Frame protocol, version 2

A named shared-memory section `Global\WCL-Frames-2` and an auto-reset event `Global\WCL-FrameReady-2` (names are placeholders until the product has a name). **The camera DLL creates both** (it runs with service privileges); the app opens them. All integers are little-endian.

**Header (256 bytes):**

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | magic `0x324C4357` ("WCL2") |
| 4 | u32 | version = 2 |
| 8 | u32 | width |
| 12 | u32 | height |
| 16 | u32 | pixel format (1 = BGRA32) |
| 20 | u32 | stride in bytes |
| 24 | u32 | slot count = 4 |
| 28 | u32 | slot stride in bytes (64-byte aligned: `align64(64 + stride * height)`) |
| 32 | u64 | `latest` frame counter (0 = no frame yet; counters start at 1) |
| 40 | u64 | heartbeat (`GetTickCount64`), refreshed on every publish and at least every 100 ms |
| 48 | u32 | producer process id |
| 56 | u64 | QPC frequency (ticks per second) |
| 64..255 | | reserved, zero |

**Slot `i`** (at `256 + i * slot stride`): a 64-byte slot header then the pixels.

| Offset in slot | Type | Field |
|---|---|---|
| 0 | u32 | `seq` (odd while the writer is updating) |
| 8 | u64 | frame counter stored in this slot |
| 16 | u64 | capture time in QPC ticks |
| 64.. | bytes | `stride * height` pixel bytes |

**Writer (per frame `n`, slot `n % 4`):** `seq += 1` (odd), write the pixels, write `counter = n` and the capture time, `seq += 1` (even), then store `latest = n` with release ordering, refresh the heartbeat and set the event. On attach the writer makes any odd `seq` even and continues counters from `latest` (a crashed writer can leave a slot odd, and the DLL keeps the section alive across producer restarts).

**Reader (wants frame `c`):** read `latest = L`. If `L < c`, nothing new. If `L - c >= 4`, the wanted frame was overwritten: jump to `c = L` and count a drop. Otherwise read slot `c % 4`; require `seq` even and `counter == c` before and after the copy; if not, retry once, then treat it as overwritten and jump to `L`. A half-written frame is never returned.

## 5. The relay (pure logic)

Inputs: a request token arrives; the ring's `latest` changes; time passes. Outputs: "deliver frame `c` to token `t`" or "deliver the slate to token `t`".

Rules:
1. **Requests wait.** A request is parked in a first-in-first-out line. The relay never answers it inside the call that created it.
2. **One frame, one request, in order.** When a new frame exists and a request is waiting, the oldest request gets the oldest undelivered frame. A frame is never given to two requests and never skipped while a request could take it.
3. **Bounded waiting.** At most 8 requests wait; a request beyond that is answered at once with the newest frame.
4. **Bounded lag.** If undelivered frames pile up (more than the line can take), jump to the newest and count the drop.
5. **Slate.** If no new frame has arrived for 250 ms and requests are waiting, answer the oldest with the slate once every 33 ms (30 fps) until frames resume.
6. **Timestamps.** A delivered frame carries its capture time converted to Media Foundation 100 ns units, `qpc * 10^7 / frequency`, computed without overflow; a slate carries the QPC time at delivery. Duration is 333,333 (30 fps).
7. **Shutdown.** Stopping the stream releases all waiting requests without delivering.

Variants B and C (only if A misses the target) change rules 1 to 2: B answers on a 30 fps timer with the newest frame; C adds a 2 to 3 frame buffer and answers with the frame nearest each tick's target time (about 70 ms of added delay).

## 6. Media source integration

- `RequestSample(token)` hands the token to the relay and returns immediately (Gate 0 showed that blocking in this call breaks delivery).
- A worker thread waits on the frame-ready event (timeout 33 ms; a 2 ms poll if the event cannot be opened), reads the ring, creates samples from the sample allocator, converts BGRA to NV12 when the negotiated type is NV12 (the cost is to be measured), sets the sample time and duration, and queues `MEMediaSample` with the token. The event queue and allocator are used from the worker under the stream's lock discipline.
- Offered types: 1280x720 at 30 fps in NV12 and RGB32. Other sizes are left to the Frame Server's scaler, which Gate 0 showed it applies; every app in the matrix must show a correct picture. (4:3 sizes were cropped in Gate 0; record how each app behaves.)
- Start, Stop, Pause and Shutdown stop and join the worker and release the relay.

## 7. Pacing experiment and decision rule

First task of the plan, on hardware, before the DLL is finished. Variant A is built into the Gate 0-style DLL with a Rust test producer that publishes a steady 30 fps test pattern with the counter strip (the producer logs its own publish-interval statistics so DLL behaviour is separated from producer jitter). Measure with the browser probe, tab visible, 10 minutes, with Teams also holding the camera.

**Pass:** counter gaps at most 1% and repeats at most 1% of frames, max gap at most 0.5 s, at least 29 fps, no freezes. **Fail:** go to variant B, then C, with the same test. **None pass:** stop and ask the owner.

## 8. Error handling

| Situation | Behaviour |
|---|---|
| App gone, hung, or sends a wrong-size or invalid header | Slate within 250 ms; picture within about 0.3 s of the app returning. |
| Frame overwritten while being read | Skip to the next complete frame; never a partial frame. |
| Wake-up event cannot be opened | 2 ms polling. |
| Camera stopped or shut down | Worker stops and joins; waiting requests released. |
| Section cannot be created | The camera shows the slate and retries every 500 ms. |

## 9. Security

The section carries the user's video, so it must not be world-accessible. The DLL creates the section and event with a DACL that allows only SYSTEM, the Frame Server's own account, and the logged-in console user. It resolves that user from the active console session (`WTSGetActiveConsoleSessionId`, then `WTSQuerySessionInformation` for the user and domain names, then `LookupAccountName`) from inside the service; the plan starts with a short test that this works as the Frame Server account. If it does not, the fallback is the INTERACTIVE group, and the owner is told. Acceptance reads the DACL back and asserts it contains no Everyone, Users or Authenticated Users entry.

## 10. Testing

- **Protocol (Rust and C++):** a stress test with a writer thread at high rate and a reader that checks every frame it reads is whole (each frame's rows all carry the same counter); wrap-around; overwritten-slot detection; header validation; writer repair of an odd `seq`.
- **Relay simulation:** a fake clock replays Windows's measured pattern (about 45 requests a second against a 30 fps producer with ±3 ms jitter). Required: at least 99.9% of frames delivered exactly once and in order, waiting line never above 8, slate after 250 ms of silence, recovery on resume, clean shutdown. A mutation check (as in `looper-core`) proves the tests fail when the rules are broken.
- **Contract:** the Rust writer and the C++ reader across a real file mapping.
- **Hardware:** the pacing experiment; a 30-minute soak; the compatibility matrix (Teams, Chrome, Edge, Discord, Zoom) with the producer killed and restarted in an app; client capacity (how many apps can hold the camera, and whether a fourth is refused); restart and recovery.

## 11. Acceptance criteria

1. Pacing: at most 1% skipped and at most 1% repeated over 10 minutes with the tab visible; no flicker (slate only when the producer is silent for 250 ms or more).
2. The 30-minute soak shows no freeze, black or slate frames while the producer runs.
3. Teams, Chrome, Edge, Discord (desktop app) and Zoom all show the picture, at the same time where the capacity allows; the capacity result is recorded.
4. Producer killed and restarted: slate then picture, with no re-selecting, in at least Teams and Zoom.
5. The DACL read-back shows access only for SYSTEM, the Frame Server account and the console user (or the documented fallback).
6. Install, upgrade (new file name, repoint, restart services) and removal leave the machine clean, verified by script.
7. All new tests pass and `check` scripts are green.

## 12. Out of scope

Webcam capture, JPEG codec, hotkeys and UI (Plans 3b, 3c), licensing and the website (Plan 4), Windows 10, DirectShow fallback, Store distribution, NV12 in shared memory (the header has a pixel-format field for it later).

## 13. Risks and open items

- The Frame Server may not tolerate requests answered later than they are made (the experiment tests exactly this).
- The console-user lookup may not work from inside the Frame Server (fallback defined above).
- A fourth simultaneous client was refused in Gate 0 for an unknown reason; this plan measures capacity but may not be able to change it.
- BGRA to NV12 per frame costs CPU in the DLL; if it is too slow, the protocol's pixel-format field allows the producer to write NV12 instead.
- Zoom's behaviour is unknown.
