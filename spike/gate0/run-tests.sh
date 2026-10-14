#!/usr/bin/env bash
# Runs every automated test suite of the Gate 0 spike. Needs the probe venv, the built producer and ReaderCheck.exe,
# SharedFrameTests.exe and PatternDump.exe (see README.md for the build commands).
set -euo pipefail
cd "$(dirname "$0")"
echo "== Python: probe + browser-probe server tests (warnings are errors) =="
probe/.venv/Scripts/python -m pytest -q -W error probe browser-probe
echo "== JavaScript: strip codec tests =="
(cd browser-probe && node --test strip-codec.test.mjs 2>&1 | grep -E "^ℹ (tests|pass|fail)")
echo "== C++: shared-frame reader tests =="
vcam-sample/gate0/SharedFrameTests.exe
