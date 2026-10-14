import numpy as np
import pytest

from counter_codec import (
    BITS, BLOCK, MARKER_LIVE, MARKER_SLATE, decode_strip, encode_strip,
)


def canvas(strip, w=1280, h=720, channels=3):
    img = np.full((h, w, channels), 90, dtype=np.uint8)
    img[: strip.shape[0], : strip.shape[1], :] = strip[:, :, None]
    return img


@pytest.mark.parametrize("counter", [0, 1, 255, 65535, 123456789, 0xFFFFFFFF])
def test_roundtrip_live(counter):
    assert decode_strip(canvas(encode_strip(counter))) == (MARKER_LIVE, counter)


def test_roundtrip_slate_marker():
    assert decode_strip(canvas(encode_strip(7, MARKER_SLATE))) == (MARKER_SLATE, 7)


def test_survives_noise_and_grayscale_input():
    rng = np.random.default_rng(1)
    img = canvas(encode_strip(4242)).astype(np.int16)
    img += rng.integers(-25, 26, img.shape)
    img = np.clip(img, 0, 255).astype(np.uint8)
    assert decode_strip(img) == (MARKER_LIVE, 4242)
    assert decode_strip(img[:, :, 0]) == (MARKER_LIVE, 4242)


def test_too_small_image_returns_none():
    assert decode_strip(np.zeros((BLOCK - 1, BLOCK * BITS, 3), np.uint8)) is None
    assert decode_strip(np.zeros((BLOCK, BLOCK * BITS - 1, 3), np.uint8)) is None


def test_unknown_marker_returns_none():
    rng = np.random.default_rng(2)
    noise = rng.integers(0, 256, (720, 1280, 3), dtype=np.uint8)
    assert decode_strip(noise) is None


def test_decoding_reads_only_the_strip_rows_so_the_probe_keeps_up_with_30fps():
    import time
    img = canvas(encode_strip(99), w=1920, h=1080)
    start = time.perf_counter()
    for _ in range(300):
        assert decode_strip(img) == (MARKER_LIVE, 99)
    per_frame_ms = (time.perf_counter() - start) / 300 * 1000
    assert per_frame_ms < 2.0, f"decode takes {per_frame_ms:.1f} ms per 1080p frame"


@pytest.mark.parametrize("scale,interp", [(0.5, "area"), (0.75, "linear"), (1.5, "linear"), (0.5, "linear")])
def test_decodes_a_strip_that_the_camera_pipeline_scaled(scale, interp):
    # The Frame Server renders one native size and scales it for clients that ask for another.
    import cv2
    mode = cv2.INTER_AREA if interp == "area" else cv2.INTER_LINEAR
    img = canvas(encode_strip(31337), w=1280, h=720)
    scaled = cv2.resize(img, None, fx=scale, fy=scale, interpolation=mode)
    assert decode_strip(scaled) == (MARKER_LIVE, 31337)
