# Probe runs

Each entry is appended by vcam_probe.py. Two earlier runs were discarded: the first was limited by the probe itself (about 21 fps; fixed test-first), the second was judged on counter gaps, which are meaningless for the built-in pattern (it over-generates ~45 counter values/s while 30 fps are delivered; see --no-drop-gate).

### pattern-1280x720-2min
- camera index 1, delivered size 1280x720
- frames=3600, duration_s=119.896, fps=30.018, max_gap_s=0.104, dropped=1381, repeats=20, freeze_events=0, slate_frames=0, invalid_frames=0, counter_resets=0
- gate: PASS

### pattern-640x360-1min
- camera index 1, delivered size 640x360
- frames=1796, duration_s=59.918, fps=29.958, max_gap_s=0.201, dropped=0, repeats=0, freeze_events=0, slate_frames=0, invalid_frames=1796, counter_resets=0
- gate: FAIL — undecodable frames: 1796

### pattern-640x360-1min (scaled by pipeline)
- camera index 1, delivered size 640x360
- frames=1800, duration_s=59.939, fps=30.014, max_gap_s=0.126, dropped=944, repeats=9, freeze_events=0, slate_frames=0, invalid_frames=0, counter_resets=0
- gate: PASS
