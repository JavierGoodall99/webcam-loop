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
