#!/usr/bin/env python3
"""Free-time measures for one run: how many free-time turns were filler
(completed, no tool call), and which distinct useful actions they took
(distinct workspace writes, memory writes, tools used).

usage: free_time.py RUN_DIR [--json OUT.json]
"""
import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from analyze import load  # noqa: E402


def main():
    lines = load(Path(sys.argv[1]))
    logs = {d["turn"]: d for d in lines if d["kind"] == "turn.log"}
    kinds = Counter()
    free = Counter()
    useful = set()
    tools_used = Counter()
    for d in lines:
        if d["kind"] != "turn.ended":
            continue
        kinds[d.get("turn_kind")] += 1
        if d.get("turn_kind") != "free":
            continue
        log = logs.get(d["turn"], {"messages": []})
        uses = [b for m in log["messages"] if isinstance(m.get("content"), list)
                for b in m["content"] if b.get("type") == "tool_use"]
        if d["status"] == "failed":
            free["failed"] += 1
            continue
        free["completed"] += 1
        if not uses:
            free["filler"] += 1
            continue
        free["with_tools"] += 1
        for b in uses:
            tools_used[b["name"]] += 1
            if b["name"] in ("ws_write", "ws_edit", "memory_write", "retain", "execute"):
                useful.add((b["name"], json.dumps(b.get("input", {}).get("path") or b.get("input", {}).get("name") or "", sort_keys=True)))
    out = {
        "turn_kinds": dict(kinds),
        "free_turns": free["failed"] + free["completed"],
        "free_failed": free["failed"],
        "free_completed": free["completed"],
        "free_filler": free["filler"],
        "free_filler_share_of_completed": round(free["filler"] / free["completed"], 4) if free["completed"] else None,
        "free_tools_used": dict(tools_used),
        "distinct_write_actions": len(useful),
        "distinct_write_targets": sorted(f"{a}:{p}" for a, p in useful),
    }
    for k, v in out.items():
        print(f"{k}: {json.dumps(v)}")
    if "--json" in sys.argv:
        Path(sys.argv[sys.argv.index("--json") + 1]).write_text(json.dumps(out, indent=1) + "\n")


main()
