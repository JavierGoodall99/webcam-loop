//! The record / loop / return state machine.
//!
//! One call to [`Looper::process`] per live camera frame; it returns the frame the virtual camera should show.
//! There is no clock and no I/O: playback advances one frame per `process` call, so behaviour is fully deterministic.
//!
//! ```text
//!   Live --pedal_down--> Recording --pedal_up (long enough)--> Loop --go_live--> Returning --k frames--> Live
//!                           |  \--pedal_up (too short)--> Live          \--pedal_down--> Recording (the loop stays on air)
//!                           \--go_live--> Live (recording discarded)
//! ```
//!
//! While a loop is on air, pressing record keeps that loop playing until the new recording replaces it; a hold that is
//! too short is ignored and the loop carries on, and go-live dissolves the loop into the live feed.
//!
//! | event \ state | Live | Recording | Loop | Returning |
//! |---|---|---|---|---|
//! | `pedal_down` | start recording | ignored (key repeat) | start recording; the loop stays on air | start recording |
//! | `pedal_up` | ignored | commit the loop, or ignore if shorter than the minimum (a loop on air carries on) | ignored | ignored |
//! | `go_live` | ignored | cancel the recording (a loop on air dissolves into live) | start the dissolve to live | ignored |
//!
//! Output while recording is the live frame, or the loop that was already on air if recording over one. A recording that reaches its frame or byte cap ends by itself and
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
    /// The loop that was playing when recording started. It stays on air until the new recording replaces it, and
    /// comes back if the hold turns out to be too short, so an accidental tap never exposes the live feed.
    over: Option<Playback>,
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

    /// The record key went down. Starts a recording unless one is already running (key repeat). If a loop is
    /// playing it stays on air while the new recording is made.
    pub fn pedal_down(&mut self) {
        let over = match std::mem::replace(&mut self.mode, Mode::Live) {
            Mode::Recording(running) => {
                self.mode = Mode::Recording(running);
                return;
            }
            Mode::Loop(play) => Some(play),
            Mode::Live | Mode::Returning { .. } => None,
        };
        self.mode = Mode::Recording(Recording {
            frames: Vec::new(),
            bytes: 0,
            dims: None,
            over,
        });
    }

    /// The record key went up. Turns a long-enough recording into a loop; a hold that is too short is ignored, and
    /// a loop that was playing carries on. On an error the looper is left showing live video.
    pub fn pedal_up(&mut self) -> Result<(), LooperError> {
        match std::mem::replace(&mut self.mode, Mode::Live) {
            Mode::Recording(rec) => {
                if !rec.frames.is_empty()
                    && rec.frames.len() >= self.config.min_record_frames as usize
                {
                    self.mode = self.commit(rec)?;
                } else if let Some(play) = rec.over {
                    self.mode = Mode::Loop(play);
                }
                Ok(())
            }
            other => {
                self.mode = other;
                Ok(())
            }
        }
    }

    /// The go-live key was pressed. Cancels a recording in progress; if a loop was on air under it, that loop
    /// dissolves into the live feed.
    pub fn go_live(&mut self) {
        self.mode = match std::mem::replace(&mut self.mode, Mode::Live) {
            Mode::Recording(rec) => match rec.over {
                Some(play) if self.config.crossfade_frames > 0 => Mode::Returning { play, step: 0 },
                _ => Mode::Live,
            },
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
                // What viewers see meanwhile: the loop that was playing, if any, otherwise the live feed.
                let shown = match rec.over.as_mut() {
                    Some(play) if play.dims == live_dims => {
                        let frame = self.decode_current(play)?;
                        play.pos = (play.pos + 1) % play.frames.len();
                        frame
                    }
                    Some(_) => {
                        // The camera changed size under the loop on air: abandon everything and show live.
                        return Ok(self.finish(live.clone(), Mode::Live));
                    }
                    None => live.clone(),
                };
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
                Ok(self.finish(shown, next))
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
