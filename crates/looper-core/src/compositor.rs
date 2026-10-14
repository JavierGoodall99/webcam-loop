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
