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

## Baseline measurements (pattern variant, Session camera, non-elevated)
- 2 min, 1280x720: 30.02 fps, max gap 0.10 s, 0 freezes, 0 undecodable, gate PASS.
- 1 min, 640x360 (pipeline-scaled): 29.96-30.01 fps, max gap 0.13-0.20 s, 0 freezes, gate PASS.
- Calibration: physical webcam 29.0 fps and virtual camera 29.9 fps with the same loop; the probe is not the limiter.

### expB-dllcreates-attach-late
- camera index 1, delivered size 1280x720
- frames=1349, duration_s=44.859, fps=30.050, max_gap_s=0.142, dropped=48, repeats=49, freeze_events=0, slate_frames=332, invalid_frames=0, counter_resets=0
- gate: FAIL — dropped 48/1397 > 1%

### expB-paced-ON-60s
- camera index 1, delivered size 1280x720
- frames=4014, duration_s=59.913, fps=66.981, max_gap_s=1.001, dropped=88, repeats=52, freeze_events=0, slate_frames=57, invalid_frames=3540, counter_resets=0
- gate: FAIL — max gap 1.00s > 0.5s; dropped 88/4102 > 1%; undecodable frames: 3540

### expB-paced-OFF-60s
- camera index 1, delivered size 1280x720
- frames=1800, duration_s=59.922, fps=30.022, max_gap_s=0.159, dropped=256, repeats=253, freeze_events=0, slate_frames=130, invalid_frames=0, counter_resets=0
- gate: FAIL — dropped 256/2056 > 1%

## Browser probe (shared client) runs
- 1 min, Chrome, rVFC: 29.32 fps, max gap 0.10 s, dropped 49/484 (10%), 0 freeze/slate/undecodable.
- 30 min, Chrome, MediaStreamTrackProcessor, tab hidden 87%: 29.44 fps, max gap 0.679 s, dropped 7456/60464 (12.3%), repeats 6446, 0 freeze/slate/undecodable.
