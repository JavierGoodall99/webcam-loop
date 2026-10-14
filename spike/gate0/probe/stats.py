"""Summarise a stream of probe samples (t_seconds, marker_or_None, counter_or_None)."""
from counter_codec import MARKER_SLATE

MAX_GAP_S = 0.5
MAX_DROP_FRACTION = 0.01
MIN_FPS = 27.0


def summarize(samples, freeze_threshold_s=1.0):
    out = dict(frames=len(samples), duration_s=0.0, fps=0.0, max_gap_s=0.0,
               dropped=0, repeats=0, freeze_events=0, slate_frames=0,
               invalid_frames=0, counter_resets=0)
    if not samples:
        return out
    out["duration_s"] = samples[-1][0] - samples[0][0]
    if len(samples) > 1 and out["duration_s"] > 0:
        out["fps"] = (len(samples) - 1) / out["duration_s"]
        out["max_gap_s"] = max(b[0] - a[0] for a, b in zip(samples, samples[1:]))

    prev = None            # previous LIVE sample
    run_start_t = None     # start time of the current identical-counter run
    run_flagged = False
    for t, marker, counter in samples:
        if marker is None:
            out["invalid_frames"] += 1
            prev, run_start_t, run_flagged = None, None, False
            continue
        if marker == MARKER_SLATE:
            out["slate_frames"] += 1
            prev, run_start_t, run_flagged = None, None, False
            continue
        # LIVE frame
        if prev is not None:
            delta = counter - prev[2]
            if delta > 1:
                out["dropped"] += delta - 1
            elif delta == 0:
                out["repeats"] += 1
            elif delta < 0:
                out["counter_resets"] += 1
            if delta == 0:
                if run_start_t is None:
                    run_start_t = prev[0]
                if not run_flagged and t - run_start_t >= freeze_threshold_s:
                    out["freeze_events"] += 1
                    run_flagged = True
            else:
                run_start_t, run_flagged = None, False
        prev = (t, marker, counter)
    return out


def gate_failures(s):
    fails = []
    if s["freeze_events"] > 0:
        fails.append(f"freeze events: {s['freeze_events']}")
    if s["max_gap_s"] > MAX_GAP_S:
        fails.append(f"max gap {s['max_gap_s']:.2f}s > {MAX_GAP_S}s")
    expected = s["frames"] + s["dropped"]
    if expected and s["dropped"] / expected > MAX_DROP_FRACTION:
        fails.append(f"dropped {s['dropped']}/{expected} > {MAX_DROP_FRACTION:.0%}")
    if s["invalid_frames"] > 0:
        fails.append(f"undecodable frames: {s['invalid_frames']}")
    if s["frames"] > 1 and s["fps"] < MIN_FPS:
        fails.append(f"fps {s['fps']:.1f} < {MIN_FPS}")
    return fails
