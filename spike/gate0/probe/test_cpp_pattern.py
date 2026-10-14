"""Cross-check: the C++ test pattern (gate0/TestPattern.h) must decode with the Python codec.

Runs the built PatternDump.exe, which writes raw BGRA frames, then decodes them directly and after a simulated
NV12 round trip (BT.601 limited range, same integer maths as the sample's converter).
"""
import pathlib
import subprocess

import cv2
import numpy as np
import pytest

from counter_codec import MARKER_LIVE, MARKER_SLATE, decode_strip

EXE = pathlib.Path(__file__).resolve().parents[1] / "vcam-sample" / "gate0" / "PatternDump.exe"


def dump(tmp_path, w, h, counter, marker):
    out = tmp_path / "frame.bgra"
    subprocess.run([str(EXE), str(w), str(h), str(counter), str(marker), str(out)], check=True)
    return np.fromfile(out, dtype=np.uint8).reshape(h, w, 4)


def bgra_to_nv12_and_back(bgra):
    h, w = bgra.shape[:2]
    b, g, r = (bgra[:, :, i].astype(np.int32) for i in range(3))
    y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16
    u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128
    v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128
    uv = np.empty((h // 2, w), dtype=np.uint8)
    uv[:, 0::2] = np.clip(u[1::2, 0::2], 0, 255)     # sample converter keeps the bottom row's chroma of each 2x2
    uv[:, 1::2] = np.clip(v[1::2, 0::2], 0, 255)
    nv12 = np.vstack([np.clip(y, 0, 255).astype(np.uint8), uv])
    return cv2.cvtColor(nv12, cv2.COLOR_YUV2BGR_NV12)


@pytest.mark.parametrize("w,h", [(1280, 720), (640, 360), (1920, 1080), (640, 480)])
def test_live_strip_decodes_at_every_offered_size(tmp_path, w, h):
    frame = dump(tmp_path, w, h, 123456789, MARKER_LIVE)
    assert decode_strip(frame[:, :, :3]) == (MARKER_LIVE, 123456789)


def test_slate_strip_decodes_and_is_flat_grey(tmp_path):
    frame = dump(tmp_path, 1280, 720, 5, MARKER_SLATE)
    assert decode_strip(frame[:, :, :3]) == (MARKER_SLATE, 5)
    body = frame[16:, :, :3]
    assert body.min() == body.max() == 40


def test_strip_survives_nv12_round_trip(tmp_path):
    frame = dump(tmp_path, 1280, 720, 987654321, MARKER_LIVE)
    assert decode_strip(bgra_to_nv12_and_back(frame)) == (MARKER_LIVE, 987654321)


def test_sweep_line_moves_between_frames(tmp_path):
    a = dump(tmp_path, 1280, 720, 10, MARKER_LIVE)
    b = dump(tmp_path, 1280, 720, 11, MARKER_LIVE)
    assert not np.array_equal(a[100], b[100])
