"""Checks that the tests can fail: breaks the code in a handful of deliberate ways (on temporary copies) and reports
whether `cargo test` notices each one. A mutation that SURVIVES means a behaviour is not pinned by any test.

Run from the crate folder:  python tools/mutation_check.py
Exit code is 0 only if every mutation was caught.
"""
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

CRATE = pathlib.Path(__file__).resolve().parents[1]

# (file under src/, exact text to find, replacement, what the break means)
MUTATIONS = [
    ("looper.rs", "frames.truncate(n - k);", "frames.truncate(n);",
     "the loop keeps the tail frames (wrong length)"),
    ("compositor.rs", "+ 128) >> 8", "+ 0) >> 8",
     "blending rounds down instead of to nearest"),
    ("looper.rs", "Some(dims) if dims != live_dims", "Some(dims) if dims != dims",
     "a size change while recording is not noticed"),
    ("looper.rs", "rec.frames.len() >= self.config.max_record_frames as usize",
     "rec.frames.len() > self.config.max_record_frames as usize",
     "the frame cap is off by one"),
    ("looper.rs", "rec.bytes >= self.config.max_record_bytes", "rec.bytes > self.config.max_record_bytes",
     "the memory cap is off by one"),
    ("seam.rs", "(((i + 1) * ALPHA_ONE as usize) / (k + 1)) as u16", "((i * ALPHA_ONE as usize) / k.max(1)) as u16",
     "the seam ramp starts at 0 and reaches the end"),
    ("looper.rs", "other @ (Mode::Live | Mode::Returning { .. }) => other,",
     "other @ (Mode::Live | Mode::Returning { .. }) => if matches!(other, Mode::Returning { .. }) { Mode::Live } else { other },",
     "go_live during the dissolve is no longer a no-op"),
    ("looper.rs", "&& rec.frames.len() >= self.config.min_record_frames as usize",
     "&& rec.frames.len() >= 0",
     "short holds are no longer ignored"),
    ("looper.rs", "self.mode = Mode::Recording(running);", "drop(running);",
     "a repeated key-down throws the recording away"),
    ("looper.rs", "self.mode = Mode::Loop(play);", "let _ = play;",
     "an ignored tap while looping no longer leaves the loop playing"),
    ("looper.rs", "Some(play) if self.config.crossfade_frames > 0 => Mode::Returning { play, step: 0 },",
     "Some(_) if self.config.crossfade_frames > 0 => Mode::Live,",
     "go_live during a re-recording jumps to live instead of dissolving from the loop on air"),
    ("looper.rs", "Some(play) if play.dims == live_dims => {", "Some(play) if play.dims == live_dims && false => {",
     "the old loop is no longer kept on air while re-recording"),
    ("config.rs", "if self.min_record_frames > self.max_record_frames {", "if false {",
     "an impossible minimum/maximum is accepted"),
]


def main() -> int:
    survivors = 0
    for i, (name, old, new, what) in enumerate(MUTATIONS):
        work = pathlib.Path(tempfile.mkdtemp(prefix="mutation-"))
        try:
            shutil.copytree(CRATE, work / "crate", ignore=shutil.ignore_patterns("target", ".git"))
            target = work / "crate" / "src" / name
            text = target.read_text(encoding="utf-8")
            if old not in text:
                print(f"[{i}] PATTERN NOT FOUND in {name}: {what}")
                survivors += 1
                continue
            target.write_text(text.replace(old, new, 1), encoding="utf-8")
            env = dict(os.environ, CARGO_TARGET_DIR=str(CRATE / "target" / "mutation"))
            run = subprocess.run(["cargo", "test", "--quiet"], cwd=work / "crate", capture_output=True, text=True, env=env)
            summary = re.search(r"test result: \w+\. \d+ passed; \d+ failed", run.stdout + run.stderr)
            caught = run.returncode != 0
            if not caught:
                survivors += 1
            print(f"[{i}] {'caught  ' if caught else 'SURVIVED'} {what}"
                  f"{'  (' + summary.group(0) + ')' if summary else ''}")
        finally:
            shutil.rmtree(work, ignore_errors=True)
    print(f"\n{len(MUTATIONS) - survivors}/{len(MUTATIONS)} mutations caught")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
