use super::*;
use crate::codec::RawCodec;
use crate::seam::build_loop;

fn cfg(min: u32, max: u32, cross: u32) -> Config {
    Config {
        fps: 30,
        crossfade_frames: cross,
        min_record_frames: min,
        max_record_frames: max,
        max_record_bytes: usize::MAX,
    }
}

fn px(v: u8) -> Frame {
    Frame::solid(2, 2, [v, v, v, 255]).unwrap()
}

fn blue(f: &Frame) -> u8 {
    f.pixel(0, 0).unwrap()[0]
}

fn looper(min: u32, max: u32, cross: u32) -> Looper<RawCodec> {
    Looper::new(cfg(min, max, cross), RawCodec).unwrap()
}

/// Feeds one live frame per value (each a flat grey of that value) and returns the outputs.
fn feed(l: &mut Looper<RawCodec>, values: &[u8]) -> Vec<Output> {
    values.iter().map(|&v| l.process(&px(v)).unwrap()).collect()
}

fn blues(outs: &[Output]) -> Vec<u8> {
    outs.iter().map(|o| blue(&o.frame)).collect()
}

/// Holds the pedal while `values` go by, then releases it.
fn record(l: &mut Looper<RawCodec>, values: &[u8]) {
    l.pedal_down();
    feed(l, values);
    l.pedal_up().unwrap();
}

/// Independent copy of the blend maths, so the tests do not just call the code under test.
fn mix(a: u8, b: u8, alpha: u32) -> u8 {
    ((a as u32 * (256 - alpha) + b as u32 * alpha + 128) >> 8) as u8
}

const TEN: [u8; 10] = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

#[test]
fn it_starts_live_and_passes_frames_through_untouched() {
    let mut l = looper(3, 100, 2);
    assert_eq!(l.state(), State::Live);
    let out = l.process(&px(42)).unwrap();
    assert_eq!(
        out,
        Output {
            frame: px(42),
            state: State::Live
        }
    );
}

#[test]
fn pressing_the_pedal_starts_recording_and_the_output_stays_live() {
    let mut l = looper(3, 100, 2);
    l.pedal_down();
    assert_eq!(l.state(), State::Recording);
    let outs = feed(&mut l, &[1, 2, 3, 4]);
    assert_eq!(blues(&outs), vec![1, 2, 3, 4]);
    assert!(outs.iter().all(|o| o.state == State::Recording));
    assert_eq!(l.recorded_frames(), 4);
}

#[test]
fn a_hold_shorter_than_the_minimum_is_ignored() {
    let mut l = looper(4, 100, 2);
    record(&mut l, &[1, 2, 3]);
    assert_eq!(l.state(), State::Live);
    assert_eq!(l.loop_len(), 0);
    assert_eq!(blues(&feed(&mut l, &[9])), vec![9]);
}

#[test]
fn a_hold_of_exactly_the_minimum_makes_a_loop() {
    let mut l = looper(3, 100, 0);
    record(&mut l, &[1, 2, 3]);
    assert_eq!(l.state(), State::Loop);
    assert_eq!(l.loop_len(), 3);
}

#[test]
fn releasing_the_pedal_with_nothing_recorded_does_nothing() {
    let mut l = looper(1, 100, 0);
    l.pedal_down();
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Live);
}

#[test]
fn the_loop_opens_with_the_tail_dissolving_into_the_head_then_plays_the_rest_and_wraps() {
    let mut l = looper(3, 100, 2);
    record(&mut l, &TEN);
    assert_eq!(l.state(), State::Loop);
    assert_eq!(l.loop_len(), 8);
    // First two frames dissolve the tail (90, 100) into the head (10, 20); then 30..80; then it wraps.
    let outs = feed(&mut l, &[250; 18]);
    let expected = [
        63, 47, 30, 40, 50, 60, 70, 80, 63, 47, 30, 40, 50, 60, 70, 80, 63, 47,
    ];
    assert_eq!(blues(&outs), expected);
    assert!(outs.iter().all(|o| o.state == State::Loop));
}

