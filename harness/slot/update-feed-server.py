#!/usr/bin/env python3
"""A local update feed for the updates suite.

usage: update-feed-server.py <log.jsonl> [port]

Listens on 127.0.0.1 (8765 by default). The first line of /tmp/feed-mode names the folder under
/harness/slot/update-feed that the server answers from (newer, same, tampered, wrongkey), read on
every request; the mode "404" answers every request with 404 and "slow" waits 30 s first.
/latest.json and /latest.json.minisig come from that folder. Any other path, for example an
installer file, is served as 404 and still logged, so the suite can prove that nothing else was
asked for. Every request is one line in the log: time, method, path, and every request header.
The folders hold signed copies of test manifests: the secret keys were thrown away after signing,
so the files are fixed. `same` is the version the debug app reports (2.0.0-dev); `newer` is 2.1.0;
`wrongkey` is signed by a key that test.pub does not match; `tampered` changed after signing.
"""
import json
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG = sys.argv[1]
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8765
MODE_FILE = "/tmp/feed-mode"
ROOT = "/harness/slot/update-feed"


def mode():
    try:
        with open(MODE_FILE) as handle:
            return handle.readline().strip() or "newer"
    except OSError:
        return "newer"


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_GET(self):
        current = mode()
        entry = {
            "t_ms": int(time.time() * 1000),
            "method": self.command,
            "path": self.path,
            "mode": current,
            "headers": dict(self.headers.items()),
        }
        with open(LOG, "a") as handle:
            handle.write(json.dumps(entry) + "\n")
        if current == "slow":
            time.sleep(30)
            current = "newer"
        body = None
        if current != "404" and self.path in ("/latest.json", "/latest.json.minisig"):
            try:
                with open(os.path.join(ROOT, current, self.path.lstrip("/")), "rb") as handle:
                    body = handle.read()
            except OSError:
                body = None
        if body is None:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
