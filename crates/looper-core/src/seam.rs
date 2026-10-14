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
