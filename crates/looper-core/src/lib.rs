//! Pure-logic core of the webcam loop product. No I/O, no clock, no dependencies.
//!
//! Frames go in, frames come out: [`looper::Looper`] decides, one live frame at a time, what the virtual camera shows.

pub mod codec;
pub mod compositor;
pub mod frame;

pub use codec::{Codec, CodecError, RawCodec};
pub use compositor::{blend, blend_into, BlendError, ALPHA_ONE};
pub use frame::{Frame, FrameError};
