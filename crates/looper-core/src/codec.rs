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
