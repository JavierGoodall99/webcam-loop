# Webcam Loop (working title) — Design

Status: draft for review. Date: 2026-10-14. Product name and domain are not decided; "Webcam Loop" is a placeholder.

## 1. Summary

A paid Windows desktop app that lets a user record a few seconds of their webcam and play it back as their camera feed in video-call apps, then dissolve back to live video. It is sold as a separate product with its own brand and a web layer for accounts, subscriptions and downloads.

- **Customer:** remote workers on video calls (Zoom, Teams, Meet, Discord).
- **Platform:** Windows 11 only for v1.
- **Business model:** free trial, then monthly or yearly subscription, billed through Lemon Squeezy.
- **Privacy position:** webcam frames are processed locally and are not uploaded. The app's network traffic is intended to be limited to authentication and licensing (exact data listed in section 6). Public privacy claims stay qualified until network behavior has been verified.

## 2. Provenance and clean-room rule

This product was inspired by the public project `haxybaxy/video-pedal`. That repository has no license file. This spec does not offer a legal opinion on what that implies; as a conservative project policy, the code is treated as not available for reuse, and this project keeps its implementation fully independent and documents its provenance.

Rules for this project:

1. No code, tests, comments, identifiers or structure are copied from `video-pedal`. Its source files (`loop_pedal.py`, `tests/`) are not read by whoever implements this.
2. The only input taken from it is the product idea (a loop pedal for the webcam), as described publicly in its README.
3. Dependencies are chosen for license compatibility with a closed-source paid product (see section 9). The OBS virtual camera and `pyvirtualcam` are not used.
4. Optionally, the author may be contacted separately about a license or partnership. That does not block this design.

## 3. Scope

**In v1:** record, loop, return to live; crossfaded loop seam and return dissolve; global hotkeys and a tray menu; virtual camera output; preview window; accounts, trial, subscription, entitlement check; signed installer download.

**Not in v1:** macOS, Windows 10, effects or filters, multiple loop slots, saving or sharing loops, cloud storage of any video, a free tier, mobile apps, Lava.top billing, device-based trial protection, Microsoft Store listing.

## 4. Architecture

Three independent pieces in separate repos, with no shared code with Cloakly.

1. **Desktop app** (Tauri 2: Rust core, React UI).
2. **Virtual camera DLL** (native COM media source loaded by Windows).
3. **Web app** (Next.js and Supabase) with Lemon Squeezy billing.

### 4.1 Desktop app (Rust core)

| Unit | Job | Depends on |
|---|---|---|
| `capture` | Open the physical webcam and emit frames. Excludes our own virtual camera when choosing a device. | Media Foundation |
| `looper` | State machine `LIVE → RECORDING → LOOP → RETURNING → LIVE`. Owns the frame ring buffer, builds the crossfaded seam, tracks the dissolve back to live. Pure logic, no I/O. | none |
| `compositor` | Takes live and loop frames plus a blend amount and returns the output frame. | none |
| `hotkeys` | Global record and go-live keys, with a tray menu fallback. | Win32 |
| `vcam_bridge` | Writes output frames to shared memory for the DLL; registers and unregisters the virtual camera. | camera DLL |
| `license` | Holds the entitlement (trial, active, expired) and gates looping. | web API |
| React UI | Setup, preview, hotkey settings, account status. | Tauri commands |

State-machine behavior to specify in detail in the plan: holds shorter than a minimum are ignored; recording while looping replaces the loop; go-live during recording cancels it; go-live during a dissolve is a no-op; recording is capped at a maximum duration and resolution-dependent memory budget.

### 4.2 Data flow

`capture → looper → compositor → vcam_bridge → shared memory → camera DLL → call app`. The preview window receives the same frames as the virtual camera.

### 4.3 Virtual camera DLL

