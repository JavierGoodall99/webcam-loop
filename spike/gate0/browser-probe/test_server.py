import json
import pathlib
import threading
import urllib.error
import urllib.request

import pytest

import server


@pytest.fixture()
def running(tmp_path):
    srv = server.make_server(port=0, results_dir=tmp_path)   # port 0: pick a free port
    thread = threading.Thread(target=srv.serve_forever, daemon=True)
    thread.start()
    yield srv, tmp_path
    srv.shutdown()
    srv.server_close()


def url(srv, path):
    return f"http://127.0.0.1:{srv.server_address[1]}{path}"


def post(srv, path, payload):
    req = urllib.request.Request(url(srv, path), data=json.dumps(payload).encode(), method="POST",
                                 headers={"Content-Type": "application/json"})
    return urllib.request.urlopen(req)


def test_serves_the_probe_page(running):
    srv, _ = running
    with urllib.request.urlopen(url(srv, "/")) as r:
        body = r.read().decode()
    assert "<title>" in body and "strip-codec.mjs" in body


def test_post_writes_the_result_file(running):
    srv, results = running
    with post(srv, "/result?label=chrome-soak", {"fps": 30.0, "frames": 3600}) as r:
        assert r.status == 200
    saved = json.loads((results / "chrome-soak.json").read_text())
    assert saved["frames"] == 3600


def test_label_cannot_escape_the_results_folder(running):
    srv, results = running
    post(srv, "/result?label=../../evil", {"x": 1}).close()
    assert not (results.parent / "evil.json").exists()
    assert [p.name for p in results.iterdir()] == ["evil.json"]          # sanitised to a name inside the folder


def test_rejects_oversized_bodies(running):
    # The server refuses without reading the body, so on Windows the client may see the 413 or a connection reset,
    # depending on timing. Either way the upload must not succeed and nothing may be saved.
    srv, results = running
    with pytest.raises((urllib.error.HTTPError, ConnectionError, OSError)) as e:
        post(srv, "/result?label=big", {"blob": "x" * (server.MAX_BODY_BYTES + 10)})
    if isinstance(e.value, urllib.error.HTTPError):
        assert e.value.code == 413
        e.value.close()                      # an HTTPError is also the response object; release its socket
    assert list(results.iterdir()) == []


def test_only_localhost_binding(running):
    srv, _ = running
    assert srv.server_address[0] == "127.0.0.1"


def test_javascript_modules_are_served_with_a_javascript_content_type(running):
    # Browsers refuse to run an ES module served as text/plain; Python's default map on Windows has no entry for .mjs.
    srv, _ = running
    with urllib.request.urlopen(url(srv, "/strip-codec.mjs")) as r:
        assert r.headers.get_content_type() in ("text/javascript", "application/javascript")
