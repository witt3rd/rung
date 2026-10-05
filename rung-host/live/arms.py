#!/usr/bin/env python3
"""Per-arm measures beside analyze.py: which models served (the response's
model field), failure rate and classes, malformed tool calls, cache hit per
served model and across served-model changes, and a sample of turns for a
coherence reading.

usage: arms.py RUN_DIR [--json OUT.json] [--sample OUT.md] [-n N] [--seed S]

Reads only the record; never needs a key. The sample holds turns where the
served model changed from the previous completed turn first, then random
completed turns, each with its header line, served models, tool calls and
final text, and blank `verdict` / `note` fields for a reader to fill.
"""
import argparse
import json
import random
import re
from collections import Counter, defaultdict
from pathlib import Path

ARG_ERR = re.compile(r"missing field|invalid type|unknown field|invalid value|expected .* at line", re.I)


def load(run):
    out = []
    for f in sorted((run / "state" / "record").glob("*")):
        for raw in open(f):
            raw = raw.strip()
            if raw:
                out.append(json.loads(raw))
    out.sort(key=lambda d: d["seq"])
    return out


def ratio(a, b):
    return round(a / b, 4) if b else None


def measure(lines):
    turns = Counter()
    classes = Counter()
    calls = [d for d in lines if d["kind"] == "llm.call"]
    served = Counter(d.get("model_served") or "?" for d in calls)
    requested = Counter(d.get("model_requested") or "?" for d in calls)
    mismatch = sum(1 for d in calls if d.get("model_served") != d.get("model_requested"))
    per = defaultdict(lambda: [0, 0, 0])  # prompt, cached, calls
    same = [0, 0]
    switched = [0, 0]
    prev = None
    switches = 0
    for d in calls:
        m = d.get("model_served")
        p, c = d.get("prompt_tokens", 0), d.get("cached_tokens", 0)
        per[m][0] += p
        per[m][1] += c
        per[m][2] += 1
        bucket = same if m == prev else switched
        if prev is not None and m != prev:
            switches += 1
        if prev is not None:
            bucket[0] += p
            bucket[1] += c
        prev = m
    completed_by = Counter()
    served_by_turn = defaultdict(list)
    for d in calls:
        served_by_turn[d.get("turn")].append(d.get("model_served"))
    for d in lines:
        if d["kind"] == "turn.ended":
            turns[d["status"]] += 1
            if d["status"] == "failed":
                f = d.get("failure") or {}
                classes[f"{f.get('origin')}/{f.get('class')}"] += 1
            if d["status"] == "completed":
                completed_by[d.get("model")] += 1
    uses = malformed = unknown = argerr = 0
    for d in lines:
        if d["kind"] != "turn.log":
            continue
        for m in d.get("messages", []):
            content = m.get("content")
            if isinstance(content, str):
                malformed += content.count("could not be executed because its arguments were malformed")
                malformed += content.count("was cut off")
                continue
            for b in content or []:
                t = b.get("type")
                if t == "tool_use":
                    uses += 1
                elif t == "tool_result" and b.get("is_error"):
                    text = b.get("content") if isinstance(b.get("content"), str) else json.dumps(b.get("content"))
                    if "unknown tool" in text:
                        unknown += 1
                    elif ARG_ERR.search(text):
                        argerr += 1
                elif t == "text" and m.get("role") == "user":
                    text = b.get("text", "")
                    malformed += text.count("could not be executed because its arguments were malformed")
    total = sum(turns.values())
    prompt = sum(d.get("prompt_tokens", 0) for d in calls)
    cached = sum(d.get("cached_tokens", 0) for d in calls)
    return {
        "turns": dict(turns),
        "turns_total": total,
        "failure_rate": ratio(turns.get("failed", 0), total),
        "failure_classes": dict(classes),
        "calls": len(calls),
        "served": dict(served.most_common()),
        "requested": dict(requested.most_common()),
        "served_ne_requested": mismatch,
        "served_switches": switches,
        "completed_by_turn_model": dict(completed_by),
        "cache_efficiency": ratio(cached, prompt),
        "cache_by_served": {m: {"calls": v[2], "efficiency": ratio(v[1], v[0])} for m, v in per.items()},
        "cache_same_model_as_prev_call": ratio(same[1], same[0]),
        "cache_after_served_switch": ratio(switched[1], switched[0]),
        "tool_uses": uses,
        "tool_malformed": malformed,
        "tool_unknown": unknown,
        "tool_arg_errors": argerr,
        "_served_by_turn": served_by_turn,
    }


def sample(lines, served_by_turn, n, seed):
    logs = {d["turn"]: d for d in lines if d["kind"] == "turn.log"}
    ended = [d for d in lines if d["kind"] == "turn.ended" and d["status"] == "completed" and d["turn"] in logs]
    changed, rest = [], []
    prev = None
    for d in ended:
        ms = served_by_turn.get(d["turn"]) or [d.get("model")]
        (changed if prev is not None and ms[0] != prev else rest).append(d)
        prev = ms[-1]
    random.Random(seed).shuffle(rest)
    pick = sorted((changed[:n] + rest)[:n], key=lambda d: d["turn"])
    out = [f"# Coherence sample ({len(pick)} completed turns; {len(changed)} follow a served-model change)\n"]
    for d in pick:
        log = logs[d["turn"]]
        ms = served_by_turn.get(d["turn"]) or []
        tools = [b.get("name") for m in log["messages"] if isinstance(m.get("content"), list)
                 for b in m["content"] if b.get("type") == "tool_use"]
        out.append(f"## turn {d['turn']} ({d.get('turn_kind')})\n")
        out.append(f"- header: `{log['header'].splitlines()[0]}`")
        out.append(f"- served: {', '.join(dict.fromkeys(m or '?' for m in ms))}")
        out.append(f"- tools: {', '.join(tools) or 'none'}")
        text = (d.get("final_text") or "").strip().replace("\n", " ")
        out.append(f"- final: {text[:600]}")
        out.append("- verdict: \n- note: \n")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run")
    ap.add_argument("--json")
    ap.add_argument("--sample")
    ap.add_argument("-n", type=int, default=12)
    ap.add_argument("--seed", type=int, default=7)
    a = ap.parse_args()
    lines = load(Path(a.run))
    m = measure(lines)
    sbt = m.pop("_served_by_turn")
    for k, v in m.items():
        print(f"{k}: {json.dumps(v)}")
    if a.json:
        Path(a.json).write_text(json.dumps(m, indent=1) + "\n")
    if a.sample:
        Path(a.sample).write_text(sample(lines, sbt, a.n, a.seed))


if __name__ == "__main__":
    main()
