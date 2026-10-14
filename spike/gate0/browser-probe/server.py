"""THROWAWAY (Gate 0). Serves the browser probe on http://127.0.0.1:<port>/ and saves posted results.

  python server.py [--port 8765] [--results ..\\browser-results]

Binds to 127.0.0.1 only (getUserMedia needs a secure context and localhost counts as one).
"""
import argparse
import json
import pathlib
import re
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

MAX_BODY_BYTES = 1_000_000
HERE = pathlib.Path(__file__).resolve().parent


def safe_label(raw: str) -> str:
    name = re.sub(r"[^A-Za-z0-9._-]+", "-", raw).strip(".-") or "result"
    return name[:80]


def make_server(port: int, results_dir: pathlib.Path):
    results_dir = pathlib.Path(results_dir)
    results_dir.mkdir(parents=True, exist_ok=True)

    class Handler(SimpleHTTPRequestHandler):
        # Python's default map has no entry for .mjs on Windows (it falls back to text/plain), and browsers refuse
        # to run an ES module with that content type.
        extensions_map = {**SimpleHTTPRequestHandler.extensions_map, ".mjs": "text/javascript", ".js": "text/javascript"}

        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(HERE), **kwargs)

        def do_POST(self):
            parsed = urlparse(self.path)
            if parsed.path != "/result":
                self.send_error(404)
                return
            length = int(self.headers.get("Content-Length", "0"))
            if length > MAX_BODY_BYTES:
                self.send_error(413, "result too large")
                return
            try:
                payload = json.loads(self.rfile.read(length))
            except ValueError:
                self.send_error(400, "invalid JSON")
                return
            label = safe_label(parse_qs(parsed.query).get("label", ["result"])[0])
            (results_dir / f"{label}.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"saved")

        def log_message(self, fmt, *args):   # keep the console quiet
            pass

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--results", default=str(HERE.parent / "browser-results"))
    args = ap.parse_args()
    srv = make_server(args.port, pathlib.Path(args.results))
    print(f"browser probe: http://127.0.0.1:{srv.server_address[1]}/   results -> {args.results}")
    srv.serve_forever()
