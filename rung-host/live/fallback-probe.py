#!/usr/bin/env python3
"""Test the router's server-side fallbacks (a `models` array on the request)
against one named model, on free models, with a fixed long prefix so the
prompt cache can show.

usage: doppler run ... --only-secrets RUNG_HOST_OPENROUTER_API_KEY -- \
  fallback-probe.py OUT.jsonl [-k N] [--gap S] MODEL [MODEL...]

Arm `single` sends `model: MODEL[0]`; arm `fallback` sends the same body with
`models: [MODEL...]`. The arms alternate request by request (N each). Each
line of OUT.jsonl holds arm, HTTP status, served model, provider, prompt and
cached tokens, latency, finish reason, whether a tool call came back and
parsed, and the router's error message. KEY_ENV names the key's env var
(default RUNG_HOST_OPENROUTER_API_KEY); the key is never printed or written.
"""
import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
from collections import Counter

URL = "https://openrouter.ai/api/v1/chat/completions"
# A stable ~2k-token prefix: the same bytes every request, so a provider
# that caches prefixes can report cached tokens on the second hit.
PREFIX = "You are a careful assistant in a sandbox. " + " ".join(
    f"Rule {i}: keep answers short, cite the rule you follow, and never invent a file." for i in range(120))
TOOLS = [{"type": "function", "function": {
    "name": "note", "description": "Record one short note.",
    "parameters": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}}]


def ask(key, body):
    req = urllib.request.Request(URL, data=json.dumps(body).encode(), method="POST", headers={
        "Authorization": f"Bearer {key}", "content-type": "application/json"})
    t = time.time()
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            code, d = r.status, json.load(r)
    except urllib.error.HTTPError as e:
        code = e.code
        try:
            d = json.load(e)
        except Exception:
            d = {}
    except Exception as e:  # timeout, reset
        return {"status": 0, "error": type(e).__name__, "latency_ms": int((time.time() - t) * 1000)}
    ms = int((time.time() - t) * 1000)
    if "error" in d:
        meta = d["error"].get("metadata") or {}
        return {"status": code, "error": (d["error"].get("message") or "")[:160],
                "provider": meta.get("provider_name"), "latency_ms": ms}
    u = d.get("usage") or {}
    ch = (d.get("choices") or [{}])[0]
    tcs = (ch.get("message") or {}).get("tool_calls") or []
    parsed = None
    if tcs:
        try:
            json.loads(tcs[0]["function"]["arguments"])
            parsed = True
        except Exception:
            parsed = False
    return {"status": code, "served": d.get("model"), "provider": d.get("provider"),
            "prompt": u.get("prompt_tokens"), "cached": (u.get("prompt_tokens_details") or {}).get("cached_tokens"),
            "finish": ch.get("finish_reason") or ch.get("native_finish_reason"),
            "tool_call": bool(tcs), "tool_parsed": parsed, "latency_ms": ms,
            # A served-but-errored choice (upstream error inside a 200)
            "error": (ch.get("error") or {}).get("message", "")[:160] or None}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("models", nargs="+")
    ap.add_argument("-k", type=int, default=15)
    ap.add_argument("--gap", type=float, default=4.0)
    a = ap.parse_args()
    key = os.environ.get(os.environ.get("KEY_ENV", "RUNG_HOST_OPENROUTER_API_KEY"), "").strip()
    if not key:
        print("no key in the environment", file=sys.stderr)
        return 2
    rows = []
    with open(a.out, "w") as f:
        for i in range(a.k):
            for arm in ("single", "fallback"):
                body = {"messages": [{"role": "system", "content": PREFIX},
                                     {"role": "user", "content": f"Request {i}: record a one-line note saying OK {i}, then stop."}],
                        "tools": TOOLS, "max_tokens": 200}
                if arm == "single":
                    body["model"] = a.models[0]
                else:
                    body["model"] = a.models[0]
                    body["models"] = a.models
                r = {"arm": arm, "i": i, **ask(key, body)}
                rows.append(r)
                f.write(json.dumps(r) + "\n")
                f.flush()
                print(json.dumps(r))
                time.sleep(a.gap)
    for arm in ("single", "fallback"):
        rs = [r for r in rows if r["arm"] == arm]
        ok = [r for r in rs if r["status"] == 200 and not r.get("error")]
        p = sum(r.get("prompt") or 0 for r in ok)
        c = sum(r.get("cached") or 0 for r in ok)
        print(f"{arm}: {len(ok)}/{len(rs)} ok; statuses {dict(Counter(r['status'] for r in rs))}; "
              f"served {dict(Counter(r.get('served') for r in ok))}; cache {c}/{p}; "
              f"tool calls {sum(r.get('tool_call') for r in ok)} parsed {sum(bool(r.get('tool_parsed')) for r in ok)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
