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
