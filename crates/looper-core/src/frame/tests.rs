use super::*;

#[test]
fn from_bgra_accepts_data_of_exactly_the_right_length() {
    let f = Frame::from_bgra(2, 3, vec![7; 2 * 3 * 4]).unwrap();
    assert_eq!((f.width(), f.height()), (2, 3));
    assert_eq!(f.data().len(), 24);
}

#[test]
fn from_bgra_rejects_wrong_lengths_and_says_what_was_expected() {
    assert_eq!(
        Frame::from_bgra(2, 2, vec![0; 15]),
        Err(FrameError::WrongLength {
            expected: 16,
            actual: 15
        })
    );
    assert_eq!(
        Frame::from_bgra(2, 2, vec![0; 17]),
        Err(FrameError::WrongLength {
            expected: 16,
            actual: 17
        })
    );
}

#[test]
fn zero_sized_frames_are_rejected() {
    assert_eq!(Frame::solid(0, 5, [0; 4]), Err(FrameError::ZeroSize));
    assert_eq!(Frame::solid(5, 0, [0; 4]), Err(FrameError::ZeroSize));
    assert_eq!(Frame::from_bgra(0, 0, vec![]), Err(FrameError::ZeroSize));
}

#[test]
fn absurdly_large_frames_are_rejected_without_allocating() {
    assert_eq!(Frame::byte_len(100_000, 100_000), Err(FrameError::TooLarge));
    assert_eq!(
        Frame::byte_len(u32::MAX, u32::MAX),
        Err(FrameError::TooLarge)
    );
    assert!(Frame::solid(100_000, 100_000, [0; 4]).is_err());
}

#[test]
fn a_720p_and_an_8k_frame_are_allowed() {
    assert_eq!(Frame::byte_len(1280, 720), Ok(1280 * 720 * 4));
    assert!(Frame::byte_len(7680, 4320).is_ok());
}

#[test]
fn solid_fills_every_pixel_including_odd_sizes() {
    for (w, h) in [(1, 1), (3, 5), (7, 1), (1, 9)] {
        let f = Frame::solid(w, h, [1, 2, 3, 255]).unwrap();
        assert_eq!(f.data().len(), (w * h * 4) as usize);
        assert!(f.data().chunks(4).all(|p| p == [1, 2, 3, 255]));
    }
}

#[test]
fn pixel_reads_by_column_and_row_and_is_none_outside() {
    let mut data = vec![0u8; 3 * 2 * 4];
    let at = (3 + 2) * 4; // column 2 of row 1 in a 3-pixel-wide frame
    data[at..at + 4].copy_from_slice(&[9, 8, 7, 6]);
    let f = Frame::from_bgra(3, 2, data).unwrap();
    assert_eq!(f.pixel(2, 1), Some([9, 8, 7, 6]));
    assert_eq!(f.pixel(0, 0), Some([0, 0, 0, 0]));
    assert_eq!(f.pixel(3, 0), None);
    assert_eq!(f.pixel(0, 2), None);
}

#[test]
fn same_size_compares_dimensions_not_content() {
    let a = Frame::solid(2, 2, [0; 4]).unwrap();
    let b = Frame::solid(2, 2, [255; 4]).unwrap();
    let c = Frame::solid(2, 3, [0; 4]).unwrap();
    assert!(a.same_size(&b));
    assert!(!a.same_size(&c));
}

#[test]
fn data_mut_changes_pixels_but_never_the_length() {
    let mut f = Frame::solid(2, 2, [0; 4]).unwrap();
    f.data_mut()[0] = 42;
    assert_eq!(f.pixel(0, 0), Some([42, 0, 0, 0]));
    assert_eq!(f.data().len(), 16);
}
