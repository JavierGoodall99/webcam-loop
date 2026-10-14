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
