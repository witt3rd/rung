#!/usr/bin/env python3
"""Performance baseline and gate for rung-agent.

Three metrics, each a median over interleaved trials against a deterministic
scripted model (a local OpenAI-compatible server that always answers the same
text), on one machine:

  cold_start_ms   spawn -> the model server receives its first request
  rss_mib_10mib   peak resident memory after loading a 10 MiB session and
                  running one turn on it (Linux /proc VmHWM)
  switch_ms       ACP session/load of a second session while a first is loaded

    scripts/perf_gate.py measure --bin base=/path/a --bin head=/path/b --out run.json
    scripts/perf_gate.py gate --baseline perf/baseline.json --current run.json
    scripts/perf_gate.py gate --current run.json --compare base head   # same-run, interleaved

Trials of the named binaries alternate, so drift hits all of them alike.
A metric whose trials spread too much (MAD/median above NOISE_LIMIT) is
withheld: no median is recorded and the gate cannot pass or fail it. Gate exit
codes: 0 ok, 1 regression over THRESHOLD, 2 only withheld/missing metrics.
A baseline is only comparable on the machine that recorded it.
"""

import argparse
import http.server
import json
import os
import shutil
import socketserver
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

THRESHOLD = 0.10
NOISE_LIMIT = 0.20
METRICS = ("cold_start_ms", "rss_mib_10mib", "switch_ms")
REPLY = {"id": "c", "model": "m", "choices": [{"message": {"content": "ok"}, "finish_reason": "stop"}]}


class Model:
    """Scripted model: same answer every time; stamps the first request."""

    def __init__(self):
        outer = self
        self.first = None

        class H(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *a):
                pass

            def do_POST(self):
                if outer.first is None:
                    outer.first = time.perf_counter()
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                body = json.dumps(REPLY).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        self.srv = socketserver.ThreadingTCPServer(("127.0.0.1", 0), H)
        self.srv.daemon_threads = True
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.srv.server_address[1]}/v1"

    def close(self):
        self.srv.shutdown()
        self.srv.server_close()


class Acp:
    def __init__(self, binary, env, cwd):
        self.p = subprocess.Popen(
            [binary, "--acp", "--tools", "none"], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env, cwd=cwd, text=True,
        )
        self.n = 0

    def call(self, method, params):
        self.n += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError("agent exited")
            v = json.loads(line)
            if v.get("id") == self.n and "method" not in v:
                if "error" in v:
                    raise RuntimeError(f"{method}: {v['error']}")
                return v["result"]

    def hwm_mib(self):
        for line in Path(f"/proc/{self.p.pid}/status").read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) / 1024
        raise RuntimeError("no VmHWM")

    def stop(self):
        self.p.kill()
        self.p.wait()


