# looper-core Implementation Plan (Plan 2 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `looper-core`, a dependency-free Rust library holding the webcam loop's record / loop / return state machine and its frame blending, fully tested without a camera.

**Architecture:** A frame-in, frame-out library. `Looper::process(live) -> Output` is called once per camera frame and returns what the virtual camera should show. There is no clock and no I/O: playback advances one frame per call, so behaviour is deterministic. Recordings are stored through a pluggable `Codec` so the product can use a compressing codec; tests use a pass-through `RawCodec`.

**Tech Stack:** Rust (edition 2021, verified on 1.91.1), no dependencies, `cargo test` / `clippy` / `rustfmt`, plus one small Python tool for mutation checking.

**Spec:** `docs/superpowers/specs/2026-10-14-webcam-loop-design.md` (sections 4.1 `looper` and `compositor`, 4.4 memory cap, 4.5 testing, and section 11 findings). Read it first.

**How this plan was prepared:** every code block below is copied from a crate that was written and run in a scratch folder before this plan was finished: 67 unit tests and 1 doc test pass, `cargo fmt --check` and `cargo clippy -- -D warnings` are clean, docs build with warnings as errors, and a mutation check confirms the tests catch 10 deliberate breaks of the code. The scratch copy was discarded; nothing in the repo has been created yet.

## Global Constraints

- `looper` and `compositor` are **pure logic with no I/O** and depend on nothing (spec 4.1: "Pure logic, no I/O", "Depends on: none"). No crates, no clock, no threads, no file or network access.
- Frames are **BGRA, 8 bits per channel, top-down, no row padding**, matching the shared-memory frame layout the camera DLL reads (spec section 11, item 1).
- Behaviour the spec requires (4.1): "holds shorter than a minimum are ignored; recording while looping replaces the loop; go-live during recording cancels it; go-live during a dissolve is a no-op; recording is capped at a maximum duration and resolution-dependent memory budget" (4.1) and "recording stops at the cap" (4.4).
- State machine shape (4.1): `LIVE -> RECORDING -> LOOP -> RETURNING -> LIVE`, with a crossfaded seam and a dissolve back to live.
- Tests need no camera (4.5). `looper` and `compositor` get thorough unit tests including state transitions, the seam blend and the edge cases above.
- Clean-room (spec section 2): nothing is taken from `haxybaxy/video-pedal`; its source is not read. All numeric defaults below were chosen independently.
- No GPL or other copyleft dependencies (there are no dependencies at all).
- The crate lives at `crates/looper-core/` in this repo and is standalone (no Cargo workspace), so it can later be depended on by path or moved into the desktop app's repo.
- Frame processing must stay far inside the real-time budget: 33 ms per frame at 30 fps for everything. Measured: well under 2 ms per 720p frame.

## Decisions this plan makes where the spec was silent

Please review these; each is easy to change before the code exists.

| # | Decision | Why |
|---|---|---|
| 1 | Crate `looper-core` at `crates/looper-core/`, standalone, no workspace. | A workspace at the repo root would have to exclude `spike/`; standalone avoids that and keeps the crate movable. |
| 2 | Frame type is BGRA8 with a size cap (256 MiB) and checked arithmetic. | Matches the camera DLL's frame layout; the cap turns absurd sizes into errors instead of huge allocations. |
| 3 | Blend weight is an integer `0..=256`, rounding to nearest (`(a*(256-t)+b*t+128)>>8`). | Exact at both ends and fast: 0.54 ms for a 720p frame (a division per byte would be about 20 times slower). |
| 4 | Defaults: dissolve 400 ms, minimum hold 750 ms, maximum recording 20 s, memory budget 512 MiB. All are fields of `Config`. | The spec names these limits but gives no numbers. |
| 5 | The loop is the recording with its last `k` frames dissolved into its first `k` and dropped; it **starts at its first frame** (nearly the tail of the recording). | Playback wraps with no jump, and the start is a dissolve from "what the camera just showed". |
| 6 | Return to live = `k` frames blending the playing loop into the live feed, `k` = the same dissolve length. `k = 0` means hard cuts. | Spec: "tracks the dissolve back to live". |
| 7 | Output while recording is always the live frame. Pressing record during a loop drops the old loop at once and starts recording. | Simple and predictable: the person sees themselves while recording. A later `go_live` then cancels to live. |
| 8 | A recording that hits the frame cap or the byte budget **ends by itself and starts looping** (even if shorter than the minimum). | Spec 4.4: "recording stops at the cap". The later key release is harmless. |
| 9 | A change in the live frame size (resolution switch, camera swap) while recording, looping or returning abandons the loop and shows live. | Never output a frame of the wrong size, and never crash. |
| 10 | If the codec fails, the looper returns the error **and falls back to live**, so the next call shows the camera again. | A failure must never black out a meeting. |
| 11 | Recordings are stored as encoded bytes through a `Codec` trait; `RawCodec` is for tests. | Measured: a raw 720p frame is 3.7 MB, so 20 s raw is 2.2 GB and the 512 MiB budget would stop a raw recording after about 4.6 s. The product (Plan 3) must supply a compressing codec (JPEG or similar). |
| 12 | The spec says the looper "owns the frame ring buffer"; this design uses a capped recording buffer instead of a sliding window. | Spec 4.4 says recording stops at the cap, not that old frames drop off. |

## Review Focus

Failure modes the spec implies that are most likely to bite someone using this software, most likely first. Each is pinned by tests in Task 6 (and Task 5 for the seam), and Task 7 adds a mutation check that proves the tests actually fail when the behaviour breaks.

1. **The camera resolution changes mid-session** (camera swap, resolution switch): no crash, and the output is always a frame of the live size. Tests: `a_resolution_change_while_*`, and the random-sequence test.
2. **Keys arrive in odd orders:** key repeat, release without a press, go-live pressed twice or during the dissolve. Defined no-ops, never a panic or a stuck state. Tests: `a_repeated_key_down_*`, `releasing_without_having_pressed_*`, `pressing_go_live_during_the_dissolve_*`, the random-sequence test.
3. **Tiny or boundary recordings:** one frame, two frames, exactly the minimum hold, a dissolve longer than the recording, minimum equal to maximum. Always a valid non-empty loop. Tests in Tasks 4, 5 and 6.
4. **The memory cap is hit while the key is still held:** the loop starts by itself and the late release is harmless. Tests: `hitting_the_frame_cap_*`, `hitting_the_memory_budget_*`.
5. **A frame cannot be read back** (corrupt stored data): an error is returned and the next frame shows live, not black. Tests: `a_decode_failure_*`.

Not covered here by design: real JPEG storage and its speed (Plan 3), and frame pacing against the camera clock (the open problem from Gate 0).

---

## File Structure

```
crates/looper-core/
├── Cargo.toml
├── README.md
├── check.sh                      # fmt, clippy, tests, docs (+ optional mutation check)
├── examples/bench.rs             # per-frame timing at 720p
├── tools/mutation_check.py       # breaks the code 10 ways and checks the tests notice
└── src/
    ├── lib.rs                    # module list, re-exports, doc example
    ├── frame.rs        frame/tests.rs        # BGRA frame type, sizes, errors
    ├── compositor.rs   compositor/tests.rs   # blend_into / blend
    ├── codec.rs        codec/tests.rs        # Codec trait + RawCodec
    ├── config.rs       config/tests.rs       # limits and defaults
    ├── seam.rs         seam/tests.rs         # the seamless-loop builder (pure)
    └── looper.rs       looper/tests.rs       # the state machine
```

Run every command from the repo root (`D:\Coding Projects\webcam-loop`). `cd crates/looper-core` before the `cargo` commands, or pass `--manifest-path crates/looper-core/Cargo.toml`.

---
### Task 1: Crate scaffold and the `Frame` type

**Files:**
- Create: `crates/looper-core/Cargo.toml`, `crates/looper-core/src/lib.rs`, `crates/looper-core/src/frame.rs`, `crates/looper-core/src/frame/tests.rs`

**Interfaces:**
- Produces: `Frame` (`byte_len`, `from_bgra`, `solid`, `width`, `height`, `data`, `data_mut`, `into_data`, `same_size`, `pixel`), `FrameError { ZeroSize, TooLarge, WrongLength { expected, actual } }`, constants `BYTES_PER_PIXEL = 4` and `MAX_FRAME_BYTES = 256 MiB`.

- [ ] **Step 1: Create the crate and write the failing tests**

`crates/looper-core/Cargo.toml`:

```toml
[package]
name = "looper-core"
version = "0.1.0"
edition = "2021"
publish = false
description = "Pure-logic core of the webcam loop product: frame blending and the record/loop/return state machine. No I/O, no dependencies."

[dependencies]
```

`crates/looper-core/src/lib.rs` (this file grows by a few lines in each task):

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod frame;

pub use frame::{Frame, FrameError};
```

`crates/looper-core/src/frame.rs` (only the test hook for now, so the tests fail to build):

```rust
#[cfg(test)]
mod tests;
```

`crates/looper-core/src/frame/tests.rs`:

```rust
use super::*;

#[test]
fn from_bgra_accepts_data_of_exactly_the_right_length() {
    let f = Frame::from_bgra(2, 3, vec![7; 2 * 3 * 4]).unwrap();
    assert_eq!((f.width(), f.height()), (2, 3));
    assert_eq!(f.data().len(), 24);
}

#[test]
fn from_bgra_rejects_wrong_lengths_and_says_what_was_expected() {
    assert_eq!(
        Frame::from_bgra(2, 2, vec![0; 15]),
        Err(FrameError::WrongLength {
            expected: 16,
            actual: 15
        })
    );
    assert_eq!(
        Frame::from_bgra(2, 2, vec![0; 17]),
        Err(FrameError::WrongLength {
            expected: 16,
            actual: 17
        })
    );
}

#[test]
fn zero_sized_frames_are_rejected() {
    assert_eq!(Frame::solid(0, 5, [0; 4]), Err(FrameError::ZeroSize));
    assert_eq!(Frame::solid(5, 0, [0; 4]), Err(FrameError::ZeroSize));
    assert_eq!(Frame::from_bgra(0, 0, vec![]), Err(FrameError::ZeroSize));
}