A Media Foundation `IMFMediaSource` registered through `MFCreateVirtualCamera`, running under the Windows Frame Server. It reads frames from shared memory and does nothing else. Language (Rust with `windows-rs`, or C++ starting from Microsoft's sample) is decided after the spike.

### 4.4 Error handling

- Webcam busy or unplugged: output shows a "camera unavailable" slate and the app retries.
- App crashes or exits: the DLL shows a static slate, never a frozen frame.
- License expired: the camera passes live video through unchanged; only looping is disabled. A lapsed subscription must never black out a meeting.
- Memory: the ring buffer is capped; recording stops at the cap.

### 4.5 Testing

- `looper` and `compositor` get thorough unit tests (state transitions, seam blend, edge cases above), written without any camera.
- `vcam_bridge` and the DLL are covered by a manual compatibility checklist.

## 5. Web app

New repo started from the Cloakly web app's structure, then re-skinned.

- Static marketing pages: home, pricing, FAQ, privacy. They must stay statically rendered; do not read `headers()` or `cookies()` in the root layout.
- Supabase auth, an account page, a download button for the signed installer.
- Lemon Squeezy checkout: free trial, then monthly and yearly plans. Webhook handler verifies the signature with `crypto.timingSafeEqual`, reserves the event in a table with a unique `(provider, event_id)` before processing, releases the reservation on failure, and has integration tests for signature rejection, replay and status transitions.
- An `entitlements` table (user, status, current period end) written only by the webhook. Tables used only by the service role have RLS enabled with no policies and a `COMMENT ON TABLE` saying so.
- Database migrations are written as files and are not applied to any hosted project without a separate explicit go-ahead.

## 6. Licensing

The app signs in through the browser and receives a signed token (status and expiry) from an entitlement endpoint. The token is cached for about 7 days for offline use. Tokens are tied to the account, not the device. Trial-abuse protection is deferred.

Data sent over the network for licensing is limited to: the sign-in exchange, the account identifier, the app version, and the entitlement request. No video, audio, frames, device lists or screenshots are sent. The plan must include a network-traffic check that confirms this.

Offline behavior:
- Cached token valid: looping works as the token states.
- Cached token expired while offline: looping is disabled until the app can reach the server and refresh. A short grace period is allowed so a brief outage does not interrupt someone mid-call.
- In every case, live camera pass-through keeps working, regardless of subscription status or connectivity.

## 7. Positioning and risk

Marketing copy focuses on convenient webcam control, smooth transitions between live and recorded video, and local processing. It avoids language about deceiving an employer or faking attendance. It also makes no claim that a recorded feed improves the user's connection or call quality; a prerecorded feed can give consistent visuals but cannot fix bandwidth or guarantee call quality. Reasons: employer and payment-processor policies, app-store review, accuracy, and reputation. The privacy statement on the site and in the privacy policy says that webcam frames stay on the device and lists exactly what is sent for authentication and licensing, and is finalized only after network behavior has been checked.

## 8. Distribution

Signed MSIX, direct download for v1. Microsoft Store submission is a later step after reviewing store policy for camera software.

## 9. Spike gate and dependencies

**Gate 0 (the first implementation task, before anything else is built).** A throwaway spike publishes a test pattern as a virtual camera through `MFCreateVirtualCamera`. Test matrix: Zoom, Microsoft Teams, Google Meet in Chrome, Google Meet in Edge, and Discord.

The gate passes only if, in every app in the matrix:
1. The camera is discovered reliably, including after the app's first launch, after restarting the call app, and after restarting Windows.
2. It streams continuously for at least 30 minutes with stable frames (no freezes, flicker or dropped-to-black periods beyond a stated threshold, measured and recorded).
3. It survives restarting the producer app and recovers cleanly if the producer is killed (slate shown, then normal video when it returns).
4. It works at the resolutions and frame rate the product needs (at least 720p at 30 fps).

The spike also records, in a short report:
- Windows versions and builds tested.
- Installation requirements, and any permissions or elevation needed to register and unregister the camera.
- Whether the registration model works with the intended MSIX packaging.
- Any app in the matrix that fails, with the reason.

**Decision rule.** If Zoom or Teams fail, the DirectShow filter fallback gets its own spike with the same criteria, and this spec is revised before any other work starts. Spike code is labeled throwaway and is not carried into the product.

**Dependency licenses.** Every dependency, and any Microsoft sample used as a reference, must be checked for compatibility with closed-source commercial distribution before it is adopted. The result and provenance of each (name, version, license, where it came from) are recorded in a dependency log in the repo. GPL components are not used.

## 9a. Requirements the implementation plan must cover

- **Installer and camera lifecycle:** registration of the virtual camera at install, cleanup on uninstall, behavior across upgrades, and recovery after crashes. Confirm the chosen MSIX approach supports the registration model.
- **Live-video fallback:** how the camera DLL behaves if the Rust process crashes or shared memory becomes unavailable. A frozen frame must never be presented as live video; the DLL shows a clearly static slate or passes through the real camera. Also test the physical camera being disconnected and being claimed by another application.
- **Licensing:** the cached-token and offline-expiry behavior in section 6.
- **Privacy:** the network-traffic verification in section 6, and the qualified privacy wording in sections 1 and 7.
- **Commercial dependencies:** the dependency log above.
- **Order of work after Gate 0 passes:** `looper` and `compositor` (with tests), then the desktop app, then licensing and the web layer.

## 10. Open items

- Product name, domain and brand.
- Pricing and trial length.
- Camera DLL language (after the spike).
- Webcam capture approach in Rust (direct Media Foundation vs. a crate), to be settled in the plan.
- Whether to contact the author of `video-pedal` about a license or partnership.
