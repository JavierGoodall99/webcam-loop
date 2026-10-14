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
| blend two frames | 0.48 ms |
| live pass-through | 1.2 ms |
| recording (raw store) | 1.8 ms |
| loop playback (raw read) | 1.3 ms |

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