#[test]
fn absurdly_large_frames_are_rejected_without_allocating() {
    assert_eq!(Frame::byte_len(100_000, 100_000), Err(FrameError::TooLarge));
    assert_eq!(
        Frame::byte_len(u32::MAX, u32::MAX),
        Err(FrameError::TooLarge)
    );
    assert!(Frame::solid(100_000, 100_000, [0; 4]).is_err());
}

#[test]
fn a_720p_and_an_8k_frame_are_allowed() {
    assert_eq!(Frame::byte_len(1280, 720), Ok(1280 * 720 * 4));
    assert!(Frame::byte_len(7680, 4320).is_ok());
}

#[test]
fn solid_fills_every_pixel_including_odd_sizes() {
    for (w, h) in [(1, 1), (3, 5), (7, 1), (1, 9)] {
        let f = Frame::solid(w, h, [1, 2, 3, 255]).unwrap();
        assert_eq!(f.data().len(), (w * h * 4) as usize);
        assert!(f.data().chunks(4).all(|p| p == [1, 2, 3, 255]));
    }
}

#[test]
fn pixel_reads_by_column_and_row_and_is_none_outside() {
    let mut data = vec![0u8; 3 * 2 * 4];
    let at = (3 + 2) * 4; // column 2 of row 1 in a 3-pixel-wide frame
    data[at..at + 4].copy_from_slice(&[9, 8, 7, 6]);
    let f = Frame::from_bgra(3, 2, data).unwrap();
    assert_eq!(f.pixel(2, 1), Some([9, 8, 7, 6]));
    assert_eq!(f.pixel(0, 0), Some([0, 0, 0, 0]));
    assert_eq!(f.pixel(3, 0), None);
    assert_eq!(f.pixel(0, 2), None);
}

#[test]
fn same_size_compares_dimensions_not_content() {
    let a = Frame::solid(2, 2, [0; 4]).unwrap();
    let b = Frame::solid(2, 2, [255; 4]).unwrap();
    let c = Frame::solid(2, 3, [0; 4]).unwrap();
    assert!(a.same_size(&b));
    assert!(!a.same_size(&c));
}

