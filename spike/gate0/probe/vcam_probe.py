"""Read the virtual camera for N minutes and report what it actually delivered.

Usage:  python vcam_probe.py --minutes 30 --out ..\\probe-run-zoom.md
        python vcam_probe.py --list
"""
import argparse
import json
import time

import cv2

from counter_codec import decode_strip
from stats import gate_failures, summarize


def open_cam(index, w, h, fps):
    cap = cv2.VideoCapture(index, cv2.CAP_MSMF)
    cap.set(cv2.CAP_PROP_FRAME_WIDTH, w)
    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, h)
    cap.set(cv2.CAP_PROP_FPS, fps)
    return cap


def find_virtual_camera(w, h, fps, max_index=10):
    for i in range(max_index):
        cap = open_cam(i, w, h, fps)
        try:
            for _ in range(15):
                ok, frame = cap.read()
                if ok and decode_strip(frame) is not None:
                    return i
        finally:
            cap.release()
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--minutes", type=float, default=1.0)
    ap.add_argument("--index", type=int, default=None)
    ap.add_argument("--width", type=int, default=1280)
    ap.add_argument("--height", type=int, default=720)
    ap.add_argument("--fps", type=int, default=30)
    ap.add_argument("--out", default=None, help="append a Markdown summary to this file")
    ap.add_argument("--label", default="run")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--no-drop-gate", action="store_true",
                    help="do not fail on counter gaps (built-in pattern advances its counter per sample request, ~45/s)")
    args = ap.parse_args()

    index = args.index if args.index is not None else find_virtual_camera(args.width, args.height, args.fps)
    if index is None:
        raise SystemExit("No camera index delivered a decodable counter strip.")
    if args.list:
        print(f"virtual camera index: {index}")
        return

    cap = open_cam(index, args.width, args.height, args.fps)
    actual = (int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)), int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)))
    samples = []
    t0 = time.monotonic()
    deadline = t0 + args.minutes * 60
    while time.monotonic() < deadline:
        ok, frame = cap.read()
        t = time.monotonic() - t0
        if not ok:
            samples.append((t, None, None))
            time.sleep(0.01)
            continue
        decoded = decode_strip(frame)
        samples.append((t, decoded[0], decoded[1]) if decoded else (t, None, None))
    cap.release()

    summary = summarize(samples)
    failures = gate_failures(summary, check_drops=not args.no_drop_gate)
    print(json.dumps({"label": args.label, "index": index, "size": actual, **summary, "failures": failures}, indent=2))
    if args.out:
        with open(args.out, "a", encoding="utf-8") as f:
            f.write(f"\n### {args.label}\n- camera index {index}, delivered size {actual[0]}x{actual[1]}\n")
            f.write("- " + ", ".join(f"{k}={v:.3f}" if isinstance(v, float) else f"{k}={v}" for k, v in summary.items()) + "\n")
            f.write(f"- gate: {'PASS' if not failures else 'FAIL — ' + '; '.join(failures)}\n")


if __name__ == "__main__":
    main()
