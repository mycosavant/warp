#!/usr/bin/env python3
# A logging stand-in for the local whisper endpoint.
#
# The fork's voice path POSTs the recording to the configured local
# transcription endpoint (default http://127.0.0.1:8080/inference,
# `app/src/voice/local_transcriber.rs`). This is a plain-HTTP loopback server
# that records method, path, headers and body size for every request and
# answers with whisper.cpp's `{text}` shape, so the app's transcribe path
# *completes* rather than hanging on a dead port. No TLS: the body is readable.
#
# It exists because the egress question about voice ("does audio leave the
# machine?") is answered by *where the POST goes*, and to see the POST at all
# the endpoint has to answer. Two ways to use it, both measured 2026-09-05:
#
#   - Point nothing at the port and the app fails closed, naming only the
#     loopback URL and never falling back to api.warp.dev. That is the
#     fail-closed proof.
#   - Run this and the POST completes; the log shows `body_bytes=<n>` for the
#     audio and the transcription renders. That is the destination proof.
#
# Usage:
#   whisper-stub.py [logfile] [port]
#   whisper-stub.py /path/to/whisper-stub.log 8080
#
# Reusable past voice: any fork feature whose "local endpoint" default is a
# loopback URL (the four small AI features, a custom inference endpoint) can be
# pointed at this to capture what it would have sent. Change the response body
# in `_h` to match what that endpoint returns.
import http.server, json, sys, datetime

LOG = sys.argv[1] if len(sys.argv) > 1 else "/tmp/whisper-stub.log"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8080


def log(m):
    with open(LOG, "a") as f:
        f.write(f"{datetime.datetime.utcnow().isoformat()}Z {m}\n")


class H(http.server.BaseHTTPRequestHandler):
    def _h(self, method):
        n = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(n) if n else b""
        log(
            f"{method} {self.path} host={self.headers.get('Host')} "
            f"ct={self.headers.get('Content-Type')} body_bytes={len(body)}"
        )
        r = json.dumps({"text": "egress probe transcription"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(r)))
        self.end_headers()
        self.wfile.write(r)

    def do_POST(self):
        self._h("POST")

    def do_GET(self):
        self._h("GET")

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    log(f"# whisper-stub listening on 127.0.0.1:{PORT}")
    http.server.HTTPServer(("127.0.0.1", PORT), H).serve_forever()
