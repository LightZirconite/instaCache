#!/usr/bin/env python3
"""Serves the churn page and collects its jank report over HTTP.

Every engine under test reaches the same page the same way, and reports
through the same channel, so the numbers are comparable. Reading the report
out of the page is what used to need engine-specific plumbing (a window
title, a JavaScriptCore binding); a POST needs none.
"""
import http.server, os, socketserver, sys, threading, time

ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, "report.jsonl")


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=ROOT, **kw)

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
