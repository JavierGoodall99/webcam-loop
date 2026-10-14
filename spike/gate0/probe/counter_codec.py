"""Machine-readable frame-counter strip shared by the test pattern, the producer and the probe.

Layout: BITS blocks of BLOCK x BLOCK pixels at the top-left, MSB first.
First 16 bits are a marker (LIVE or SLATE), the next 32 are a frame counter.
White block = 1, black block = 0.
"""
import numpy as np

BLOCK = 8
BITS = 48
MARKER_LIVE = 0xA5C3
MARKER_SLATE = 0x5A3C
_VALID_MARKERS = (MARKER_LIVE, MARKER_SLATE)


def encode_strip(counter: int, marker: int = MARKER_LIVE) -> np.ndarray:
    value = (marker << 32) | (counter & 0xFFFFFFFF)
    strip = np.zeros((BLOCK, BLOCK * BITS), dtype=np.uint8)
    for i in range(BITS):
        if (value >> (BITS - 1 - i)) & 1:
            strip[:, i * BLOCK:(i + 1) * BLOCK] = 255
    return strip


def decode_strip(image: np.ndarray):
    """Return (marker, counter) or None if the image is too small or the marker is unknown."""
    if image.ndim == 3:
        gray = image.mean(axis=2)
    else:
        gray = image.astype(np.float64)
    if gray.shape[0] < BLOCK or gray.shape[1] < BLOCK * BITS:
        return None
    row = BLOCK // 2
    value = 0
    for i in range(BITS):
        bit = 1 if gray[row, i * BLOCK + BLOCK // 2] >= 128 else 0
        value = (value << 1) | bit
    marker = value >> 32
    if marker not in _VALID_MARKERS:
        return None
    return marker, value & 0xFFFFFFFF