#[test]
fn the_looper_builds_exactly_the_loop_the_pure_seam_builder_does() {
    let mut l = looper(3, 100, 3);
    record(&mut l, &TEN);
    let frames: Vec<Frame> = TEN.iter().map(|&v| px(v)).collect();
    let reference: Vec<u8> = build_loop(&frames, 3).unwrap().iter().map(blue).collect();
    let played = blues(&feed(&mut l, &vec![0; reference.len()]));
    assert_eq!(played, reference);
}

#[test]
fn go_live_while_looping_dissolves_into_the_live_feed_then_goes_live() {
    let mut l = looper(3, 100, 2);
    record(&mut l, &[10, 20, 30, 40, 50, 60]); // loop of 4: [mix(50,10), mix(60,20), 30, 40]
    l.go_live();
    assert_eq!(l.state(), State::Returning);
    let outs = feed(&mut l, &[200, 200, 200, 200]);
    let loop0 = mix(50, 10, 85);
    let loop1 = mix(60, 20, 170);
    assert_eq!(blue(&outs[0].frame), mix(loop0, 200, 85));
    assert_eq!(blue(&outs[1].frame), mix(loop1, 200, 170));
    assert_eq!(outs[0].state, State::Returning);
    assert_eq!(outs[1].state, State::Live); // the dissolve's last frame is handled, and the looper is live again
    assert_eq!(
        outs[2],
        Output {
            frame: px(200),
            state: State::Live
        }
    );
    assert_eq!(
        outs[3],
        Output {
            frame: px(200),
            state: State::Live
        }
    );
}

#[test]
fn pressing_go_live_during_the_dissolve_does_nothing() {
    let mut l = looper(3, 100, 4);
    record(&mut l, &[10, 20, 30, 40, 50, 60, 70, 80]);
    l.go_live();
    feed(&mut l, &[200]);
    assert_eq!(l.state(), State::Returning);
    l.go_live();
    assert_eq!(l.state(), State::Returning);
    let outs = feed(&mut l, &[200, 200, 200]);
    assert_eq!(outs.last().unwrap().state, State::Live); // finished on schedule: 4 dissolve frames in all
}

#[test]
fn go_live_while_already_live_does_nothing() {
    let mut l = looper(3, 100, 2);
    l.go_live();
    assert_eq!(l.state(), State::Live);
    assert_eq!(blues(&feed(&mut l, &[5])), vec![5]);
}

#[test]
fn go_live_while_recording_cancels_the_recording() {
    let mut l = looper(3, 100, 2);
    l.pedal_down();
    feed(&mut l, &[1, 2, 3, 4, 5]);
    l.go_live();
    assert_eq!(l.state(), State::Live);
    l.pedal_up().unwrap(); // the release that follows the cancel must not resurrect anything
    assert_eq!(l.state(), State::Live);
    assert_eq!(l.loop_len(), 0);
    assert_eq!(blues(&feed(&mut l, &[7])), vec![7]);
}

#[test]
fn a_repeated_key_down_does_not_restart_the_recording() {
    let mut l = looper(3, 100, 0);
    l.pedal_down();
    feed(&mut l, &[1, 2]);
    l.pedal_down();
    feed(&mut l, &[3, 4]);
    assert_eq!(l.recorded_frames(), 4);
    l.pedal_up().unwrap();
    assert_eq!(l.loop_len(), 4);
}

#[test]
fn releasing_without_having_pressed_does_nothing_in_any_state() {
    let mut l = looper(3, 100, 0);
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Live);
    record(&mut l, &[1, 2, 3]);
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Loop);
    l.go_live();
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Live); // crossfade 0: go_live went straight to live
}

#[test]
fn recording_while_looping_replaces_the_loop() {
    let mut l = looper(3, 100, 0);
    record(&mut l, &[10, 20, 30, 40]);
    assert_eq!(l.state(), State::Loop);
    l.pedal_down();
    assert_eq!(l.state(), State::Recording);
    assert_eq!(blues(&feed(&mut l, &[100, 110, 120])), vec![100, 110, 120]); // live while recording
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Loop);
    assert_eq!(
        blues(&feed(&mut l, &[0; 7])),
        vec![100, 110, 120, 100, 110, 120, 100]
    ); // the old loop is gone
}

