#!/usr/bin/env python3
"""Scripted OpenAI-compatible server for the examples: no key, no network.

usage: mock_llm.py PORT_FILE REPLY [REPLY ...]
Answers the Nth chat request with REPLY N (the last one repeats) and writes
every request body to PORT_FILE.requests (one JSON per line).
"""
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

port_file, replies = sys.argv[1], sys.argv[2:]
n = 0

class H(BaseHTTPRequestHandler):
    def do_POST(self):
        global n
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        with open(port_file + ".requests", "a") as f:
            f.write(json.dumps(json.loads(body)) + "\n")
        text = replies[min(n, len(replies) - 1)]
        n += 1
        out = json.dumps({"id": "c", "model": "m", "choices": [
            {"message": {"content": text}, "finish_reason": "stop"}]}).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass

s = HTTPServer(("127.0.0.1", 0), H)
open(port_file, "w").write(str(s.server_port))
s.serve_forever()
