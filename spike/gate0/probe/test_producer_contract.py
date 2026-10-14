"""Contract test across three languages: Rust producer -> shared memory -> C++ reader (TryReadFrame) -> Python codec.

Uses a per-session Local\\ section so it needs no elevation. Requires:
  producer/target/release/gate0-producer.exe   (cargo build --release)
  vcam-sample/gate0/ReaderCheck.exe            (cl ... ReaderCheck.cpp /link advapi32.lib)
"""
import pathlib
import subprocess
import time
import uuid

import numpy as np
import pytest

from counter_codec import MARKER_LIVE, decode_strip

GATE0 = pathlib.Path(__file__).resolve().parents[1]
PRODUCER = GATE0 / "producer" / "target" / "release" / "gate0-producer.exe"
READER = GATE0 / "vcam-sample" / "gate0" / "ReaderCheck.exe"
W, H = 1280, 720

EXIT_OK, EXIT_STALE, EXIT_ABSENT = 0, 3, 4


def read_once(name, out):
    r = subprocess.run([str(READER), name, str(out)], capture_output=True, text=True)
    return r.returncode, r.stdout.strip()


@pytest.fixture()
def producer():
    name = f"Local\\gate0-contract-{uuid.uuid4().hex[:8]}"
    proc = subprocess.Popen([str(PRODUCER), "--name", name, "--fps", "60"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    yield name, proc
    if proc.poll() is None:
        proc.kill()
    proc.wait()


def wait_for_frame(name, out, timeout=5.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        code, text = read_once(name, out)
        if code == EXIT_OK:
            return text
        time.sleep(0.1)
    raise AssertionError(f"reader never got a frame, last exit {code}: {text}")


def test_frame_round_trips_through_rust_cpp_python(producer, tmp_path):
    name, _ = producer
    out = tmp_path / "f.bgra"
    text = wait_for_frame(name, out)
    reported_counter = int(text.split("counter=")[1].split()[0])
    frame = np.fromfile(out, dtype=np.uint8).reshape(H, W, 4)
    marker, counter = decode_strip(frame[:, :, :3])
    assert marker == MARKER_LIVE
    assert counter == reported_counter


def test_counter_advances_while_producer_runs(producer, tmp_path):
    name, _ = producer
    a = int(wait_for_frame(name, tmp_path / "a.bgra").split("counter=")[1].split()[0])
    time.sleep(0.5)
    b = int(wait_for_frame(name, tmp_path / "b.bgra").split("counter=")[1].split()[0])
    assert b > a


def test_reader_refuses_when_producer_is_gone(producer, tmp_path):
    name, proc = producer
    wait_for_frame(name, tmp_path / "x.bgra")
    proc.kill()
    proc.wait()
    time.sleep(1.3)   # longer than the 1 s staleness window
    code, _ = read_once(name, tmp_path / "y.bgra")
    assert code in (EXIT_STALE, EXIT_ABSENT)


def test_new_producer_recovers_a_section_left_with_an_odd_sequence(tmp_path):
    # A writer that died mid-update leaves the sequence odd ("writing"). The Frame Server DLL keeps the section alive
    # across producer crashes, so the next producer inherits it. With the odd sequence left alone no read can ever succeed
    # (0%); once it is repaired most reads succeed. A minority still hit a genuine mid-write tear, so assert a rate.
    name = f"Local\\gate0-contract-{uuid.uuid4().hex[:8]}"
    proc = subprocess.Popen([str(PRODUCER), "--name", name, "--fps", "60", "--poison-seq"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        wait_for_frame(name, tmp_path / "first.bgra", timeout=8.0)
        results = [read_once(name, tmp_path / f"r{i}.bgra")[0] for i in range(40)]
        ok = results.count(EXIT_OK)
        assert ok >= 20, f"only {ok}/40 reads succeeded: {results}"
    finally:
        proc.kill()
        proc.wait()
