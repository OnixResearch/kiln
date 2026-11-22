#!/usr/bin/env python3
"""Local fault endpoint for the composed Lattice #6 qualification.

Modes mirror src/candidate_worker/tests/isolation/faults/endpoint.rs:
  fail  answer every chat-completion request with HTTP 400 and the injected marker
  hold  record the request, then hold the response until RELEASE exists (or the
        client disconnects / 120 s pass), then answer HTTP 400
Every admitted request is appended to LOG as one JSON line. No upstream model is
contacted; this is the only endpoint the brokered worker can reach.
"""
import http.server
import json
import os
import sys
import time

MARKER = "lattice-injected-http-400"
mode, port_file, log, release = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        body = json.loads(self.rfile.read(length) or b"{}")
        with open(log, "a") as handle:
            handle.write(json.dumps({"path": self.path, "model": body.get("model"), "at": time.time()}) + "\n")
        if mode == "hold":
            deadline = time.time() + 120
            while not os.path.exists(release) and time.time() < deadline:
                time.sleep(0.05)
        payload = json.dumps({"error": {"message": MARKER, "type": "invalid_request_error", "code": "invalid_request"}}).encode()
        try:
            self.send_response(400)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(payload)
        except (BrokenPipeError, ConnectionResetError):
            pass


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
with open(port_file, "w") as handle:
    handle.write(str(server.server_address[1]))
server.serve_forever()