#[test]
fn pressing_record_during_the_dissolve_starts_a_recording() {
    let mut l = looper(3, 100, 4);
    record(&mut l, &[10, 20, 30, 40, 50, 60, 70, 80]);
    l.go_live();
    feed(&mut l, &[200]);
    l.pedal_down();
    assert_eq!(l.state(), State::Recording);
    assert_eq!(blues(&feed(&mut l, &[5])), vec![5]);
}

#[test]
fn hitting_the_frame_cap_ends_the_recording_and_starts_the_loop_by_itself() {
    let mut l = looper(2, 4, 0);
    l.pedal_down();
    let outs = feed(&mut l, &[1, 2, 3, 4, 5, 6]);
    assert_eq!(blues(&outs), vec![1, 2, 3, 4, 1, 2]); // frame 4 filled the buffer; frames 5 and 6 are loop playback
    assert_eq!(outs[2].state, State::Recording);
    assert_eq!(outs[3].state, State::Loop);
    l.pedal_up().unwrap(); // the late release is harmless
    assert_eq!(l.state(), State::Loop);
    assert_eq!(l.loop_len(), 4);
}

#[test]
fn hitting_the_memory_budget_also_ends_the_recording() {
    let config = Config {
        max_record_bytes: 3 * 16,
        ..cfg(1, 1000, 0)
    }; // a 2x2 frame is 16 bytes
    let mut l = Looper::new(config, RawCodec).unwrap();
    l.pedal_down();
    let outs = feed(&mut l, &[1, 2, 3, 4]);
    assert_eq!(outs[2].state, State::Loop);
    assert_eq!(l.loop_len(), 3);
}

#[test]
fn a_resolution_change_while_recording_abandons_the_recording_and_shows_live() {
    let mut l = looper(3, 100, 2);
    l.pedal_down();
    feed(&mut l, &[1, 2]);
    let bigger = Frame::solid(3, 3, [9, 9, 9, 255]).unwrap();
    let out = l.process(&bigger).unwrap();
    assert_eq!(
        out,
        Output {
            frame: bigger,
            state: State::Live
        }
    );
    assert_eq!(l.recorded_frames(), 0);
}

#[test]
fn a_resolution_change_while_looping_drops_the_loop_and_shows_live() {
    let mut l = looper(3, 100, 2);
    record(&mut l, &TEN);
    let bigger = Frame::solid(3, 3, [9, 9, 9, 255]).unwrap();
    let out = l.process(&bigger).unwrap();
    assert_eq!(
        out,
        Output {
            frame: bigger,
            state: State::Live
        }
    );
    assert_eq!(l.loop_len(), 0);
    assert_eq!(blues(&feed(&mut l, &[4])), vec![4]); // and it stays live at the old size too
}

#[test]
fn a_resolution_change_while_returning_goes_live_too() {
    let mut l = looper(3, 100, 4);
    record(&mut l, &[10, 20, 30, 40, 50, 60, 70, 80]);
    l.go_live();
    let bigger = Frame::solid(4, 1, [9, 9, 9, 255]).unwrap();
    let out = l.process(&bigger).unwrap();
    assert_eq!(out.state, State::Live);
    assert_eq!(out.frame, bigger);
}

#[test]
fn with_no_crossfade_the_loop_is_the_recording_and_the_return_is_instant() {
    let mut l = looper(3, 100, 0);
    record(&mut l, &[1, 2, 3, 4]);
    assert_eq!(blues(&feed(&mut l, &[0; 5])), vec![1, 2, 3, 4, 1]);
    l.go_live();
    assert_eq!(l.state(), State::Live);
    assert_eq!(blues(&feed(&mut l, &[77])), vec![77]);
}

#[test]
fn a_one_frame_recording_loops_that_one_frame() {
    let mut l = looper(1, 100, 12);
    record(&mut l, &[33]);
    assert_eq!(l.loop_len(), 1);
    assert_eq!(blues(&feed(&mut l, &[0; 4])), vec![33; 4]);
}

