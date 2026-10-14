use super::*;

#[test]
fn raw_codec_round_trips_a_frame_exactly() {
    let f = Frame::from_bgra(3, 2, (0..24).collect()).unwrap();
    let bytes = RawCodec.encode(&f);
    assert_eq!(RawCodec.decode(&bytes, 3, 2).unwrap(), f);
}

#[test]
fn decoding_bytes_of_the_wrong_length_is_corrupt() {
    assert_eq!(RawCodec.decode(&[0; 23], 3, 2), Err(CodecError::Corrupt));
    assert_eq!(RawCodec.decode(&[0; 25], 3, 2), Err(CodecError::Corrupt));
    assert_eq!(RawCodec.decode(&[], 3, 2), Err(CodecError::Corrupt));
}

#[test]
fn decoding_into_a_zero_sized_frame_is_corrupt_not_a_panic() {
    assert_eq!(RawCodec.decode(&[], 0, 0), Err(CodecError::Corrupt));
}

#[test]
fn encoded_size_is_the_pixel_byte_count() {
    let f = Frame::solid(4, 4, [1, 2, 3, 4]).unwrap();
    assert_eq!(RawCodec.encode(&f).len(), 64);
}
