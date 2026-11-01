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
# session ids (time-ordered hex) and hashed file names differ on every run
IDENT = re.compile(r"\b[0-9a-f]{16}(?:-[0-9a-f]{6})?\b")


SCOPE = re.compile(r"rung-scope:[0-9a-f]+")  # hashed from the working directory


def norm(txt, work):
    txt = re.sub(r'"pid": \d+', '"pid": 0', txt)
    txt = SCOPE.sub("rung-scope:<I>", txt.replace(work, "<W>"))
    return IDENT.sub("<I>", STAMP.sub("<T>", txt))


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
    reqs = [norm(l.rstrip("\n"), work) for l in open(port + ".requests")] \
        if os.path.exists(port + ".requests") else []
    found = []
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
            found.append((rel, norm(txt, work)))
    # Files are keyed by their normalised name plus the order of the original
    # names (ids are time-ordered), so run-specific ids do not read as diffs.
    sessions, seen = {}, {}
    for rel, txt in sorted(found):
        key = norm(rel, work)
        seen[key] = seen.get(key, 0) + 1
        sessions[f"{key}#{seen[key]}"] = txt
    return reqs, sessions


def prefix_break(a, b):
    """Cache prefix break: request b is not byte-prefixed by the same
    leading messages as a (shared leading chars < the shorter request)."""
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return a != b and n < min(len(a), len(b))


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
