use super::*;

fn px(v: u8) -> Frame {
    Frame::solid(2, 2, [v, v, v, 255]).unwrap()
}

fn recording(values: &[u8]) -> Vec<Frame> {
    values.iter().map(|&v| px(v)).collect()
}

fn blue(f: &Frame) -> u8 {
    f.pixel(0, 0).unwrap()[0]
}

#[test]
fn a_ten_frame_recording_with_a_two_frame_dissolve_makes_an_eight_frame_loop() {
    let out = build_loop(&recording(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 2).unwrap();
    assert_eq!(out.len(), 8);
}

#[test]
fn the_first_loop_frames_are_the_tail_dissolving_into_the_head() {
    // alpha(0,2) = 85, alpha(1,2) = 170.
    // (90 * 171 + 10 * 85 + 128) >> 8 = 63   and   (100 * 86 + 20 * 170 + 128) >> 8 = 47
    let out = build_loop(&recording(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 2).unwrap();
    assert_eq!(blue(&out[0]), 63);
    assert_eq!(blue(&out[1]), 47);
}

#[test]
fn the_rest_of_the_loop_is_the_recording_unchanged() {
    let out = build_loop(&recording(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]), 2).unwrap();
    let rest: Vec<u8> = out[2..].iter().map(blue).collect();
    assert_eq!(rest, vec![30, 40, 50, 60, 70, 80]);
}

#[test]
fn the_dissolve_ramp_is_strictly_increasing_and_never_reaches_either_end() {
    for k in 1..40usize {
        let alphas: Vec<u16> = (0..k).map(|i| seam_alpha(i, k)).collect();
        assert!(alphas[0] > 0, "k={k}");
        assert!(*alphas.last().unwrap() < ALPHA_ONE, "k={k}");
        assert!(alphas.windows(2).all(|w| w[0] < w[1]), "k={k}: {alphas:?}");
    }
}

#[test]
fn a_dissolve_longer_than_half_the_recording_is_shortened() {
    // 5 frames, 10 requested: only 2 can be used, so the loop is 3 frames long.
    assert_eq!(crossfade_len(5, 10), 2);
    assert_eq!(
        build_loop(&recording(&[1, 2, 3, 4, 5]), 10).unwrap().len(),
        3
    );
}

#[test]
fn no_dissolve_means_the_loop_is_the_recording() {
    let rec = recording(&[5, 6, 7]);
    assert_eq!(build_loop(&rec, 0).unwrap(), rec);
}

#[test]
fn recordings_of_one_or_two_frames_still_make_a_valid_loop() {
    assert_eq!(build_loop(&recording(&[9]), 12).unwrap(), recording(&[9]));
    let two = build_loop(&recording(&[0, 200]), 12).unwrap(); // k = 1, loop length 1
    assert_eq!(two.len(), 1);
    assert_eq!(blue(&two[0]), 100); // halfway between the tail (200) and the head (0)
}

#[test]
fn the_loop_is_never_empty_and_never_longer_than_the_recording() {
    for n in 1..30usize {
        for requested in [0u32, 1, 2, 5, 12, 100] {
            let rec: Vec<Frame> = (0..n).map(|i| px(i as u8)).collect();
            let out = build_loop(&rec, requested).unwrap();
            assert!(
                !out.is_empty() && out.len() <= n,
                "n={n} requested={requested}"
            );
        }
    }
}

#[test]
fn the_wrap_around_flows_from_the_last_loop_frame_into_a_frame_close_to_the_tail() {
    // Recording is a ramp: frame i has value 10 * i. The loop's last frame is recording frame n-k-1 and the loop's
    // first frame is the tail dissolved into the head, so the step across the wrap is small (about one frame).
    let values: Vec<u8> = (0..12).map(|i| i * 10).collect();
    let out = build_loop(&recording(&values), 3).unwrap();
    let last = blue(out.last().unwrap()) as i32; // frame 8 = 80
    let first = blue(&out[0]) as i32; // tail frame 9 (90) pulled a quarter of the way towards the head (0)
    assert!(
        (first - last).abs() <= 30,
        "jump across the wrap was {}",
        (first - last).abs()
    );
}

#[test]
fn an_empty_recording_and_mixed_sizes_are_errors() {
    assert_eq!(build_loop(&[], 2), Err(SeamError::Empty));
    let mixed = vec![px(1), Frame::solid(3, 3, [0; 4]).unwrap()];
    assert_eq!(build_loop(&mixed, 2), Err(SeamError::SizeMismatch));
}