#[test]
fn a_two_frame_recording_with_a_long_crossfade_still_loops() {
    let mut l = looper(2, 100, 12);
    record(&mut l, &[0, 200]);
    assert_eq!(l.loop_len(), 1);
    assert_eq!(blues(&feed(&mut l, &[0; 3])), vec![100; 3]);
}

#[test]
fn a_recording_of_one_by_one_frames_works() {
    let mut l = Looper::new(cfg(2, 100, 1), RawCodec).unwrap();
    l.pedal_down();
    for v in [10u8, 20, 30, 40] {
        l.process(&Frame::solid(1, 1, [v, v, v, 255]).unwrap())
            .unwrap();
    }
    l.pedal_up().unwrap();
    let out = l
        .process(&Frame::solid(1, 1, [0, 0, 0, 255]).unwrap())
        .unwrap();
    assert_eq!(out.state, State::Loop);
    assert_eq!((out.frame.width(), out.frame.height()), (1, 1));
}

#[test]
fn an_invalid_config_is_refused() {
    assert!(Looper::new(
        Config {
            min_record_frames: 0,
            ..cfg(1, 10, 0)
        },
        RawCodec
    )
    .is_err());
    assert!(Looper::new(cfg(11, 10, 0), RawCodec).is_err());
}

/// A codec whose decoder always fails, to exercise the error paths.
struct BrokenDecoder;

impl Codec for BrokenDecoder {
    fn encode(&self, frame: &Frame) -> Vec<u8> {
        frame.data().to_vec()
    }

    fn decode(&self, _bytes: &[u8], _w: u32, _h: u32) -> Result<Frame, CodecError> {
        Err(CodecError::Corrupt)
    }
}

#[test]
fn a_decode_failure_during_playback_returns_the_error_and_falls_back_to_live() {
    let mut l = Looper::new(cfg(2, 100, 0), BrokenDecoder).unwrap(); // no crossfade: commit decodes nothing
    l.pedal_down();
    for v in [1u8, 2, 3] {
        l.process(&px(v)).unwrap();
    }
    l.pedal_up().unwrap();
    assert_eq!(l.state(), State::Loop);
    assert_eq!(
        l.process(&px(9)),
        Err(LooperError::Codec(CodecError::Corrupt))
    );
    assert_eq!(l.state(), State::Live);
    assert_eq!(
        l.process(&px(9)).unwrap(),
        Output {
            frame: px(9),
            state: State::Live
        }
    );
}

#[test]
fn a_decode_failure_while_building_the_loop_leaves_the_looper_live() {
    let mut l = Looper::new(cfg(2, 100, 2), BrokenDecoder).unwrap();
    l.pedal_down();
    for v in [1u8, 2, 3, 4] {
        l.process(&px(v)).unwrap();
    }
    assert_eq!(l.pedal_up(), Err(LooperError::Codec(CodecError::Corrupt)));
    assert_eq!(l.state(), State::Live);
}

/// Tiny deterministic random numbers, so the stress test needs no dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn random_sequences_of_events_never_panic_and_always_return_a_frame_of_the_live_size() {
    for seed in 0..60u64 {
        let mut rng = Lcg(seed + 1);
        let min = 1 + (seed % 3) as u32;
        let max = min + (seed % 6) as u32;
        let mut l = looper(min, max, (seed % 5) as u32);
        for _ in 0..3000 {
            match rng.next() % 10 {
                0 | 1 => l.pedal_down(),
                2 | 3 => l.pedal_up().unwrap(),
                4 => l.go_live(),
                _ => {
                    let (w, h) = match rng.next() % 12 {
                        0 => (3, 2),
                        1 => (1, 1),
                        _ => (2, 2),
                    };
                    let v = (rng.next() % 256) as u8;
                    let live = Frame::solid(w, h, [v, v, v, 255]).unwrap();
                    let out = l.process(&live).unwrap();
                    assert_eq!(
                        (out.frame.width(), out.frame.height()),
                        (w, h),
                        "seed {seed}"
                    );
                    assert_eq!(out.state, l.state(), "seed {seed}");
                }
            }
        }
    }
}
