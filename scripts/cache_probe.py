#!/usr/bin/env python3
"""Cache-hit probe: per-call cached/prompt tokens and first differing byte
offset between consecutive requests.

usage: cache_probe.py REQUESTS USAGE_LOG

REQUESTS   session record of outbound requests, in call order: JSONL (one
           request body per line, or {"request": body}), or a JSON array /
           {"requests": [...]}. A body may be a string (raw bytes).
USAGE_LOG  JSONL of llm.call events ({"turn","call","prompt_tokens",
           "cached_tokens"}, optionally under "body"/"data"), same order.
Requests are compared as compact, key-sorted JSON.
"""
import json
import sys


def _load(path):
    text = open(path, encoding="utf-8").read()
    try:
        doc = json.loads(text)
        if isinstance(doc, dict):
            doc = doc.get("requests", [doc])
        return doc
    except json.JSONDecodeError:
        return [json.loads(l) for l in text.splitlines() if l.strip()]


def canon(req):
    if isinstance(req, dict) and set(req) == {"request"}:
        req = req["request"]
    if isinstance(req, str):
        return req.encode()
    return json.dumps(req, sort_keys=True, separators=(",", ":")).encode()


def first_diff(a, b):
    n = min(len(a), len(b))
    for i in range(n):
        if a[i] != b[i]:
            return i
    return None if len(a) == len(b) else n


def usage_rows(path):
    rows = []
    for l in open(path, encoding="utf-8"):
        if not l.strip():
            continue
        e = json.loads(l)
        e = e.get("body") or e.get("data") or e
        if "prompt_tokens" in e:
            rows.append(e)
    return rows


def probe(requests, usage):
    out, prev = [], None
    for i, req in enumerate(requests):
        b = canon(req)
        u = usage[i] if i < len(usage) else {}
        out.append({
            "n": i + 1, "turn": u.get("turn"), "call": u.get("call"),
            "cached": u.get("cached_tokens"), "prompt": u.get("prompt_tokens"),
            "diff": None if prev is None else first_diff(prev, b),
            "bytes": len(b), "first": prev is None,
        })
        prev = b
    return out


def render(rows):
    lines = ["n  turn call cached/prompt  diff_offset  bytes"]
    for r in rows:
        d = "-" if r["first"] else ("same" if r["diff"] is None else r["diff"])
        c, p = r["cached"], r["prompt"]
        ratio = "?" if c is None or p is None else f"{c}/{p}"
        lines.append(f"{r['n']:<2} {r['turn'] if r['turn'] is not None else '-':<4} "
                     f"{r['call'] if r['call'] is not None else '-':<4} "
                     f"{ratio:<13} {d!s:<12} {r['bytes']}")
    return "\n".join(lines)


def main(argv):
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    print(render(probe(_load(argv[1]), usage_rows(argv[2]))))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
