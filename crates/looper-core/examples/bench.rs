//! Rough timing of the per-frame work at 720p. Run with: cargo run --release --example bench
//! The budget at 30 fps is 33 ms per frame for everything; this only measures the looper's share.

use looper_core::{blend_into, Config, Frame, Looper, RawCodec, State};
use std::time::Instant;

const W: u32 = 1280;
const H: u32 = 720;
const FRAMES: usize = 300;

fn time_ms(label: &str, mut f: impl FnMut()) {
    f(); // warm up
    let start = Instant::now();
    for _ in 0..FRAMES {
        f();
    }
    let per = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
    println!("{label:<44} {per:6.2} ms/frame");
}

fn main() {
    let a = Frame::solid(W, H, [10, 20, 30, 255]).unwrap();
    let b = Frame::solid(W, H, [200, 150, 100, 255]).unwrap();
    let mut dst = a.clone();
    time_ms("blend_into 1280x720", || {
        blend_into(&mut dst, &a, &b, 100).unwrap()
    });

    let config = Config::for_fps(30).unwrap();

    let mut live = Looper::new(config, RawCodec).unwrap();
    time_ms("process: live pass-through (clone)", || {
        live.process(&a).unwrap();
    });

    let mut rec = Looper::new(config, RawCodec).unwrap();
    rec.pedal_down();
    let mut n = 0;
    time_ms("process: recording (raw encode)", || {
        if n < 200 {
            rec.process(&a).unwrap();
            n += 1;
        }
    });

    let mut lp = Looper::new(config, RawCodec).unwrap();
    lp.pedal_down();
    for _ in 0..60 {
        lp.process(&a).unwrap();
    }
    lp.pedal_up().unwrap();
    assert_eq!(lp.state(), State::Loop);
    time_ms("process: loop playback (raw decode)", || {
        lp.process(&b).unwrap();
    });

    let raw_mb = (W * H * 4) as f64 / 1e6;
    println!(
        "\nstored raw: {raw_mb:.1} MB per frame; {:.0} MB for the default 20 s at 30 fps",
        raw_mb * 600.0
    );
}
