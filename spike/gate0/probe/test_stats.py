from counter_codec import MARKER_LIVE, MARKER_SLATE
from stats import gate_failures, summarize

L = MARKER_LIVE


def steady(seconds=3, fps=30, start=0):
    return [(i / fps, L, start + i) for i in range(int(seconds * fps))]


def test_steady_stream_is_clean():
    s = summarize(steady())
    assert s["dropped"] == 0 and s["repeats"] == 0 and s["freeze_events"] == 0
    assert s["invalid_frames"] == 0
    assert 29 <= s["fps"] <= 31
    assert gate_failures(s) == []


def test_dropped_frames_are_counted_from_counter_jumps():
    samples = steady(1)
    samples = samples[:10] + [(t, m, c + 5) for (t, m, c) in samples[10:]]
    assert summarize(samples)["dropped"] == 5


def test_freeze_is_detected():
    base = steady(1)
    frozen = [(1.0 + i * 0.1, L, 29) for i in range(25)]   # same counter for 2.5 s
    s = summarize(base + frozen)
    assert s["freeze_events"] == 1
    assert "freeze" in " ".join(gate_failures(s))


def test_slate_and_invalid_frames_are_counted_separately():
    samples = steady(1) + [(1.0, MARKER_SLATE, 0), (1.03, None, None)]
    s = summarize(samples)
    assert s["slate_frames"] == 1 and s["invalid_frames"] == 1


def test_large_gap_fails_the_gate():
    samples = steady(1) + [(2.0, L, 30), (2.03, L, 31)]
    s = summarize(samples)
    assert s["max_gap_s"] > 0.5
    assert any("gap" in f for f in gate_failures(s))


def test_empty_input_is_safe():
    s = summarize([])
    assert s["frames"] == 0 and s["fps"] == 0.0


def test_dropped_check_can_be_turned_off_for_sources_that_overgenerate():
    # The built-in pattern advances its counter on every sample request (~45/s) while only 30 fps are delivered,
    # so counter gaps there are not lost frames. Delivery is still judged by fps, gap and freeze.
    over = [(i / 30, L, i * 3) for i in range(90)]
    s = summarize(over)
    assert s["dropped"] > 0
    assert any("dropped" in f for f in gate_failures(s))
    assert gate_failures(s, check_drops=False) == []
