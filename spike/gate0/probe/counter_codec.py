"""Machine-readable frame-counter strip shared by the test pattern, the producer and the probe.

Layout: BITS blocks of BLOCK x BLOCK pixels at the top-left, MSB first.
First 16 bits are a marker (LIVE or SLATE), the next 32 are a frame counter.
White block = 1, black block = 0.

The camera pipeline may scale the picture (the Frame Server renders one native size and scales it for clients that
ask for another), so decoding tries a few block sizes and accepts the first that yields a valid marker.
"""
import numpy as np

BLOCK = 8
BITS = 48
MARKER_LIVE = 0xA5C3
MARKER_SLATE = 0x5A3C
_VALID_MARKERS = (MARKER_LIVE, MARKER_SLATE)
# Native size first (the common case), then the sizes produced by common scalings (0.5x -> 4, 1.5x -> 12, 0.75x -> 6, 2x -> 16).
_CANDIDATE_BLOCKS = (8, 4, 12, 6, 16, 2, 3, 5, 7, 9, 10, 11, 13, 14, 15)


def encode_strip(counter: int, marker: int = MARKER_LIVE) -> np.ndarray:
    value = (marker << 32) | (counter & 0xFFFFFFFF)
    strip = np.zeros((BLOCK, BLOCK * BITS), dtype=np.uint8)
    for i in range(BITS):
        if (value >> (BITS - 1 - i)) & 1:
            strip[:, i * BLOCK:(i + 1) * BLOCK] = 255
    return strip


def _read_value(image: np.ndarray, block: int):
    cols = ((np.arange(BITS) + 0.5) * block).astype(int)
    if cols[-1] >= image.shape[1]:
        return None
    pixels = image[1, cols]            # row 1 lies inside the strip even when the picture is scaled down to a quarter
    gray = pixels.mean(axis=1) if pixels.ndim == 2 else pixels.astype(np.float64)
    value = 0
    for g in gray:
        value = (value << 1) | (1 if g >= 128 else 0)
    return value


def decode_strip(image: np.ndarray):
    """Return (marker, counter), or None if no block size yields a known marker (too small, noise, or no strip)."""
    if image.shape[0] < 2:
        return None
    for block in _CANDIDATE_BLOCKS:
        value = _read_value(image, block)
        if value is None:
            continue
        marker = value >> 32
        if marker in _VALID_MARKERS:
            return marker, value & 0xFFFFFFFF
    return None
