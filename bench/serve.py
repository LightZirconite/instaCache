#!/usr/bin/env python3
"""Serves the churn page and collects its jank report over HTTP.

Every engine under test reaches the same page the same way, and reports
through the same channel, so the numbers are comparable. Reading the report
out of the page is what used to need engine-specific plumbing (a window
title, a JavaScriptCore binding); a POST needs none.
"""
import http.server, json, os, socketserver, sys

ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, "report.jsonl")


# Requests that actually reached the network, by path prefix. Whether a warm
# start hits the server at all is the only reliable way to see the disk cache
# working: over loopback the transfer itself is too fast to time.
HITS = {"assets": 0}


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=ROOT, **kw)

    def do_GET(self):
        if self.path == "/stats":
            body = json.dumps(HITS).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/stats/reset":
            HITS["assets"] = 0
            self.send_response(204)
            self.end_headers()
            return
        if self.path.startswith("/assets/"):
            HITS["assets"] += 1
        super().do_GET()

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(n).decode("utf-8", "replace")
        with open(OUT, "a") as fh:
            fh.write(body + "\n")
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()

    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        # Anything under /assets/ is declared cacheable for a year, which is
        # what a real CDN says about a versioned bundle. Without it the browser
        # revalidates on every load and a warm start cannot be told from a cold
        # one -- the measurement would show the cache doing nothing when the
        # server is the reason.
        if self.path.startswith("/assets/"):
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        super().end_headers()

    def log_message(self, *a):
        pass


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8731
    open(OUT, "w").close()
    Server(("127.0.0.1", port), Handler).serve_forever()
