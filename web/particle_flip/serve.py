"""Serve a local evidence directory and retain browser readbacks."""
import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("evidence", type=Path)
parser.add_argument("--port", type=int, default=8476)
args = parser.parse_args()

page = Path(__file__).resolve().parent
output = args.evidence.resolve()


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=output, **kw)

    def translate_path(self, path):
        name = path.split("?", 1)[0]
        if name in {"/", "/index.html", "/splash.js"}:
            return str(page / ("index.html" if name == "/" else name[1:]))
        return super().translate_path(path)

    def do_POST(self):
        match = re.fullmatch(r"/capture/(measurements\.json|error\.json|(?:spread|clumped|boundaries|cancellation)-(?:forward|reversed|capacity)\.grid)", self.path)
        size = int(self.headers.get("Content-Length", "0"))
        if not match or not 0 < size <= 4 * 1024 * 1024:
            self.send_error(400)
            return
        target = output / "browser"
        target.mkdir(exist_ok=True)
        (target / match[1]).write_bytes(self.rfile.read(size))
        self.send_response(204)
        self.end_headers()


with ThreadingHTTPServer(("127.0.0.1", args.port), Handler) as server:
    print(f"http://127.0.0.1:{args.port}", flush=True)
    server.serve_forever()
