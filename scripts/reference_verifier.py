#!/usr/bin/env python3
"""Reference verifier: replay scripted inputs on a pinned previous release
and on the candidate, and diff provider request bytes and saved sessions.

usage: reference_verifier.py --ref-dir DIR --cand-dir DIR [--expected NOTE]
                             [--receipts DIR]
DIR holds `rung-agent` and `mock-llm` (e.g. target/debug of each build).
Exits 1 on any difference not covered by the expected-diff note: a JSON
file {"requests": [{"index": N, "reason": "..."}], "sessions": [{"file":
"relative/path", "reason": "..."}]}. Every entry needs a written reason.
Metric lines: unintended_request_diffs=N cache_prefix_breaks=M
"""
import argparse, json, os, re, subprocess, sys, tempfile, time

# one scripted conversation per entry; each is its own agent invocation
INPUTS = [
    "Remember this: the deploy branch is release/x",
    "Which deploy branch do we use?",
    "Summarise our conversation so far.",
]
REPLIES = ["Noted.", "release/x", "Summary."]
STAMP = re.compile(r"\d{4}-\d\d-\d\dT[\d:.]+Z?|\b\d{10,}\b")


def run(d, work):
    os.makedirs(work)
    port = os.path.join(work, "port")
    env = dict(os.environ, RUNG_HOME=work, HOME=work, XDG_CONFIG_HOME=work,
               RUNG_CONFIG=os.path.join(work, "none.yaml"),
               RUNG_MEMORY="baseline", RUNG_MODEL="m", RUNG_API_KEY="k",
               RUNG_PROTOCOL="openai")
    mock = subprocess.Popen([os.path.join(d, "mock-llm"), port, *REPLIES])
    try:
        for _ in range(100):
            if os.path.exists(port) and os.path.getsize(port):
                break
            time.sleep(0.1)
        env["RUNG_BASE_URL"] = "http://127.0.0.1:" + open(port).read().strip()
        for text in INPUTS:
            subprocess.run([os.path.join(d, "rung-agent"), "--tools", "none",
                            text], cwd=work, env=env, check=True,
                           stdout=subprocess.DEVNULL, timeout=120)
    finally:
        mock.kill()
    reqs = [l.rstrip("\n") for l in open(port + ".requests")] \
        if os.path.exists(port + ".requests") else []
    sessions = {}
    for root, _, files in os.walk(work):
        for f in files:
            p = os.path.join(root, f)
            rel = os.path.relpath(p, work)
            if rel in ("port", "port.requests", "none.yaml"):
                continue
            try:
                txt = open(p, encoding="utf-8").read()
            except UnicodeDecodeError:
                continue
            sessions[rel] = STAMP.sub("<T>", txt.replace(work, "<W>"))
    return reqs, sessions


def prefix_break(a, b):
    """Cache prefix break: request b is not byte-prefixed by the same
    leading messages as a (shared leading chars < the shorter request)."""
    return a != b and os.path.commonprefix([a, b]) != min(a, b, key=len)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref-dir", required=True)
    ap.add_argument("--cand-dir", required=True)
    ap.add_argument("--expected")
    ap.add_argument("--receipts", default="verifier-receipts")
    a = ap.parse_args()
    a.ref_dir, a.cand_dir = os.path.abspath(a.ref_dir), os.path.abspath(a.cand_dir)
    exp = {"requests": [], "sessions": []}
    if a.expected and os.path.exists(a.expected):
        exp = json.load(open(a.expected))
    for k in ("requests", "sessions"):
        for e in exp.get(k, []):
            if not e.get("reason"):
                sys.exit(f"expected-diff entry without a reason: {e}")
    ok_req = {e["index"] for e in exp.get("requests", [])}
    ok_ses = {e["file"] for e in exp.get("sessions", [])}
    tmp = tempfile.mkdtemp(prefix="refverify-")
    rr, rs = run(a.ref_dir, os.path.join(tmp, "ref"))
    cr, cs = run(a.cand_dir, os.path.join(tmp, "cand"))
    diffs, breaks = [], 0
    if len(rr) != len(cr):
        diffs.append({"kind": "request_count", "ref": len(rr), "cand": len(cr)})
    for i, (x, y) in enumerate(zip(rr, cr)):
        if x != y:
            br = prefix_break(x, y)
            breaks += br
            diffs.append({"kind": "request", "index": i, "prefix_break": br,
                          "covered": i in ok_req})
    for f in sorted(set(rs) | set(cs)):
        if rs.get(f) != cs.get(f):
            diffs.append({"kind": "session", "file": f,
                          "covered": f in ok_ses})
    bad = [d for d in diffs if not d.get("covered")]
    os.makedirs(a.receipts, exist_ok=True)
    json.dump({"ref_requests": rr, "cand_requests": cr,
               "ref_sessions": rs, "cand_sessions": cs, "diffs": diffs},
              open(os.path.join(a.receipts, "receipt.json"), "w"), indent=1)
    line = f"unintended_request_diffs={sum(d['kind'] != 'session' for d in bad)} cache_prefix_breaks={breaks}"
    open(os.path.join(a.receipts, "metric.txt"), "w").write(line + "\n")
    print(line)
    for d in bad:
        print("UNCOVERED", json.dumps(d))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
