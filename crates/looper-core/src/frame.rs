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