def write_session(cwd, sid, nbytes):
    """A session of about nbytes: alternating user/assistant lines of 4 KiB."""
    d = Path(cwd) / ".rung" / "sessions"
    d.mkdir(parents=True, exist_ok=True)
    chunk = "x" * 4096
    lines = []
    for i in range(max(2, nbytes // 4200)):
        lines.append({"role": "user" if i % 2 == 0 else "assistant", "text": chunk})
    s = {"id": sid, "kind": "explore", "status": "idle", "cwd": str(cwd), "lines": lines}
    (d / f"{sid}.json").write_text(json.dumps(s))


def trial(binary):
    work = Path(tempfile.mkdtemp(prefix="perf-gate-"))
    model = Model()
    try:
        env = {k: v for k, v in os.environ.items() if not k.startswith("RUNG_")}
        env.update({
            "HOME": str(work), "XDG_CONFIG_HOME": str(work), "RUNG_HOME": str(work),
            "RUNG_CONFIG": str(work / "none.yaml"), "RUNG_BASE_URL": model.url,
            "RUNG_MODEL": "m", "RUNG_API_KEY": "k", "RUNG_PROTOCOL": "openai",
        })
        out = {}
        # cold start: spawn -> first model request of a fresh session
        t0 = time.perf_counter()
        a = Acp(binary, env, work)
        a.call("initialize", {"protocolVersion": 1})
        sid = a.call("session/new", {"cwd": str(work), "mcpServers": []})["sessionId"]
        a.call("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "hi"}]})
        out["cold_start_ms"] = (model.first - t0) * 1000
        a.stop()
        # memory at a 10 MiB session, and switch time between two sessions
        write_session(work, "big", 10 * 1024 * 1024)
        write_session(work, "other", 256 * 1024)
        a = Acp(binary, env, work)
        a.call("initialize", {"protocolVersion": 1})
        a.call("session/load", {"sessionId": "other", "cwd": str(work), "mcpServers": []})
        t1 = time.perf_counter()
        a.call("session/load", {"sessionId": "big", "cwd": str(work), "mcpServers": []})
        out["switch_ms"] = (time.perf_counter() - t1) * 1000
        a.call("session/prompt", {"sessionId": "big", "prompt": [{"type": "text", "text": "hi"}]})
        out["rss_mib_10mib"] = a.hwm_mib()
        a.stop()
        return out
    finally:
        model.close()
        shutil.rmtree(work, ignore_errors=True)


def summarize(samples, noise_limit=NOISE_LIMIT):
    """Median of samples, or None (withheld) when too noisy to compare."""
    med = statistics.median(samples)
    if med <= 0:
        return None
    mad = statistics.median(abs(s - med) for s in samples)
    if mad / med > noise_limit:
        return None
    return med


def measure(bins, trials):
    raw = {name: {m: [] for m in METRICS} for name, _ in bins}
    for _ in range(trials):  # interleaved: one trial of each binary per round
        for name, path in bins:
            for m, v in trial(path).items():
                raw[name][m].append(v)
    return {
        "threshold": THRESHOLD, "noise_limit": NOISE_LIMIT, "trials": trials,
        "results": {
            name: {m: {"median": summarize(v), "samples": [round(x, 3) for x in v]} for m, v in per.items()}
            for name, per in raw.items()
        },
    }


def compare(base, cur, threshold=THRESHOLD):
    """Per metric: (status, base, cur, ratio) with status ok|regression|withheld."""
    rows = {}
    for m in METRICS:
        b, c = base.get(m, {}).get("median"), cur.get(m, {}).get("median")
        if b is None or c is None:
            rows[m] = ("withheld", b, c, None)
            continue
        ratio = c / b
        rows[m] = ("regression" if ratio > 1 + threshold else "ok", b, c, ratio)
    return rows


def verdict(rows):
    if any(r[0] == "regression" for r in rows.values()):
        return 1
    if any(r[0] == "withheld" for r in rows.values()):
        return 2
    return 0


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("measure")
    m.add_argument("--bin", action="append", required=True, metavar="NAME=PATH")
    m.add_argument("--trials", type=int, default=9)
    m.add_argument("--out", required=True)
    g = sub.add_parser("gate")
    g.add_argument("--current", required=True)
    g.add_argument("--baseline")
    g.add_argument("--compare", nargs=2, metavar=("BASE", "HEAD"))
    g.add_argument("--name", default="head", help="entry of --current to gate against --baseline")
    a = ap.parse_args()

    if a.cmd == "measure":
        bins = [tuple(b.split("=", 1)) for b in a.bin]
        bins = [(n, str(Path(p).resolve())) for n, p in bins]
        Path(a.out).write_text(json.dumps(measure(bins, a.trials), indent=2) + "\n")
        print(f"wrote {a.out}")
        return 0

    cur = json.loads(Path(a.current).read_text())["results"]
    if a.compare:
        base, head = cur[a.compare[0]], cur[a.compare[1]]
    elif a.baseline:
        base = json.loads(Path(a.baseline).read_text())["results"]["head"]
        head = cur[a.name]
    else:
        ap.error("gate needs --baseline or --compare")
    rows = compare(base, head)
    for k, (st, b, c, r) in rows.items():
        print(f"{k:16} {st:10} base={b} current={c}" + (f" ratio={r:.3f}" if r else ""))
    return verdict(rows)


if __name__ == "__main__":
    sys.exit(main())
