use super::*;

fn px(v: u8) -> Frame {
    Frame::solid(2, 2, [v, v, v, 255]).unwrap()
}

#[test]
fn alpha_zero_gives_exactly_the_first_frame() {
    let (a, b) = (px(10), px(200));
    assert_eq!(blend(&a, &b, 0).unwrap(), a);
}

#[test]
fn alpha_one_gives_exactly_the_second_frame() {
    let (a, b) = (px(10), px(200));
    assert_eq!(blend(&a, &b, ALPHA_ONE).unwrap(), b);
}

#[test]
fn the_midpoint_of_black_and_white_is_mid_grey() {
    let out = blend(&px(0), &px(255), 128).unwrap();
    assert_eq!(out.pixel(0, 0), Some([128, 128, 128, 255]));
}

#[test]
fn blending_identical_frames_changes_nothing_at_any_alpha() {
    let a = px(77);
    for alpha in 0..=ALPHA_ONE {
        assert_eq!(blend(&a, &a, alpha).unwrap(), a, "alpha {alpha}");
    }
}

#[test]
fn every_channel_is_blended_independently() {
    let a = Frame::solid(1, 1, [0, 100, 200, 255]).unwrap();
    let b = Frame::solid(1, 1, [255, 100, 0, 255]).unwrap();
    let out = blend(&a, &b, 128).unwrap();
    assert_eq!(out.pixel(0, 0), Some([128, 100, 100, 255]));
}

#[test]
fn the_result_never_overflows_a_byte() {
    let out = blend(&px(255), &px(255), 255).unwrap();
    assert_eq!(out.pixel(0, 0), Some([255, 255, 255, 255]));
}

#[test]
fn increasing_alpha_never_moves_a_channel_backwards() {
    let (a, b) = (px(10), px(250));
    let mut last = 0u8;
    for alpha in 0..=ALPHA_ONE {
        let v = blend(&a, &b, alpha).unwrap().pixel(0, 0).unwrap()[0];
        assert!(v >= last, "alpha {alpha}: {v} < {last}");
        last = v;
    }
}

#[test]
fn frames_of_different_sizes_cannot_be_blended() {
    let a = Frame::solid(2, 2, [0; 4]).unwrap();
    let b = Frame::solid(2, 3, [0; 4]).unwrap();
    assert_eq!(blend(&a, &b, 100), Err(BlendError::SizeMismatch));
    let mut dst = Frame::solid(3, 3, [0; 4]).unwrap();
    assert_eq!(
        blend_into(&mut dst, &a, &a, 100),
        Err(BlendError::SizeMismatch)
    );
}

#[test]
fn an_alpha_above_the_scale_is_rejected() {
    let a = px(1);
    assert_eq!(
        blend(&a, &a, ALPHA_ONE + 1),
        Err(BlendError::AlphaOutOfRange)
    );
}

#[test]
fn blend_into_overwrites_the_destination_completely() {
    let mut dst = px(99);
    blend_into(&mut dst, &px(10), &px(20), 0).unwrap();
    assert_eq!(dst, px(10));
}