#[test]
fn data_mut_changes_pixels_but_never_the_length() {
    let mut f = Frame::solid(2, 2, [0; 4]).unwrap();
    f.data_mut()[0] = 42;
    assert_eq!(f.pixel(0, 0), Some([42, 0, 0, 0]));
    assert_eq!(f.data().len(), 16);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd crates/looper-core && cargo test
```

Expected: the build fails: `error[E0432]: unresolved imports 'frame::Frame', 'frame::FrameError'` and `error[E0433]: failed to resolve: use of undeclared type 'Frame'`.

- [ ] **Step 3: Write the implementation** by replacing `crates/looper-core/src/frame.rs` with:

```rust
//! A video frame: tightly packed BGRA, 8 bits per channel, top-down, no row padding.

use std::fmt;

pub const BYTES_PER_PIXEL: usize = 4;

/// Largest frame accepted (an 8K BGRA frame is about 133 MB). Guards against absurd sizes and arithmetic overflow.
pub const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    ZeroSize,
    TooLarge,
    WrongLength { expected: usize, actual: usize },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::ZeroSize => write!(f, "frame width and height must be non-zero"),
            FrameError::TooLarge => write!(f, "frame is larger than {MAX_FRAME_BYTES} bytes"),
            FrameError::WrongLength { expected, actual } => {
                write!(f, "frame data is {actual} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Frame {
    /// Number of bytes a `width` x `height` frame occupies, or why that size is not allowed.
    pub fn byte_len(width: u32, height: u32) -> Result<usize, FrameError> {
        if width == 0 || height == 0 {
            return Err(FrameError::ZeroSize);
        }
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or(FrameError::TooLarge)?;
        if bytes > MAX_FRAME_BYTES {
            return Err(FrameError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn from_bgra(width: u32, height: u32, data: Vec<u8>) -> Result<Self, FrameError> {
        let expected = Self::byte_len(width, height)?;
        if data.len() != expected {
            return Err(FrameError::WrongLength {
                expected,
                actual: data.len(),
            });
        }
        Ok(Frame {
            width,
            height,
            data,
        })
    }

    /// A frame where every pixel is `bgra`.
    pub fn solid(width: u32, height: u32, bgra: [u8; 4]) -> Result<Self, FrameError> {
        let bytes = Self::byte_len(width, height)?;
        let mut data = Vec::with_capacity(bytes);
        for _ in 0..bytes / BYTES_PER_PIXEL {
            data.extend_from_slice(&bgra);
        }
        Ok(Frame {
            width,
            height,
            data,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable access to the pixels. The length cannot change, so the size invariant holds.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn same_size(&self, other: &Frame) -> bool {
        self.width == other.width && self.height == other.height
    }

    /// The BGRA value at column `x`, row `y`, or `None` outside the frame.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * BYTES_PER_PIXEL;
        Some([
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ])
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test
```

Expected: `test result: ok. 9 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): crate scaffold and BGRA Frame type"
```

---
### Task 2: The compositor

**Files:**
- Create: `crates/looper-core/src/compositor.rs`, `crates/looper-core/src/compositor/tests.rs`
- Modify: `crates/looper-core/src/lib.rs`

**Interfaces:**
- Consumes: `Frame` (Task 1).
- Produces: `ALPHA_ONE: u16 = 256`; `blend_into(dst: &mut Frame, a: &Frame, b: &Frame, alpha: u16) -> Result<(), BlendError>` (writes `a*(1-alpha) + b*alpha`, exact at 0 and 256); `blend(a, b, alpha) -> Result<Frame, BlendError>`; `BlendError { SizeMismatch, AlphaOutOfRange }`.

- [ ] **Step 1: Write the failing tests.** Replace `crates/looper-core/src/lib.rs` with:

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod compositor;
pub mod frame;

pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use frame::{Frame, FrameError};
```

Create `crates/looper-core/src/compositor.rs` containing only the test hook:

```rust
#[cfg(test)]
mod tests;
```

`crates/looper-core/src/compositor/tests.rs`:

```rust
use super::*;

fn px(v: u8) -> Frame {
    Frame::solid(2, 2, [v, v, v, 255]).unwrap()
}

#[test]
fn alpha_zero_gives_exactly_the_first_frame() {
    let (a, b) = (px(10), px(200));
    assert_eq!(blend(&a, &b, 0).unwrap(), a);
}

#[test]
fn alpha_one_gives_exactly_the_second_frame() {
    let (a, b) = (px(10), px(200));
    assert_eq!(blend(&a, &b, ALPHA_ONE).unwrap(), b);
}

#[test]
fn the_midpoint_of_black_and_white_is_mid_grey() {
    let out = blend(&px(0), &px(255), 128).unwrap();
    assert_eq!(out.pixel(0, 0), Some([128, 128, 128, 255]));
}

#[test]
fn blending_identical_frames_changes_nothing_at_any_alpha() {
    let a = px(77);
    for alpha in 0..=ALPHA_ONE {
        assert_eq!(blend(&a, &a, alpha).unwrap(), a, "alpha {alpha}");
    }
}

#[test]
fn every_channel_is_blended_independently() {
    let a = Frame::solid(1, 1, [0, 100, 200, 255]).unwrap();
    let b = Frame::solid(1, 1, [255, 100, 0, 255]).unwrap();
    let out = blend(&a, &b, 128).unwrap();
    assert_eq!(out.pixel(0, 0), Some([128, 100, 100, 255]));
}

#[test]
fn the_result_never_overflows_a_byte() {
    let out = blend(&px(255), &px(255), 255).unwrap();
    assert_eq!(out.pixel(0, 0), Some([255, 255, 255, 255]));
}

#[test]
fn increasing_alpha_never_moves_a_channel_backwards() {
    let (a, b) = (px(10), px(250));
    let mut last = 0u8;
    for alpha in 0..=ALPHA_ONE {
        let v = blend(&a, &b, alpha).unwrap().pixel(0, 0).unwrap()[0];
        assert!(v >= last, "alpha {alpha}: {v} < {last}");
        last = v;
    }
}

#[test]
fn frames_of_different_sizes_cannot_be_blended() {
    let a = Frame::solid(2, 2, [0; 4]).unwrap();
    let b = Frame::solid(2, 3, [0; 4]).unwrap();
    assert_eq!(blend(&a, &b, 100), Err(BlendError::SizeMismatch));
    let mut dst = Frame::solid(3, 3, [0; 4]).unwrap();
    assert_eq!(
        blend_into(&mut dst, &a, &a, 100),
        Err(BlendError::SizeMismatch)
    );
}

#[test]
fn an_alpha_above_the_scale_is_rejected() {
    let a = px(1);
    assert_eq!(
        blend(&a, &a, ALPHA_ONE + 1),
        Err(BlendError::AlphaOutOfRange)
    );
}

#[test]
fn blend_into_overwrites_the_destination_completely() {
    let mut dst = px(99);
    blend_into(&mut dst, &px(10), &px(20), 0).unwrap();
    assert_eq!(dst, px(10));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd crates/looper-core && cargo test compositor
```

Expected: the build fails with unresolved imports of `blend`, `blend_into`, `BlendError`, `ALPHA_ONE`.

- [ ] **Step 3: Write the implementation** by replacing `crates/looper-core/src/compositor.rs` with:

```rust
//! Blends two frames. This is the only place pixel maths happens.

use crate::frame::Frame;
use std::fmt;

/// Blend weight scale: `0` is entirely the first frame, `ALPHA_ONE` entirely the second.
pub const ALPHA_ONE: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendError {
    SizeMismatch,
    AlphaOutOfRange,
}

impl fmt::Display for BlendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlendError::SizeMismatch => write!(f, "frames to blend have different sizes"),
            BlendError::AlphaOutOfRange => {
                write!(f, "blend alpha must be between 0 and {ALPHA_ONE}")
            }
        }
    }
}

impl std::error::Error for BlendError {}

/// Writes `a * (1 - alpha) + b * alpha` into `dst`, per byte, rounding to nearest.
/// `alpha` is in `0..=ALPHA_ONE`; `0` gives exactly `a` and `ALPHA_ONE` exactly `b`.
pub fn blend_into(dst: &mut Frame, a: &Frame, b: &Frame, alpha: u16) -> Result<(), BlendError> {
    if alpha > ALPHA_ONE {
        return Err(BlendError::AlphaOutOfRange);
    }
    if !dst.same_size(a) || !dst.same_size(b) {
        return Err(BlendError::SizeMismatch);
    }
    let wb = alpha as u32;
    let wa = ALPHA_ONE as u32 - wb;
    for ((d, &pa), &pb) in dst.data_mut().iter_mut().zip(a.data()).zip(b.data()) {
        *d = ((pa as u32 * wa + pb as u32 * wb + 128) >> 8) as u8;
    }
    Ok(())
}

/// Like [`blend_into`] but allocates the result.
pub fn blend(a: &Frame, b: &Frame, alpha: u16) -> Result<Frame, BlendError> {
    let mut out = a.clone();
    blend_into(&mut out, a, b, alpha)?;
    Ok(out)
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test
```

Expected: `test result: ok. 19 passed; 0 failed` (9 frame + 10 compositor).

- [ ] **Step 5: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): compositor with exact integer blending"
```

---
### Task 3: The `Codec` trait

**Files:**
- Create: `crates/looper-core/src/codec.rs`, `crates/looper-core/src/codec/tests.rs`
- Modify: `crates/looper-core/src/lib.rs`

**Interfaces:**
- Consumes: `Frame` (Task 1).
- Produces: `trait Codec { fn encode(&self, &Frame) -> Vec<u8>; fn decode(&self, &[u8], width: u32, height: u32) -> Result<Frame, CodecError>; }`, `CodecError::Corrupt`, `RawCodec` (stores frames unchanged).

- [ ] **Step 1: Write the failing tests.** Replace `crates/looper-core/src/lib.rs` with:

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod codec;
pub mod compositor;
pub mod frame;

pub use codec::{Codec, CodecError, RawCodec};
pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use frame::{Frame, FrameError};
```

Create `crates/looper-core/src/codec.rs` containing only the test hook:

```rust
#[cfg(test)]
mod tests;
```

`crates/looper-core/src/codec/tests.rs`:

```rust
use super::*;

#[test]
fn raw_codec_round_trips_a_frame_exactly() {
    let f = Frame::from_bgra(3, 2, (0..24).collect()).unwrap();
    let bytes = RawCodec.encode(&f);
    assert_eq!(RawCodec.decode(&bytes, 3, 2).unwrap(), f);
}

#[test]
fn decoding_bytes_of_the_wrong_length_is_corrupt() {
    assert_eq!(RawCodec.decode(&[0; 23], 3, 2), Err(CodecError::Corrupt));
    assert_eq!(RawCodec.decode(&[0; 25], 3, 2), Err(CodecError::Corrupt));
    assert_eq!(RawCodec.decode(&[], 3, 2), Err(CodecError::Corrupt));
}

#[test]
fn decoding_into_a_zero_sized_frame_is_corrupt_not_a_panic() {
    assert_eq!(RawCodec.decode(&[], 0, 0), Err(CodecError::Corrupt));
}

#[test]
fn encoded_size_is_the_pixel_byte_count() {
    let f = Frame::solid(4, 4, [1, 2, 3, 4]).unwrap();
    assert_eq!(RawCodec.encode(&f).len(), 64);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd crates/looper-core && cargo test codec
```

Expected: the build fails with unresolved imports of `Codec`, `CodecError`, `RawCodec`.

- [ ] **Step 3: Write the implementation** by replacing `crates/looper-core/src/codec.rs` with:

```rust
//! How recorded frames are stored. The looper keeps recordings as encoded bytes so the memory budget can be met;
//! the product will plug in JPEG (or similar). Tests use [`RawCodec`], which stores frames unchanged.

use crate::frame::Frame;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// The stored bytes could not be turned back into a frame of the expected size.
    Corrupt,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::Corrupt => write!(f, "stored frame data is corrupt"),
        }
    }
}

impl std::error::Error for CodecError {}

pub trait Codec {
    fn encode(&self, frame: &Frame) -> Vec<u8>;
    fn decode(&self, bytes: &[u8], width: u32, height: u32) -> Result<Frame, CodecError>;
}

/// Stores frames as they are: the pixel bytes, nothing more.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawCodec;

impl Codec for RawCodec {
    fn encode(&self, frame: &Frame) -> Vec<u8> {
        frame.data().to_vec()
    }

    fn decode(&self, bytes: &[u8], width: u32, height: u32) -> Result<Frame, CodecError> {
        Frame::from_bgra(width, height, bytes.to_vec()).map_err(|_| CodecError::Corrupt)
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test
```

Expected: `test result: ok. 23 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): Codec trait and RawCodec"
```

---
### Task 4: `Config`

**Files:**
- Create: `crates/looper-core/src/config.rs`, `crates/looper-core/src/config/tests.rs`
- Modify: `crates/looper-core/src/lib.rs`

**Interfaces:**
- Produces: `Config { fps, crossfade_frames, min_record_frames, max_record_frames, max_record_bytes }` (all `pub`, `Copy`); `Config::for_fps(fps) -> Result<Config, ConfigError>` (defaults in frames: dissolve 400 ms, minimum 750 ms, maximum 20 s, 512 MiB); `Config::validate(&self) -> Result<(), ConfigError>`; `ConfigError { ZeroFps, FpsTooHigh, ZeroMinRecord, MinAboveMax, ZeroMaxBytes }`.

- [ ] **Step 1: Write the failing tests.** Replace `crates/looper-core/src/lib.rs` with:

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod codec;
pub mod compositor;
pub mod config;
pub mod frame;

pub use codec::{Codec, CodecError, RawCodec};
pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use config::{Config, ConfigError};
pub use frame::{Frame, FrameError};
```

Create `crates/looper-core/src/config.rs` containing only the test hook:

```rust
#[cfg(test)]
mod tests;
```

`crates/looper-core/src/config/tests.rs`:

```rust
use super::*;

#[test]
fn the_defaults_at_30_fps_are_in_frames() {
    let c = Config::for_fps(30).unwrap();
    assert_eq!(c.fps, 30);
    assert_eq!(c.crossfade_frames, 12); // 400 ms
    assert_eq!(c.min_record_frames, 23); // 750 ms, rounded to nearest
    assert_eq!(c.max_record_frames, 600); // 20 s
    assert_eq!(c.max_record_bytes, 512 * 1024 * 1024);
}

#[test]
fn the_defaults_scale_with_the_frame_rate() {
    let c = Config::for_fps(60).unwrap();
    assert_eq!(
        (c.crossfade_frames, c.min_record_frames, c.max_record_frames),
        (24, 45, 1200)
    );
}

#[test]
fn a_frame_rate_of_zero_or_above_the_limit_is_rejected() {
    assert_eq!(Config::for_fps(0), Err(ConfigError::ZeroFps));
    assert_eq!(Config::for_fps(241), Err(ConfigError::FpsTooHigh));
    assert!(Config::for_fps(240).is_ok());
    assert!(Config::for_fps(1).is_ok());
}

#[test]
fn even_a_one_fps_default_has_a_valid_minimum_hold() {
    let c = Config::for_fps(1).unwrap();
    assert!(c.min_record_frames >= 1 && c.min_record_frames <= c.max_record_frames);
}

fn valid() -> Config {
    Config {
        fps: 30,
        crossfade_frames: 5,
        min_record_frames: 10,
        max_record_frames: 100,
        max_record_bytes: 1000,
    }
}

#[test]
fn validate_accepts_a_sane_config_including_no_crossfade() {
    assert_eq!(valid().validate(), Ok(()));
    assert_eq!(
        Config {
            crossfade_frames: 0,
            ..valid()
        }
        .validate(),
        Ok(())
    );
    assert_eq!(
        Config {
            min_record_frames: 100,
            ..valid()
        }
        .validate(),
        Ok(())
    ); // min == max is allowed
}

#[test]
fn validate_rejects_each_kind_of_nonsense() {
    assert_eq!(
        Config { fps: 0, ..valid() }.validate(),
        Err(ConfigError::ZeroFps)
    );
    assert_eq!(
        Config {
            min_record_frames: 0,
            ..valid()
        }
        .validate(),
        Err(ConfigError::ZeroMinRecord)
    );
    assert_eq!(
        Config {
            min_record_frames: 101,
            ..valid()
        }
        .validate(),
        Err(ConfigError::MinAboveMax)
    );
    assert_eq!(
        Config {
            max_record_bytes: 0,
            ..valid()
        }
        .validate(),
        Err(ConfigError::ZeroMaxBytes)
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd crates/looper-core && cargo test config
```

Expected: the build fails with unresolved imports of `Config`, `ConfigError`.

- [ ] **Step 3: Write the implementation** by replacing `crates/looper-core/src/config.rs` with:

```rust
//! Tunable limits for the looper. Every number here is a default that the product may change.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Output frame rate; only used to turn the durations below into frame counts.
    pub fps: u32,
    /// Length of the dissolve at the loop seam and when returning to live. 0 means hard cuts.
    pub crossfade_frames: u32,
    /// Holds shorter than this are ignored (too short to loop).
    pub min_record_frames: u32,
    /// Recording stops, and the loop starts, at this many frames.
    pub max_record_frames: u32,
    /// Recording also stops when the stored data reaches this many bytes (resolution-dependent memory budget).
    pub max_record_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ZeroFps,
    FpsTooHigh,
    ZeroMinRecord,
    MinAboveMax,
    ZeroMaxBytes,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ConfigError::ZeroFps => "fps must be at least 1",
            ConfigError::FpsTooHigh => "fps must be at most 240",
            ConfigError::ZeroMinRecord => "the minimum recording length must be at least 1 frame",
            ConfigError::MinAboveMax => "the minimum recording length is longer than the maximum",
            ConfigError::ZeroMaxBytes => "the recording memory budget must be above zero",
        };
        write!(f, "{msg}")
    }
}

impl std::error::Error for ConfigError {}

fn ms_to_frames(fps: u32, ms: u32) -> u32 {
    ((fps as u64 * ms as u64 + 500) / 1000) as u32
}

impl Config {
    pub const MAX_FPS: u32 = 240;
    pub const DEFAULT_CROSSFADE_MS: u32 = 400;
    pub const DEFAULT_MIN_RECORD_MS: u32 = 750;
    pub const DEFAULT_MAX_RECORD_SECONDS: u32 = 20;
    pub const DEFAULT_MAX_RECORD_BYTES: usize = 512 * 1024 * 1024;

    /// The defaults, expressed in frames at `fps`.
    pub fn for_fps(fps: u32) -> Result<Config, ConfigError> {
        if fps == 0 {
            return Err(ConfigError::ZeroFps);
        }
        if fps > Self::MAX_FPS {
            return Err(ConfigError::FpsTooHigh);
        }
        let config = Config {
            fps,
            crossfade_frames: ms_to_frames(fps, Self::DEFAULT_CROSSFADE_MS),
            min_record_frames: ms_to_frames(fps, Self::DEFAULT_MIN_RECORD_MS).max(1),
            max_record_frames: fps * Self::DEFAULT_MAX_RECORD_SECONDS,
            max_record_bytes: Self::DEFAULT_MAX_RECORD_BYTES,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.fps == 0 {
            return Err(ConfigError::ZeroFps);
        }
        if self.fps > Self::MAX_FPS {
            return Err(ConfigError::FpsTooHigh);
        }
        if self.min_record_frames == 0 {
            return Err(ConfigError::ZeroMinRecord);
        }
        if self.min_record_frames > self.max_record_frames {
            return Err(ConfigError::MinAboveMax);
        }
        if self.max_record_bytes == 0 {
            return Err(ConfigError::ZeroMaxBytes);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test
```

Expected: `test result: ok. 29 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): Config with validated defaults"
```

---
### Task 5: The seamless loop (`seam`)

**Files:**
- Create: `crates/looper-core/src/seam.rs`, `crates/looper-core/src/seam/tests.rs`
- Modify: `crates/looper-core/src/lib.rs`

**Interfaces:**
- Consumes: `Frame` (Task 1), `blend` and `ALPHA_ONE` (Task 2).
- Produces: `crossfade_len(frame_count: usize, requested: u32) -> usize` (the request, but at most half the recording); `seam_alpha(i: usize, k: usize) -> u16` (strictly increasing, never 0 or 256); `build_loop(frames: &[Frame], requested: u32) -> Result<Vec<Frame>, SeamError>`; `SeamError { Empty, SizeMismatch }`. `build_loop` is the reference implementation that `Looper` (Task 6) is tested against.

- [ ] **Step 1: Write the failing tests.** Replace `crates/looper-core/src/lib.rs` with:

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod codec;
pub mod compositor;
pub mod config;
pub mod frame;
pub mod seam;

pub use codec::{Codec, CodecError, RawCodec};
pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use config::{Config, ConfigError};
pub use frame::{Frame, FrameError};
```

Create `crates/looper-core/src/seam.rs` containing only the test hook:

```rust
#[cfg(test)]
mod tests;
```

`crates/looper-core/src/seam/tests.rs`:

```rust
use super::*;

fn px(v: u8) -> Frame {
    Frame::solid(2, 2, [v, v, v, 255]).unwrap()
}

fn recording(values: &[u8]) -> Vec<Frame> {
    values.iter().map(|&v| px(v)).collect()
}

fn blue(f: &Frame) -> u8 {
    f.pixel(0, 0).unwrap()[0]
}

#[test]
fn a_ten_frame_recording_with_a_two_frame_dissolve_makes_an_eight_frame_loop() {
    let out = build_loop(&recording(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 2).unwrap();
    assert_eq!(out.len(), 8);
}

#[test]
fn the_first_loop_frames_are_the_tail_dissolving_into_the_head() {
    // alpha(0,2) = 85, alpha(1,2) = 170.
    // (90 * 171 + 10 * 85 + 128) >> 8 = 63   and   (100 * 86 + 20 * 170 + 128) >> 8 = 47
    let out = build_loop(&recording(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 2).unwrap();
    assert_eq!(blue(&out[0]), 63);
    assert_eq!(blue(&out[1]), 47);
}

#[test]
fn the_rest_of_the_loop_is_the_recording_unchanged() {
    let out = build_loop(&recording(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 2).unwrap();
    let rest: Vec<u8> = out[2..].iter().map(blue).collect();
    assert_eq!(rest, vec![30, 40, 50, 60, 70, 80]);
}

#[test]
fn the_dissolve_ramp_is_strictly_increasing_and_never_reaches_either_end() {
    for k in 1..40usize {
        let alphas: Vec<u16> = (0..k).map(|i| seam_alpha(i, k)).collect();
        assert!(alphas[0] > 0, "k={k}");
        assert!(*alphas.last().unwrap() < ALPHA_ONE, "k={k}");
        assert!(alphas.windows(2).all(|w| w[0] < w[1]), "k={k}: {alphas:?}");
    }
}

#[test]
fn a_dissolve_longer_than_half_the_recording_is_shortened() {
    // 5 frames, 10 requested: only 2 can be used, so the loop is 3 frames long.
    assert_eq!(crossfade_len(5, 10), 2);
    assert_eq!(
        build_loop(&recording(&[1, 2, 3, 4, 5]), 10).unwrap().len(),
        3
    );
}

#[test]
fn no_dissolve_means_the_loop_is_the_recording() {
    let rec = recording(&[5, 6, 7]);
    assert_eq!(build_loop(&rec, 0).unwrap(), rec);
}

#[test]
fn recordings_of_one_or_two_frames_still_make_a_valid_loop() {
    assert_eq!(build_loop(&recording(&[9]), 12).unwrap(), recording(&[9]));
    let two = build_loop(&recording(&[0, 200]), 12).unwrap(); // k = 1, loop length 1
    assert_eq!(two.len(), 1);
    assert_eq!(blue(&two[0]), 100); // halfway between the tail (200) and the head (0)
}

#[test]
fn the_loop_is_never_empty_and_never_longer_than_the_recording() {
    for n in 1..30usize {
        for requested in [0u32, 1, 2, 5, 12, 100] {
            let rec: Vec<Frame> = (0..n).map(|i| px(i as u8)).collect();
            let out = build_loop(&rec, requested).unwrap();
            assert!(
                !out.is_empty() && out.len() <= n,
                "n={n} requested={requested}"
            );
        }
    }
}

#[test]
fn the_wrap_around_flows_from_the_last_loop_frame_into_a_frame_close_to_the_tail() {
    // Recording is a ramp: frame i has value 10 * i. The loop's last frame is recording frame n-k-1 and the loop's
    // first frame is the tail dissolved into the head, so the step across the wrap is small (about one frame).
    let values: Vec<u8> = (0..12).map(|i| i * 10).collect();
    let out = build_loop(&recording(&values), 3).unwrap();
    let last = blue(out.last().unwrap()) as i32; // frame 8 = 80
    let first = blue(&out[0]) as i32; // tail frame 9 (90) pulled a quarter of the way towards the head (0)
    assert!(
        (first - last).abs() <= 30,
        "jump across the wrap was {}",
        (first - last).abs()
    );
}

#[test]
fn an_empty_recording_and_mixed_sizes_are_errors() {
    assert_eq!(build_loop(&[], 2), Err(SeamError::Empty));
    let mixed = vec![px(1), Frame::solid(3, 3, [0; 4]).unwrap()];
    assert_eq!(build_loop(&mixed, 2), Err(SeamError::SizeMismatch));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd crates/looper-core && cargo test seam
```

Expected: the build fails with unresolved imports of `build_loop`, `crossfade_len`, `seam_alpha`, `SeamError`.

- [ ] **Step 3: Write the implementation** by replacing `crates/looper-core/src/seam.rs` with:

```rust
//! The seamless loop. A recording of `n` frames becomes a loop of `n - k` frames in which the last `k` frames of the
//! recording are dissolved into the first `k`, so when playback wraps there is no jump.
//!
//! ```text
//! recording:  h0 h1 h2 h3 h4 h5 h6 h7 h8 h9        (k = 2)
//! loop:       [t8>h0] [t9>h1] h2 h3 h4 h5 h6 h7    (length 8; the first two are tail-into-head dissolves)
//! ```
//!
//! The loop starts at its first frame, which is nearly the tail of the recording (what the camera showed a moment
//! before the key was released), and ends on a frame that flows straight back into it.

use crate::compositor::{blend, ALPHA_ONE};
use crate::frame::Frame;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamError {
    Empty,
    SizeMismatch,
}

impl fmt::Display for SeamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeamError::Empty => write!(f, "cannot build a loop from no frames"),
            SeamError::SizeMismatch => {
                write!(f, "all frames of a recording must have the same size")
            }
        }
    }
}

impl std::error::Error for SeamError {}

/// How many frames of dissolve a recording of `frame_count` frames can afford: the request, but never more than
/// half the recording (so the loop is never shorter than the dissolve).
pub fn crossfade_len(frame_count: usize, requested: u32) -> usize {
    (requested as usize).min(frame_count / 2)
}

/// Blend weight (towards the head frame) for step `i` of a `k`-step dissolve: strictly increasing, never 0 or `ALPHA_ONE`.
pub fn seam_alpha(i: usize, k: usize) -> u16 {
    (((i + 1) * ALPHA_ONE as usize) / (k + 1)) as u16
}

/// Builds the loop from decoded frames. `requested` is the wanted dissolve length in frames.
pub fn build_loop(frames: &[Frame], requested: u32) -> Result<Vec<Frame>, SeamError> {
    let first = frames.first().ok_or(SeamError::Empty)?;
    if frames.iter().any(|f| !f.same_size(first)) {
        return Err(SeamError::SizeMismatch);
    }
    let n = frames.len();
    let k = crossfade_len(n, requested);
    if k == 0 {
        return Ok(frames.to_vec());
    }
    let mut out = Vec::with_capacity(n - k);
    for i in 0..k {
        let mixed = blend(&frames[n - k + i], &frames[i], seam_alpha(i, k))
            .map_err(|_| SeamError::SizeMismatch)?;
        out.push(mixed);
    }
    out.extend_from_slice(&frames[k..n - k]);
    Ok(out)
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test
```

Expected: `test result: ok. 39 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): seamless loop builder"
```

---
### Task 6: The `Looper` state machine

**Files:**
- Create: `crates/looper-core/src/looper.rs`, `crates/looper-core/src/looper/tests.rs`
- Modify: `crates/looper-core/src/lib.rs`

**Interfaces:**
- Consumes: `Frame`, `blend`, `Codec`/`CodecError`, `Config`/`ConfigError`, `crossfade_len`/`seam_alpha`/`build_loop` (Tasks 1 to 5).
- Produces: `Looper<C: Codec>` with `new(Config, C) -> Result<Self, ConfigError>`, `pedal_down(&mut self)`, `pedal_up(&mut self) -> Result<(), LooperError>`, `go_live(&mut self)`, `process(&mut self, live: &Frame) -> Result<Output, LooperError>`, `state() -> State`, `recorded_frames() -> usize`, `loop_len() -> usize`; `State { Live, Recording, Loop, Returning }`; `Output { frame: Frame, state: State }` (the state is the one **after** the frame was handled); `LooperError { Codec(CodecError), Blend(BlendError) }`. Plan 3's desktop app builds on exactly these names.

The behaviour table is the doc comment at the top of `looper.rs`; the tests below are its executable form.

- [ ] **Step 1: Write the failing tests.** Replace `crates/looper-core/src/lib.rs` with:

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod codec;
pub mod compositor;
pub mod config;
pub mod frame;
pub mod looper;
pub mod seam;

pub use codec::{Codec, CodecError, RawCodec};
pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use config::{Config, ConfigError};
pub use frame::{Frame, FrameError};
pub use looper::{Looper, LooperError, Output, State};
```

Create `crates/looper-core/src/looper.rs` containing only the test hook:

```rust
#[cfg(test)]
mod tests;
```

`crates/looper-core/src/looper/tests.rs`:

```rust
use super::*;
use crate::codec::RawCodec;
use crate::seam::build_loop;

fn cfg(min: u32, max: u32, cross: u32) -> Config {
    Config {
        fps: 30,
        crossfade_frames: cross,
        min_record_frames: min,
        max_record_frames: max,
        max_record_bytes: usize::MAX,
    }
}

fn px(v: u8) -> Frame {
    Frame::solid(2, 2, [v, v, v, 255]).unwrap()
}

fn blue(f: &Frame) -> u8 {
    f.pixel(0, 0).unwrap()[0]
}

fn looper(min: u32, max: u32, cross: u32) -> Looper<RawCodec> {
    Looper::new(cfg(min, max, cross), RawCodec).unwrap()
}

/// Feeds one live frame per value (each a flat grey of that value) and returns the outputs.
fn feed(l: &mut Looper<RawCodec>, values: &[u8]) -> Vec<Output> {
    values.iter().map(|&v| l.process(&px(v)).unwrap()).collect()
}

fn blues(outs: &[Output]) -> Vec<u8> {
    outs.iter().map(|o| blue(&o.frame)).collect()
}

/// Holds the pedal while `values` go by, then releases it.
fn record(l: &mut Looper<RawCodec>, values: &[u8]) {
    l.pedal_down();
    feed(l, values);
    l.pedal_up().unwrap();
}

/// Independent copy of the blend maths, so the tests do not just call the code under test.
fn mix(a: u8, b: u8, alpha: u32) -> u8 {
    ((a as u32 * (256 - alpha) + b as u32 * alpha + 128) >> 8) as u8
}

const TEN: [u8; 10] = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

#[test]
fn it_starts_live_and_passes_frames_through_untouched() {
    let mut l = looper(3, 100, 2);
    assert_eq!(l.state(), State::Live);
    let out = l.process(&px(42)).unwrap();
    assert_eq!(
        out,
        Output {
            frame: px(42),
            state: State::Live
        }
    );
}

#[test]
fn pressing_the_pedal_starts_recording_and_the_output_stays_live() {
    let mut l = looper(3, 100, 2);
    l.pedal_down();
    assert_eq!(l.state(), State::Recording);
    let outs = feed(&mut l, &[1, 2, 3, 4]);
    assert_eq!(blues(&outs), vec![1, 2, 3, 4]);
    assert!(outs.iter().all(|o| o.state == State::Recording));
    assert_eq!(l.recorded_frames(), 4);
}

#[test]
fn a_hold_shorter_than_the_minimum_is_ignored() {
    let mut l = looper(4, 100, 2);
    record(&mut l, &[1, 2, 3]);
    assert_eq!(l.state(), State::Live);
    assert_eq!(l.loop_len(), 0);
    assert_eq!(blues(&feed(&mut l, &[9])), vec![9]);
}

#[test]
fn a_hold_of_exactly_the_minimum_makes_a_loop() {
    let mut l = looper(3, 100, 0);
    record(&mut l, &[1, 2, 3]);
    assert_eq!(l.state(), State::Loop);
    assert_eq!(l.loop_len(), 3);
}

#[test]
fn releasing_the_pedal_with_nothing_recorded_does_nothing() {
    let mut l = looper(1, 100, 0);
    l.pedal_down();
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Live);
}

#[test]
fn the_loop_opens_with_the_tail_dissolving_into_the_head_then_plays_the_rest_and_wraps() {
    let mut l = looper(3, 100, 2);
    record(&mut l, &TEN);
    assert_eq!(l.state(), State::Loop);
    assert_eq!(l.loop_len(), 8);
    // First two frames dissolve the tail (90, 100) into the head (10, 20); then 30..80; then it wraps.
    let outs = feed(&mut l, &[250; 18]);
    let expected = [
        63, 47, 30, 40, 50, 60, 70, 80, 63, 47, 30, 40, 50, 60, 70, 80, 63, 47,
    ];
    assert_eq!(blues(&outs), expected);
    assert!(outs.iter().all(|o| o.state == State::Loop));
}

#[test]
fn the_looper_builds_exactly_the_loop_the_pure_seam_builder_does() {
    let mut l = looper(3, 100, 3);
    record(&mut l, &TEN);
    let frames: Vec<Frame> = TEN.iter().map(|&v| px(v)).collect();
    let reference: Vec<u8> = build_loop(&frames, 3).unwrap().iter().map(blue).collect();
    let played = blues(&feed(&mut l, &vec![0; reference.len()]));
    assert_eq!(played, reference);
}

#[test]
fn go_live_while_looping_dissolves_into_the_live_feed_then_goes_live() {
    let mut l = looper(3, 100, 2);
    record(&mut l, &[10, 20, 30, 40, 50, 60]); // loop of 4: [mix(50,10), mix(60,20), 30, 40]
    l.go_live();
    assert_eq!(l.state(), State::Returning);
    let outs = feed(&mut l, &[200, 200, 200, 200]);
    let loop0 = mix(50, 10, 85);
    let loop1 = mix(60, 20, 170);
    assert_eq!(blue(&outs[0].frame), mix(loop0, 200, 85));
    assert_eq!(blue(&outs[1].frame), mix(loop1, 200, 170));
    assert_eq!(outs[0].state, State::Returning);
    assert_eq!(outs[1].state, State::Live); // the dissolve's last frame is handled, and the looper is live again
    assert_eq!(
        outs[2],
        Output {
            frame: px(200),
            state: State::Live
        }
    );
    assert_eq!(
        outs[3],
        Output {
            frame: px(200),
            state: State::Live
        }
    );
}

#[test]
fn pressing_go_live_during_the_dissolve_does_nothing() {
    let mut l = looper(3, 100, 4);
    record(&mut l, &[10, 20, 30, 40, 50, 60, 70, 80]);
    l.go_live();
    feed(&mut l, &[200]);
    assert_eq!(l.state(), State::Returning);
    l.go_live();
    assert_eq!(l.state(), State::Returning);
    let outs = feed(&mut l, &[200, 200, 200]);
    assert_eq!(outs.last().unwrap().state, State::Live); // finished on schedule: 4 dissolve frames in all
}

#[test]
fn go_live_while_already_live_does_nothing() {
    let mut l = looper(3, 100, 2);
    l.go_live();
    assert_eq!(l.state(), State::Live);
    assert_eq!(blues(&feed(&mut l, &[5])), vec![5]);
}

#[test]
fn go_live_while_recording_cancels_the_recording() {
    let mut l = looper(3, 100, 2);
    l.pedal_down();
    feed(&mut l, &[1, 2, 3, 4, 5]);
    l.go_live();
    assert_eq!(l.state(), State::Live);
    l.pedal_up().unwrap(); // the release that follows the cancel must not resurrect anything
    assert_eq!(l.state(), State::Live);
    assert_eq!(l.loop_len(), 0);
    assert_eq!(blues(&feed(&mut l, &[7])), vec![7]);
}

#[test]
fn a_repeated_key_down_does_not_restart_the_recording() {
    let mut l = looper(3, 100, 0);
    l.pedal_down();
    feed(&mut l, &[1, 2]);
    l.pedal_down();
    feed(&mut l, &[3, 4]);
    assert_eq!(l.recorded_frames(), 4);
    l.pedal_up().unwrap();
    assert_eq!(l.loop_len(), 4);
}

#[test]
fn releasing_without_having_pressed_does_nothing_in_any_state() {
    let mut l = looper(3, 100, 0);
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Live);
    record(&mut l, &[1, 2, 3]);
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Loop);
    l.go_live();
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Live); // crossfade 0: go_live went straight to live
}

#[test]
fn recording_while_looping_replaces_the_loop() {
    let mut l = looper(3, 100, 0);
    record(&mut l, &[10, 20, 30, 40]);
    assert_eq!(l.state(), State::Loop);
    l.pedal_down();
    assert_eq!(l.state(), State::Recording);
    assert_eq!(blues(&feed(&mut l, &[100, 110, 120])), vec![100, 110, 120]); // live while recording
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Loop);
    assert_eq!(
        blues(&feed(&mut l, &[0; 7])),
        vec![100, 110, 120, 100, 110, 120, 100]
    ); // the old loop is gone
}

#[test]
fn pressing_record_during_the_dissolve_starts_a_recording() {
    let mut l = looper(3, 100, 4);
    record(&mut l, &[10, 20, 30, 40, 50, 60, 70, 80]);
    l.go_live();
    feed(&mut l, &[200]);
    l.pedal_down();
    assert_eq!(l.state(), State::Recording);
    assert_eq!(blues(&feed(&mut l, &[5])), vec![5]);
}

#[test]
fn hitting_the_frame_cap_ends_the_recording_and_starts_the_loop_by_itself() {
    let mut l = looper(2, 4, 0);
    l.pedal_down();
    let outs = feed(&mut l, &[1, 2, 3, 4, 5, 6]);
    assert_eq!(blues(&outs), vec![1, 2, 3, 4, 1, 2]); // frame 4 filled the buffer; frames 5 and 6 are loop playback
    assert_eq!(outs[2].state, State::Recording);
    assert_eq!(outs[3].state, State::Loop);
    l.pedal_up().unwrap(); // the late release is harmless
    assert_eq!(l.state(), State::Loop);
    assert_eq!(l.loop_len(), 4);
}

#[test]
fn hitting_the_memory_budget_also_ends_the_recording() {
    let config = Config {
        max_record_bytes: 3 * 16,
        ..cfg(1, 1000, 0)
    }; // a 2x2 frame is 16 bytes
    let mut l = Looper::new(config, RawCodec).unwrap();
    l.pedal_down();
    let outs = feed(&mut l, &[1, 2, 3, 4]);
    assert_eq!(outs[2].state, State::Loop);
    assert_eq!(l.loop_len(), 3);
}

#[test]
fn a_resolution_change_while_recording_abandons_the_recording_and_shows_live() {
    let mut l = looper(3, 100, 2);
    l.pedal_down();
    feed(&mut l, &[1, 2]);
    let bigger = Frame::solid(3, 3, [9, 9, 9, 255]).unwrap();
    let out = l.process(&bigger).unwrap();
    assert_eq!(
        out,
        Output {
            frame: bigger,
            state: State::Live
        }
    );
    assert_eq!(l.recorded_frames(), 0);
}

#[test]
fn a_resolution_change_while_looping_drops_the_loop_and_shows_live() {
    let mut l = looper(3, 100, 2);
    record(&mut l, &TEN);
    let bigger = Frame::solid(3, 3, [9, 9, 9, 255]).unwrap();
    let out = l.process(&bigger).unwrap();
    assert_eq!(
        out,
        Output {
            frame: bigger,
            state: State::Live
        }
    );
    assert_eq!(l.loop_len(), 0);
    assert_eq!(blues(&feed(&mut l, &[4])), vec![4]); // and it stays live at the old size too
}

#[test]
fn a_resolution_change_while_returning_goes_live_too() {
    let mut l = looper(3, 100, 4);
    record(&mut l, &[10, 20, 30, 40, 50, 60, 70, 80]);
    l.go_live();
    let bigger = Frame::solid(4, 1, [9, 9, 9, 255]).unwrap();
    let out = l.process(&bigger).unwrap();
    assert_eq!(out.state, State::Live);
    assert_eq!(out.frame, bigger);
}

#[test]
fn with_no_crossfade_the_loop_is_the_recording_and_the_return_is_instant() {
    let mut l = looper(3, 100, 0);
    record(&mut l, &[1, 2, 3, 4]);
    assert_eq!(blues(&feed(&mut l, &[0; 5])), vec![1, 2, 3, 4, 1]);
    l.go_live();
    assert_eq!(l.state(), State::Live);
    assert_eq!(blues(&feed(&mut l, &[77])), vec![77]);
}

#[test]
fn a_one_frame_recording_loops_that_one_frame() {
    let mut l = looper(1, 100, 12);
    record(&mut l, &[33]);
    assert_eq!(l.loop_len(), 1);
    assert_eq!(blues(&feed(&mut l, &[0; 4])), vec![33; 4]);
}

#[test]
fn a_two_frame_recording_with_a_long_crossfade_still_loops() {
    let mut l = looper(2, 100, 12);
    record(&mut l, &[0, 200]);
    assert_eq!(l.loop_len(), 1);
    assert_eq!(blues(&feed(&mut l, &[0; 3])), vec![100; 3]);
}

#[test]
fn a_recording_of_one_by_one_frames_works() {
    let mut l = Looper::new(cfg(2, 100, 1), RawCodec).unwrap();
    l.pedal_down();
    for v in [10u8, 20, 30, 40] {
        l.process(&Frame::solid(1, 1, [v, v, v, 255]).unwrap())
            .unwrap();
    }
    l.pedal_up().unwrap();
    let out = l
        .process(&Frame::solid(1, 1, [0, 0, 0, 255]).unwrap())
        .unwrap();
    assert_eq!(out.state, State::Loop);
    assert_eq!((out.frame.width(), out.frame.height()), (1, 1));
}

#[test]
fn an_invalid_config_is_refused() {
    assert!(Looper::new(
        Config {
            min_record_frames: 0,
            ..cfg(1, 10, 0)
        },
        RawCodec
    )
    .is_err());
    assert!(Looper::new(cfg(11, 10, 0), RawCodec).is_err());
}

/// A codec whose decoder always fails, to exercise the error paths.
struct BrokenDecoder;

impl Codec for BrokenDecoder {
    fn encode(&self, frame: &Frame) -> Vec<u8> {
        frame.data().to_vec()
    }

    fn decode(&self, _bytes: &[u8], _w: u32, _h: u32) -> Result<Frame, CodecError> {
        Err(CodecError::Corrupt)
    }
}

#[test]
fn a_decode_failure_during_playback_returns_the_error_and_falls_back_to_live() {
    let mut l = Looper::new(cfg(2, 100, 0), BrokenDecoder).unwrap(); // no crossfade: commit decodes nothing
    l.pedal_down();
    for v in [1u8, 2, 3] {
        l.process(&px(v)).unwrap();
    }
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Loop);
    assert_eq!(
        l.process(&px(9)),
        Err(LooperError::Codec(CodecError::Corrupt))
    );
    assert_eq!(l.state(), State::Live);
    assert_eq!(
        l.process(&px(9)).unwrap(),
        Output {
            frame: px(9),
            state: State::Live
        }
    );
}

#[test]
fn a_decode_failure_while_building_the_loop_leaves_the_looper_live() {
    let mut l = Looper::new(cfg(2, 100, 2), BrokenDecoder).unwrap();
    l.pedal_down();
    for v in [1u8, 2, 3, 4] {
        l.process(&px(v)).unwrap();
    }
    assert_eq!(l.pedal_up(), Err(LooperError::Codec(CodecError::Corrupt)));
    assert_eq!(l.state(), State::Live);
}

/// Tiny deterministic random numbers, so the stress test needs no dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn random_sequences_of_events_never_panic_and_always_return_a_frame_of_the_live_size() {
    for seed in 0..60u64 {
        let mut rng = Lcg(seed + 1);
        let min = 1 + (seed % 3) as u32;
        let max = min + (seed % 6) as u32;
        let mut l = looper(min, max, (seed % 5) as u32);
        for _ in 0..3000 {
            match rng.next() % 10 {
                0 | 1 => l.pedal_down(),
                2 | 3 => l.pedal_up().unwrap(),
                4 => l.go_live(),
                _ => {
                    let (w, h) = match rng.next() % 12 {
                        0 => (3, 2),
                        1 => (1, 1),
                        _ => (2, 2),
                    };
                    let v = (rng.next() % 256) as u8;
                    let live = Frame::solid(w, h, [v, v, v, 255]).unwrap();
                    let out = l.process(&live).unwrap();
                    assert_eq!(
                        (out.frame.width(), out.frame.height()),
                        (w, h),
                        "seed {seed}"
                    );
                    assert_eq!(out.state, l.state(), "seed {seed}");
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd crates/looper-core && cargo test looper
```

Expected: the build fails with many errors such as `unresolved imports 'looper::Looper' ...` and `cannot find type 'Frame' in this scope`.

- [ ] **Step 3: Write the implementation** by replacing `crates/looper-core/src/looper.rs` with:

```rust
//! The record / loop / return state machine.
//!
//! One call to [`Looper::process`] per live camera frame; it returns the frame the virtual camera should show.
//! There is no clock and no I/O: playback advances one frame per `process` call, so behaviour is fully deterministic.
//!
//! ```text
//!   Live --pedal_down--> Recording --pedal_up (long enough)--> Loop --go_live--> Returning --k frames--> Live
//!                           |  \--pedal_up (too short)--> Live          \--pedal_down--> Recording (replaces the loop)
//!                           \--go_live--> Live (recording discarded)
//! ```
//!
//! | event \ state | Live | Recording | Loop | Returning |
//! |---|---|---|---|---|
//! | `pedal_down` | start recording | ignored (key repeat) | start recording | start recording |
//! | `pedal_up` | ignored | commit the loop, or ignore if shorter than the minimum | ignored | ignored |
//! | `go_live` | ignored | cancel the recording | start the dissolve to live | ignored |
//!
//! Output while recording is always the live frame. A recording that reaches its frame or byte cap ends by itself and
//! starts looping. A change of live frame size while recording, looping or returning abandons the loop and shows live.

use crate::codec::{Codec, CodecError};
use crate::compositor::{blend, BlendError};
use crate::config::{Config, ConfigError};
use crate::frame::Frame;
use crate::seam::{crossfade_len, seam_alpha};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Live,
    Recording,
    Loop,
    Returning,
}

/// What to show for one live frame, and the state the looper is in after handling it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub frame: Frame,
    pub state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LooperError {
    Codec(CodecError),
    Blend(BlendError),
}

impl From<CodecError> for LooperError {
    fn from(e: CodecError) -> Self {
        LooperError::Codec(e)
    }
}

impl From<BlendError> for LooperError {
    fn from(e: BlendError) -> Self {
        LooperError::Blend(e)
    }
}

impl fmt::Display for LooperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LooperError::Codec(e) => write!(f, "{e}"),
            LooperError::Blend(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LooperError {}

struct Recording {
    frames: Vec<Vec<u8>>,
    bytes: usize,
    dims: Option<(u32, u32)>,
}

struct Playback {
    frames: Vec<Vec<u8>>,
    dims: (u32, u32),
    pos: usize,
}

enum Mode {
    Live,
    Recording(Recording),
    Loop(Playback),
    Returning { play: Playback, step: u32 },
}

impl Mode {
    fn state(&self) -> State {
        match self {
            Mode::Live => State::Live,
            Mode::Recording(_) => State::Recording,
            Mode::Loop(_) => State::Loop,
            Mode::Returning { .. } => State::Returning,
        }
    }
}

pub struct Looper<C: Codec> {
    config: Config,
    codec: C,
    mode: Mode,
}

impl<C: Codec> Looper<C> {
    pub fn new(config: Config, codec: C) -> Result<Self, ConfigError> {
        config.validate()?;
        Ok(Looper {
            config,
            codec,
            mode: Mode::Live,
        })
    }

    pub fn state(&self) -> State {
        self.mode.state()
    }

    /// Frames captured so far in the current recording (0 when not recording).
    pub fn recorded_frames(&self) -> usize {
        match &self.mode {
            Mode::Recording(r) => r.frames.len(),
            _ => 0,
        }
    }

    /// Length in frames of the loop being played (0 when there is none).
    pub fn loop_len(&self) -> usize {
        match &self.mode {
            Mode::Loop(p) | Mode::Returning { play: p, .. } => p.frames.len(),
            _ => 0,
        }
    }

    /// The record key went down. Starts a recording unless one is already running (key repeat).
    pub fn pedal_down(&mut self) {
        if !matches!(self.mode, Mode::Recording(_)) {
            self.mode = Mode::Recording(Recording {
                frames: Vec::new(),
                bytes: 0,
                dims: None,
            });
        }
    }

    /// The record key went up. Turns a long-enough recording into a loop; ignores one that is too short.
    /// On an error the looper is left showing live video.
    pub fn pedal_up(&mut self) -> Result<(), LooperError> {
        match std::mem::replace(&mut self.mode, Mode::Live) {
            Mode::Recording(rec) => {
                if !rec.frames.is_empty()
                    && rec.frames.len() >= self.config.min_record_frames as usize
                {
                    self.mode = self.commit(rec)?;
                }
                Ok(())
            }
            other => {
                self.mode = other;
                Ok(())
            }
        }
    }

    /// The go-live key was pressed.
    pub fn go_live(&mut self) {
        self.mode = match std::mem::replace(&mut self.mode, Mode::Live) {
            Mode::Recording(_) => Mode::Live,
            Mode::Loop(play) => {
                if self.config.crossfade_frames == 0 {
                    Mode::Live
                } else {
                    Mode::Returning { play, step: 0 }
                }
            }
            other @ (Mode::Live | Mode::Returning { .. }) => other,
        };
    }

    /// Handles one live frame and returns the frame to show. On an error the looper falls back to live video, so the
    /// next call shows the camera again.
    pub fn process(&mut self, live: &Frame) -> Result<Output, LooperError> {
        let live_dims = (live.width(), live.height());
        match std::mem::replace(&mut self.mode, Mode::Live) {
            Mode::Live => Ok(self.finish(live.clone(), Mode::Live)),

            Mode::Recording(mut rec) => {
                match rec.dims {
                    None => rec.dims = Some(live_dims),
                    Some(dims) if dims != live_dims => {
                        return Ok(self.finish(live.clone(), Mode::Live))
                    }
                    Some(_) => {}
                }
                let bytes = self.codec.encode(live);
                rec.bytes = rec.bytes.saturating_add(bytes.len());
                rec.frames.push(bytes);
                let full = rec.frames.len() >= self.config.max_record_frames as usize
                    || rec.bytes >= self.config.max_record_bytes;
                let next = if full {
                    self.commit(rec)?
                } else {
                    Mode::Recording(rec)
                };
                Ok(self.finish(live.clone(), next))
            }

            Mode::Loop(mut play) => {
                if play.dims != live_dims {
                    return Ok(self.finish(live.clone(), Mode::Live));
                }
                let frame = self.decode_current(&play)?;
                play.pos = (play.pos + 1) % play.frames.len();
                Ok(self.finish(frame, Mode::Loop(play)))
            }

            Mode::Returning { mut play, step } => {
                if play.dims != live_dims {
                    return Ok(self.finish(live.clone(), Mode::Live));
                }
                let loop_frame = self.decode_current(&play)?;
                let k = self.config.crossfade_frames as u64;
                let alpha =
                    (((step as u64 + 1) * crate::compositor::ALPHA_ONE as u64) / (k + 1)) as u16;
                let out = blend(&loop_frame, live, alpha)?;
                play.pos = (play.pos + 1) % play.frames.len();
                let step = step + 1;
                let next = if step as u64 >= k {
                    Mode::Live
                } else {
                    Mode::Returning { play, step }
                };
                Ok(self.finish(out, next))
            }
        }
    }

    fn finish(&mut self, frame: Frame, mode: Mode) -> Output {
        let state = mode.state();
        self.mode = mode;
        Output { frame, state }
    }

    fn decode_current(&self, play: &Playback) -> Result<Frame, LooperError> {
        Ok(self
            .codec
            .decode(&play.frames[play.pos], play.dims.0, play.dims.1)?)
    }

    /// Turns a finished recording into a loop: the last `k` frames are dissolved into the first `k` and dropped.
    fn commit(&self, rec: Recording) -> Result<Mode, LooperError> {
        let Some(dims) = rec.dims else {
            return Ok(Mode::Live);
        };
        let n = rec.frames.len();
        let k = crossfade_len(n, self.config.crossfade_frames);
        let mut frames = rec.frames;
        if k > 0 {
            let mut seam = Vec::with_capacity(k);
            for i in 0..k {
                let tail = self.codec.decode(&frames[n - k + i], dims.0, dims.1)?;
                let head = self.codec.decode(&frames[i], dims.0, dims.1)?;
                let mixed = blend(&tail, &head, seam_alpha(i, k))?;
                seam.push(self.codec.encode(&mixed));
            }
            frames.truncate(n - k);
            for (i, encoded) in seam.into_iter().enumerate() {
                frames[i] = encoded;
            }
        }
        Ok(Mode::Loop(Playback {
            frames,
            dims,
            pos: 0,
        }))
    }
}

#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test
```

Expected: `test result: ok. 67 passed; 0 failed` (9 frame, 10 compositor, 4 codec, 6 config, 10 seam, 28 looper).

- [ ] **Step 5: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): record/loop/return state machine"
```

---
### Task 7: Quality gates, documentation, timing, and the mutation check

**Files:**
- Create: `crates/looper-core/README.md`, `crates/looper-core/check.sh`, `crates/looper-core/examples/bench.rs`, `crates/looper-core/tools/mutation_check.py`
- Modify: `crates/looper-core/src/lib.rs` (add a compiled usage example)

**Interfaces:**
- Consumes: the whole crate.
- Produces: one command that runs every gate (`./check.sh`); a doc example that is compiled as a test, so it cannot go stale; measured per-frame timings; a tool that proves the tests fail when the code is broken.

- [ ] **Step 1: Add the usage example to the crate docs.** Replace `crates/looper-core/src/lib.rs` with the final version:

```rust
//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.
//!
//! ```
//! use looper_core::{Config, Frame, Looper, RawCodec, State};
//!
//! let mut looper = Looper::new(Config::for_fps(30).unwrap(), RawCodec).unwrap();
//! let live = Frame::solid(4, 4, [10, 20, 30, 255]).unwrap();
//!
//! looper.pedal_down(); // hold the record key...
//! for _ in 0..30 {
//!     looper.process(&live).unwrap(); // ...while frames arrive (the output stays live)
//! }
//! looper.pedal_up().unwrap(); // release: the recording becomes a loop
//! assert_eq!(looper.state(), State::Loop);
//!
//! let shown = looper.process(&live).unwrap(); // from now on the camera shows the loop
//! assert_eq!(shown.state, State::Loop);
//! looper.go_live(); // dissolve back to the live feed
//! ```

pub mod codec;
pub mod compositor;
pub mod config;
pub mod frame;
pub mod looper;
pub mod seam;

pub use codec::{Codec, CodecError, RawCodec};
pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use config::{Config, ConfigError};
pub use frame::{Frame, FrameError};
pub use looper::{Looper, LooperError, Output, State};
```

- [ ] **Step 2: Add the check script and the benchmark.**

`crates/looper-core/check.sh`:

```bash
#!/usr/bin/env bash
# Runs every quality gate for the crate. Add --mutation to also run the (slower) mutation check.
set -euo pipefail
cd "$(dirname "$0")"
echo "== fmt ==";    cargo fmt --check
echo "== clippy =="; cargo clippy --all-targets -- -D warnings
echo "== tests ==";  cargo test --quiet
echo "== docs ==";   RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --quiet
if [ "${1:-}" = "--mutation" ]; then echo "== mutation check =="; python tools/mutation_check.py; fi
echo "ALL CHECKS PASSED"
```

`crates/looper-core/examples/bench.rs`:

```rust
//! Rough timing of the per-frame work at 720p. Run with: cargo run --release --example bench
//! The budget at 30 fps is 33 ms per frame for everything; this only measures the looper's share.

use looper_core::{blend_into, Config, Frame, Looper, RawCodec, State};
use std::time::Instant;

const W: u32 = 1280;
const H: u32 = 720;
const FRAMES: usize = 300;

fn time_ms(label: &str, mut f: impl FnMut()) {
    f(); // warm up
    let start = Instant::now();
    for _ in 0..FRAMES {
        f();
    }
    let per = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
    println!("{label:<44} {per:6.2} ms/frame");
}

fn main() {
    let a = Frame::solid(W, H, [10, 20, 30, 255]).unwrap();
    let b = Frame::solid(W, H, [200, 150, 100, 255]).unwrap();
    let mut dst = a.clone();
    time_ms("blend_into 1280x720", || {
        blend_into(&mut dst, &a, &b, 100).unwrap()
    });

    let config = Config::for_fps(30).unwrap();

    let mut live = Looper::new(config, RawCodec).unwrap();
    time_ms("process: live pass-through (clone)", || {
        live.process(&a).unwrap();
    });

    let mut rec = Looper::new(config, RawCodec).unwrap();
    rec.pedal_down();
    let mut n = 0;
    time_ms("process: recording (raw encode)", || {
        if n < 200 {
            rec.process(&a).unwrap();
            n += 1;
        }
    });

    let mut lp = Looper::new(config, RawCodec).unwrap();
    lp.pedal_down();
    for _ in 0..60 {
        lp.process(&a).unwrap();
    }
    lp.pedal_up().unwrap();
    assert_eq!(lp.state(), State::Loop);
    time_ms("process: loop playback (raw decode)", || {
        lp.process(&b).unwrap();
    });

    let raw_mb = (W * H * 4) as f64 / 1e6;
    println!(
        "\nstored raw: {raw_mb:.1} MB per frame; {:.0} MB for the default 20 s at 30 fps",
        raw_mb * 600.0
    );
}
```

Then make the script executable: `chmod +x crates/looper-core/check.sh`.

- [ ] **Step 3: Format, lint, test and build the docs**

```bash
cd crates/looper-core && cargo fmt && ./check.sh
```

Expected: ends with `ALL CHECKS PASSED`; the test step shows `67 passed` for the unit tests and `1 passed` for the doc test. `cargo fmt` may rewrite long lines in the files above; that is expected, and the committed files should be the formatted ones.

- [ ] **Step 4: Measure the per-frame cost**

```bash
cargo run --release --example bench
```

Expected: lines like `blend_into 1280x720  0.5 ms/frame` and the loop/recording/live timings each below about 2 ms (measured: 0.54, 1.17, 1.38, 1.09), then `stored raw: 3.7 MB per frame; 2212 MB for the default 20 s at 30 fps`. If a number is more than about 5 times higher, stop and investigate before continuing. Copy the real numbers into the README table in the next step.

- [ ] **Step 5: Add the mutation-check tool and run it**

`crates/looper-core/tools/mutation_check.py`:

```python
"""Checks that the tests can fail: breaks the code in a handful of deliberate ways (on temporary copies) and reports
whether `cargo test` notices each one. A mutation that SURVIVES means a behaviour is not pinned by any test.

Run from the crate folder:  python tools/mutation_check.py
Exit code is 0 only if every mutation was caught.
"""
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

CRATE = pathlib.Path(__file__).resolve().parents[1]

# (file under src/, exact text to find, replacement, what the break means)
MUTATIONS = [
    ("looper.rs", "frames.truncate(n - k);", "frames.truncate(n);",
     "the loop keeps the tail frames (wrong length)"),
    ("compositor.rs", "+ 128) >> 8", "+ 0) >> 8",
     "blending rounds down instead of to nearest"),
    ("looper.rs", "Some(dims) if dims != live_dims", "Some(dims) if dims != dims",
     "a size change while recording is not noticed"),
    ("looper.rs", "rec.frames.len() >= self.config.max_record_frames as usize",
     "rec.frames.len() > self.config.max_record_frames as usize",
     "the frame cap is off by one"),
    ("looper.rs", "rec.bytes >= self.config.max_record_bytes", "rec.bytes > self.config.max_record_bytes",
     "the memory cap is off by one"),
    ("seam.rs", "(((i + 1) * ALPHA_ONE as usize) / (k + 1)) as u16", "((i * ALPHA_ONE as usize) / k.max(1)) as u16",
     "the seam ramp starts at 0 and reaches the end"),
    ("looper.rs", "other @ (Mode::Live | Mode::Returning { .. }) => other,",
     "other @ (Mode::Live | Mode::Returning { .. }) => if matches!(other, Mode::Returning { .. }) { Mode::Live } else { other },",
     "go_live during the dissolve is no longer a no-op"),
    ("looper.rs", "&& rec.frames.len() >= self.config.min_record_frames as usize",
     "&& rec.frames.len() >= 0",
     "short holds are no longer ignored"),
    ("looper.rs", "if !matches!(self.mode, Mode::Recording(_)) {", "if true {",
     "a repeated key-down restarts the recording"),
    ("config.rs", "if self.min_record_frames > self.max_record_frames {", "if false {",
     "an impossible minimum/maximum is accepted"),
]


def main() -> int:
    survivors = 0
    for i, (name, old, new, what) in enumerate(MUTATIONS):
        work = pathlib.Path(tempfile.mkdtemp(prefix="mutation-"))
        try:
            shutil.copytree(CRATE, work / "crate", ignore=shutil.ignore_patterns("target", ".git"))
            target = work / "crate" / "src" / name
            text = target.read_text(encoding="utf-8")
            if old not in text:
                print(f"[{i}] PATTERN NOT FOUND in {name}: {what}")
                survivors += 1
                continue
            target.write_text(text.replace(old, new, 1), encoding="utf-8")
            env = dict(os.environ, CARGO_TARGET_DIR=str(CRATE / "target" / "mutation"))
            run = subprocess.run(["cargo", "test", "--quiet"], cwd=work / "crate", capture_output=True, text=True, env=env)
            summary = re.search(r"test result: \w+\. \d+ passed; \d+ failed", run.stdout + run.stderr)
            caught = run.returncode != 0
            if not caught:
                survivors += 1
            print(f"[{i}] {'caught  ' if caught else 'SURVIVED'} {what}"
                  f"{'  (' + summary.group(0) + ')' if summary else ''}")
        finally:
            shutil.rmtree(work, ignore_errors=True)
    print(f"\n{len(MUTATIONS) - survivors}/{len(MUTATIONS)} mutations caught")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
```

```bash
python tools/mutation_check.py
```

Expected: ten lines starting `[0] caught` through `[9] caught` and `10/10 mutations caught`, exit code 0. **If any line says `SURVIVED`, a behaviour is not pinned by a test: add a test for it (RED first), do not weaken the tool.** If a line says `PATTERN NOT FOUND`, `cargo fmt` reflowed the text it searches for: update that pattern in the tool, not the code.

- [ ] **Step 6: Write the README**

`crates/looper-core/README.md`:

```markdown
# looper-core

The pure-logic core of the webcam loop product: the record / loop / return state machine and the frame blending it
needs. No I/O, no clock, no dependencies. The desktop app feeds it one live camera frame at a time and shows whatever
frame it returns.

## Behaviour

States: `Live`, `Recording`, `Loop`, `Returning` (the dissolve back to live).

| event \ state | Live | Recording | Loop | Returning |
|---|---|---|---|---|
| `pedal_down` | start recording | ignored (key repeat) | start recording (the old loop is dropped) | start recording |
| `pedal_up` | ignored | make the loop, or ignore if shorter than the minimum | ignored | ignored |
| `go_live` | ignored | cancel the recording | start the dissolve to live | ignored |

- The output while recording is always the live frame.
- A recording that reaches its frame cap or byte budget ends by itself and starts looping; the later key release is harmless.
- **The loop** is the recording with its last `k` frames dissolved into its first `k` and then dropped, so playback wraps
  without a jump. It starts at its first frame, which is nearly the tail of the recording.
- **Returning** blends the loop into the live feed over `k` frames, then the output is live.
- If the live frame size changes while recording, looping or returning, the loop is abandoned and live video is shown.
- If storing or reading a frame fails, the looper falls back to live video and returns the error.
- Playback advances one frame per `process` call, so behaviour is deterministic and easy to test.

## Defaults (`Config::for_fps`)

| Setting | Default | Meaning |
|---|---|---|
| crossfade | 400 ms (12 frames at 30 fps) | dissolve at the seam and when returning to live; 0 means hard cuts |
| minimum hold | 750 ms (23 frames) | shorter holds are ignored |
| maximum recording | 20 s (600 frames) | recording stops and the loop starts |
| memory budget | 512 MiB of stored data | recording also stops here |

All are fields of `Config`, so the product can change them.

## Memory and speed (1280x720, release build, this PC)

| Work | Time per frame |
|---|---|
| blend two frames | 0.54 ms |
| live pass-through | 1.2 ms |
| recording (raw store) | 1.4 ms |
| loop playback (raw read) | 1.1 ms |

The budget at 30 fps is 33 ms for everything. **Storage is the real constraint:** a raw 720p frame is 3.7 MB, so 20
seconds raw would be 2.2 GB and the 512 MiB default budget would end a raw recording after about 4.6 s. The product
must plug in a compressing [`Codec`](src/codec.rs) (JPEG or similar). `RawCodec` exists for tests.

## Using it

```rust
use looper_core::{Config, Frame, Looper, RawCodec, State};

let mut looper = Looper::new(Config::for_fps(30)?, RawCodec)?;
looper.pedal_down();                     // record key down
let shown = looper.process(&live_frame)?; // call once per camera frame; show `shown.frame`
looper.pedal_up()?;                      // record key up: the recording becomes a loop
looper.go_live();                        // dissolve back to the live feed
```

## Checks

```
./check.sh               # fmt, clippy (warnings are errors), tests, docs
./check.sh --mutation    # also breaks the code in 10 ways and confirms the tests notice each one
cargo run --release --example bench
```

## Not here

Capture, hotkeys, the virtual camera, a real frame codec, and anything that touches Windows. The frame type is BGRA,
8 bits per channel, top-down with no row padding, matching the shared-memory frame layout used by the camera DLL.
```

Replace the timing numbers in the README table with the ones from Step 4 if they differ.

- [ ] **Step 7: Run everything once more, with the mutation check**

```bash
./check.sh --mutation
```

Expected: `ALL CHECKS PASSED` and `10/10 mutations caught`.

- [ ] **Step 8: Commit**

```bash
git add crates/looper-core
git commit -m "feat(looper-core): quality gates, README, benchmark and mutation check"
```

---

## Self-Review

**Spec coverage** (spec 4.1, 4.4, 4.5):
| Requirement | Where |
|---|---|
| State machine `LIVE -> RECORDING -> LOOP -> RETURNING -> LIVE` | Task 6 (`State`, `Looper`) |
| Crossfaded seam | Task 5 (`build_loop`), Task 6 (`commit`) |
| Dissolve back to live | Task 6 (`Mode::Returning`), test `go_live_while_looping_dissolves_*` |
| Holds shorter than the minimum are ignored | Task 6 `a_hold_shorter_than_the_minimum_is_ignored` |
| Recording while looping replaces the loop | Task 6 `recording_while_looping_replaces_the_loop` |
| Go-live during recording cancels it | Task 6 `go_live_while_recording_cancels_the_recording` |
| Go-live during a dissolve is a no-op | Task 6 `pressing_go_live_during_the_dissolve_does_nothing` |
| Capped by maximum duration and a resolution-dependent memory budget | Task 4 (`Config`), Task 6 `hitting_the_frame_cap_*`, `hitting_the_memory_budget_*` |
| Compositor: live + loop frames + blend amount -> output frame | Task 2 |
| Unit tests without a camera, including edge cases | Tasks 1 to 6; Task 7 proves the tests catch breakage |

**Placeholder scan:** none. Every code block is the verified file, and every command states its expected output.

**Type consistency:** checked by the compiler, since the plan's files are the compiled files. Names used by later tasks (`blend`, `ALPHA_ONE`, `crossfade_len`, `seam_alpha`, `build_loop`, `Codec`, `Config`) are defined in the Interfaces blocks of the tasks that produce them.

**Review Focus:** each of the five items is pinned by named tests (see the list above) and by the mutation check.

**Known limits:** `RawCodec` is for tests only; the product needs a compressing codec (Plan 3). Frame pacing against the camera clock is not this crate's problem (Gate 0's open issue). Nothing here touches Windows.
