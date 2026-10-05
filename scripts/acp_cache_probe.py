#!/usr/bin/env python3
"""Live prompt-cache probe: the same rung-agent turn, twice, through ACP.

A recording proxy sits between `rung-agent --acp` and an OpenAI-compatible
route. The probe opens one session with a long system text, sends a turn,
then a second turn, and prints for each model call how far its request
extends the one before (tools, system, messages) and the usage the route
reported (prompt, cached, cost).

The key is read from the env var named by --key-env and only ever passed to
rung-agent's environment; the proxy forwards the Authorization header and
writes no header anywhere. Request bodies and usage go to --out.

    doppler run -p fleet -c dev_work -- python3 scripts/acp_cache_probe.py \
        --model <free model id> --out /tmp/cache-probe
"""

import argparse
import atexit
import shutil
import http.server
import json
import os
import socketserver
import subprocess
import sys
import tempfile
import threading
import urllib.request
from pathlib import Path

BODIES = []
USAGE = []
LOCK = threading.Lock()


def make_handler(upstream):
    class Proxy(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        def do_POST(self):
            raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            with LOCK:
                BODIES.append(raw.decode())
            req = urllib.request.Request(
                upstream.rstrip("/") + self.path.split("/v1", 1)[-1],
                data=raw,
                method="POST",
            )
            for h in ("Authorization", "Content-Type", "Accept"):
                if self.headers.get(h):
                    req.add_header(h, self.headers[h])
            try:
                resp = urllib.request.urlopen(req, timeout=300)
                status, body = resp.status, resp.read()
            except urllib.error.HTTPError as e:
                status, body = e.code, e.read()
            usage, provider = None, None
            for line in body.decode(errors="replace").splitlines():
                if line.startswith("data: {"):
                    try:
                        chunk = json.loads(line[6:])
                    except ValueError:
                        continue
                    provider = chunk.get("provider") or provider
                    if chunk.get("usage"):
                        usage = chunk["usage"]
            if usage is None:
                try:
                    usage = json.loads(body).get("usage")
                except ValueError:
                    usage = {"status": status}
            if isinstance(usage, dict) and provider:
                usage["provider"] = provider
            with LOCK:
                USAGE.append(usage)
            self.send_response(status)
            ctype = "text/event-stream" if b"data:" in body[:64] else "application/json"
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    return Proxy


def canon(body):
    """The body with cache markers removed: they mark, they are not content."""

    def strip(v):
        if isinstance(v, dict):
            return {k: strip(x) for k, x in v.items() if k != "cache_control"}
        if isinstance(v, list):
            return [strip(x) for x in v]
        return v

    return strip(json.loads(body))


def extends(prev, nxt):
    """Where `nxt` stops extending `prev`, or None when it extends it."""
    for k in ("model", "tools", "session_id", "reasoning_effort"):
        if prev.get(k) != nxt.get(k):
            return f"field {k}"
    a, b = prev["messages"], nxt["messages"]
    for i, m in enumerate(a):
        if i >= len(b) or json.dumps(m) != json.dumps(b[i]):
            return f"message {i} ({m.get('role')})"
    return None


class Acp:
    def __init__(self, cmd, env, cwd):
        self.p = subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, env=env, cwd=cwd, text=True,
        )
        self.n = 0

    def call(self, method, params):
        self.n += 1
        msg = {"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}
        self.p.stdin.write(json.dumps(msg) + "\n")
        self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise SystemExit("agent exited")
            v = json.loads(line)
            if v.get("id") == self.n and "method" not in v:
                return v


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--upstream", default="https://openrouter.ai/api/v1")
    ap.add_argument("--key-env", default="OPENROUTER_API_KEY")
    ap.add_argument("--bin", default="target/debug/rung-agent")
    ap.add_argument("--out", default=None)
    ap.add_argument("--memory", default=None, help="memory setting, e.g. mcp:http://127.0.0.1:9000/mcp")
    ap.add_argument("--scope", default=None, help="memory scope (RUNG_MEMORY_SCOPE)")
    ap.add_argument("--asks", nargs=2, default=None, help="override the two turn texts")
    a = ap.parse_args()
    if not os.environ.get(a.key_env):
        raise SystemExit(f"{a.key_env} is not set")

    srv = socketserver.ThreadingTCPServer(("127.0.0.1", 0), make_handler(a.upstream))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    port = srv.server_address[1]

    work = Path(tempfile.mkdtemp(prefix="cache-probe-cwd-"))
    atexit.register(shutil.rmtree, work, ignore_errors=True)
    (work / "notes.txt").write_text(
        "".join(f"note {i:03}: the quick brown fox jumps over the lazy dog\n" for i in range(150))
    )
    env = {k: v for k, v in os.environ.items() if not k.startswith("RUNG_")}
    env.update({
        "HOME": str(work), "XDG_CONFIG_HOME": str(work), "RUNG_HOME": str(work),
        "RUNG_CONFIG": str(work / "none.yaml"),
        "RUNG_BASE_URL": f"http://127.0.0.1:{port}/v1",
        "RUNG_MODEL": a.model, "RUNG_KEY_FILE": a.key_env, "RUNG_PROTOCOL": "openai",
        "RUNG_MAX_TOKENS": "300",
    })
    if a.memory:
        env["RUNG_MEMORY"] = a.memory
    if a.scope:
        env["RUNG_MEMORY_SCOPE"] = a.scope
    system = "You are a terse assistant. Answer in one short sentence.\n" + "".join(
        f"Rule {i}: keep answers short, plain and factual; never pad.\n" for i in range(120)
    )
    acp = Acp([str(Path(a.bin).resolve()), "--acp", "--tools", "read"], env, work)
    acp.call("initialize", {"protocolVersion": 1})
    sid = acp.call("session/new", {
        "cwd": str(work), "mcpServers": [], "_meta": {"systemPrompt": system},
    })["result"]["sessionId"]
    for ask in a.asks or ("Read notes.txt with read_file and tell me how many lines it has.",
                          "Thanks. What is the last note's number?"):
        r = acp.call("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": ask}]})
        print("turn:", r.get("result", {}).get("stopReason") or r.get("error"))
    acp.p.kill()

    out = Path(a.out) if a.out else Path(tempfile.mkdtemp(prefix="cache-probe-"))
    out.mkdir(parents=True, exist_ok=True)
    (out / "requests.jsonl").write_text("".join(b + "\n" for b in BODIES))
    (out / "usage.jsonl").write_text("".join(json.dumps(u) + "\n" for u in USAGE))
    prev = None
    for i, (b, u) in enumerate(zip(BODIES, USAGE)):
        u = u or {}  # an error answer reports no usage
        body = canon(b)
        where = "first" if prev is None else (extends(prev, body) or "extends")
        details = (u or {}).get("prompt_tokens_details") or {}
        print(f"call {i}: {where:28} prompt={u.get('prompt_tokens')} "
              f"cached={details.get('cached_tokens')} cost={u.get('cost')} "
              f"msgs={len(body['messages'])} session_id={'session_id' in body} "
              f"provider={u.get('provider')}")
        prev = body
    print(f"evidence: {out}")


if __name__ == "__main__":
    sys.exit(main())
