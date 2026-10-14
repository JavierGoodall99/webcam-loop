"""Generate cross-language fixtures for the browser probe: one pixel row (row 1) of strips made by the Python encoder.

Run:  python make_fixtures.py   (writes ../browser-probe/fixtures.json)
The JavaScript decoder (browser-probe/strip-codec.mjs) must decode each case to the same marker and counter.
"""
import base64
import json
import pathlib

import cv2
import numpy as np

from counter_codec import MARKER_LIVE, MARKER_SLATE, encode_strip

CASES = [
    ("native-live", 123456789, MARKER_LIVE, 1.0),
    ("native-slate", 7, MARKER_SLATE, 1.0),
    ("native-zero", 0, MARKER_LIVE, 1.0),
    ("scaled-half", 31337, MARKER_LIVE, 0.5),
    ("scaled-one-and-a-half", 4294967295, MARKER_LIVE, 1.5),
]


def main():
    out = []
    for name, counter, marker, scale in CASES:
        img = np.full((720, 1280, 3), 90, dtype=np.uint8)
        strip = encode_strip(counter, marker)
        img[: strip.shape[0], : strip.shape[1], :] = strip[:, :, None]
        if scale != 1.0:
            img = cv2.resize(img, None, fx=scale, fy=scale, interpolation=cv2.INTER_LINEAR)
        row = img[1]                                              # BGR row
        rgba = np.dstack([row[:, 2], row[:, 1], row[:, 0], np.full(row.shape[0], 255, np.uint8)])[0]
        out.append({"name": name, "width": int(row.shape[0]), "marker": marker, "counter": counter,
                    "rowRgbaBase64": base64.b64encode(rgba.tobytes()).decode("ascii")})
    dest = pathlib.Path(__file__).resolve().parents[1] / "browser-probe" / "fixtures.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps({"cases": out}), encoding="utf-8")
    print(f"wrote {dest} ({len(out)} cases)")


if __name__ == "__main__":
    main()
