#!/usr/bin/env python3
"""Compute the pre-registered live-run measures (docs/rung-host-live-v1-prereg.md)
from a run directory: the record, the exit codes, the syscall traces.

usage: analyze.py RUN_DIR [--json OUT.json]

Prints one line per measure and, with --json, writes every number the
evidence report cites. Reads only; never needs a key.
"""
import glob
import json
import re
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def load(run):
    lines = []
    for f in sorted(glob.glob(str(run / "state" / "record" / "*"))):
        for raw in open(f):
            raw = raw.strip()
            if raw:
                lines.append(json.loads(raw))
    lines.sort(key=lambda d: d["seq"])
    return lines


def pct(xs, p):
    if not xs:
        return None
    xs = sorted(xs)
    k = max(0, min(len(xs) - 1, int(round(p / 100 * len(xs) + 0.5)) - 1))
    return xs[k]


def of(lines, kind):
    return [d for d in lines if d["kind"] == kind]


PROVIDER_CLASSES = {"rate_limit", "overloaded", "transport", "timeout"}


def l1(run, lines):
    windows = []
    for e in sorted(glob.glob(str(run / "exit.*"))):
        n = e.rsplit(".", 1)[1]
        log = run / f"host.{n}.log"
        text = log.read_text(errors="replace") if log.exists() else ""
        windows.append({"window": int(n), "exit": int(Path(e).read_text().strip()),
                        "panic": "panicked" in text})
    starts = of(lines, "host.start")
    halts = of(lines, "halted")
    ok = (len(windows) == len(starts) == len(halts) and windows
          and all(w["exit"] == 0 and not w["panic"] for w in windows))
    first, last = lines[0]["at"], lines[-1]["at"]
    return {"pass": bool(ok), "windows": windows, "starts": len(starts), "halts": len(halts),
            "halt_why": [h["why"] for h in halts], "record_span_s": round((last - first) / 1000, 1)}


def l2(lines):
    listed = [d for d in of(lines, "ladder.listed") if d.get("at_start")]
    turns = of(lines, "turn.started")
    ended = of(lines, "turn.ended")
    first_turn = turns[0] if turns else None
    first_list = listed[0] if listed else None
    best = None
    if first_list:
        avail = [r for r in first_list["rungs"] if r["available"]]
        best = avail[0]["model"] if avail else None
    completed = Counter(d["model"] for d in ended if d["status"] == "completed")
    ok = bool(first_list and first_list["ok"] and first_turn and first_list["seq"] < first_turn["seq"]
              and first_turn["model"] == best and sum(completed.values()) > 0)
    return {"pass": ok, "listed_before_first_turn": bool(first_list and first_turn and first_list["seq"] < first_turn["seq"]),
            "listing": [{k: r[k] for k in ("model", "available", "why")} for r in (first_list or {}).get("rungs", [])],
            "first_turn_model": first_turn and first_turn["model"], "best_standing": best,
            "completed_by_model": dict(completed),
            "turns_by_status": dict(Counter(d["status"] for d in ended))}


def l3(lines):
    out = {"provider_failures": 0, "unroutable": 0, "stepped_down": 0, "at_bottom": 0,
           "not_stepped": 0, "platform_429": 0, "platform_stepped": 0, "probes_up": 0,
           "switches": []}
    ladder = of(lines, "host.start")[0]["config"]["ladder"]
    pending = None
    for d in lines:
        k = d["kind"]
        if k == "turn.ended" and d["status"] == "failed" and d.get("failure"):
            f = d["failure"]
            if f.get("origin") == "platform":
                out["platform_429"] += 1
                pending = ("platform", d)
            elif f.get("unroutable") or f.get("class") in PROVIDER_CLASSES:
                out["provider_failures"] += 1
                out["unroutable"] += bool(f.get("unroutable"))
                pending = ("provider", d)
        elif k == "model.switch":
            out["switches"].append({"from": d["from"], "to": d["to"], "direction": d["direction"], "why": d["why"]})
            if d["direction"] == "up":
                out["probes_up"] += 1
            elif pending:
                if pending[0] == "provider":
                    out["stepped_down"] += 1
                else:
                    out["platform_stepped"] += 1
                pending = None
        elif k == "turn.started" and pending:
            if pending[0] == "provider":
                if pending[1].get("model") == ladder[-1]:
                    out["at_bottom"] += 1
                else:
                    out["not_stepped"] += 1
            pending = None
    exercised = out["provider_failures"] + out["platform_429"] > 0
    out["exercised"] = exercised
    out["pass"] = (None if not exercised else
                   out["not_stepped"] == 0 and out["platform_stepped"] == 0)
    return out


def l4(lines):
    calls = of(lines, "llm.call")
    by_epoch = defaultdict(lambda: {"s": set(), "l": set()})
    for c in calls:
        p = c.get("prefix") or {}
        by_epoch[c["epoch"]]["s"].add(p.get("s_hash"))
        by_epoch[c["epoch"]]["l"].add(p.get("l_hash"))
    unstable = [e for e, v in by_epoch.items() if len(v["s"]) > 1 or len(v["l"]) > 1]
    breaks = of(lines, "cache.break")
    host_causes = {"rollover", "swap", "model_switch", "restart", "epoch", "recovered"}
    unexplained = [b for b in breaks if not any(h in str(b.get("cause")) for h in host_causes)]
    prompt = sum(c.get("prompt_tokens", 0) for c in calls)
    cached = sum(c.get("cached_tokens", 0) for c in calls)
    return {"pass": not unstable and not unexplained, "calls": len(calls), "epochs": len(by_epoch),
            "unstable_epochs": unstable, "cache_breaks": dict(Counter(str(b.get("cause")) for b in breaks)),
            "cache_cold": dict(Counter(str(b.get("cause")) for b in of(lines, "cache.cold"))),
            "prompt_tokens": prompt, "cached_tokens": cached,
            "cache_efficiency": round(cached / prompt, 4) if prompt else None,
            "cache_write_tokens": sum(c.get("cache_write_tokens", 0) for c in calls)}


def waits_between(lines, a, b):
    """The degraded waits that ended between two record positions, by class, ms."""
    out, cls = Counter(), None
    for d in lines:
        if d["kind"] == "degraded":
            cls = d["class"]
        elif d["kind"] == "degraded.ended" and a < d["seq"] < b:
            out[cls or "?"] += d.get("waited_ms", 0)
    return dict(out)


def l5(lines):
    accepted = {}
    for d in of(lines, "stimulus.accepted"):
        accepted.setdefault(d["item"]["id"], d)
    boundaries = of(lines, "boundary")
    admitted = {}
    for d in of(lines, "stimulus.admitted"):
        for i in d.get("ids", []) + d.get("digests", []):
            iid = i if isinstance(i, str) else i.get("id")
            admitted.setdefault(iid, d)
    owner = [i for i, d in accepted.items() if d["item"].get("role") == "owner"]
    disposed = {d["id"]: d for d in of(lines, "stimulus.disposed")}
    rows, deferred = [], 0
    for i in owner:
        a = accepted[i]
        ad = admitted.get(i)
        # An item accepted inside a boundary's poll may be admitted by that
        # boundary; one accepted during a wait, by the next. Either way it is
        # deferred only when a later boundary than the next one admits it.
        nxt = next((b for b in boundaries if b["seq"] > a["seq"]), None)
        first_boundary = nxt["n"] if nxt else None
        if ad and first_boundary is not None and ad["boundary"] > first_boundary:
            deferred += 1
        rows.append({"id": i, "kind": a["item"].get("kind"), "admitted": bool(ad),
                     "latency_ms": ad["at"] - a["at"] if ad else None,
                     "disposition": disposed.get(i, {}).get("disposition"),
                     "disposed_ms": disposed[i]["at"] - a["at"] if i in disposed else None,
                     "boundary_gap": (ad["boundary"] - first_boundary) if ad and first_boundary is not None else None,
                     "waited_in": waits_between(lines, a["seq"], ad["seq"]) if ad else None})
    lat = [r["latency_ms"] for r in rows if r["latency_ms"] is not None]
    turn_ms = [d["elapsed_ms"] for d in of(lines, "turn.ended")]
    p95, tp95 = pct(lat, 95), pct(turn_ms, 95)
    done = [r["disposed_ms"] for r in rows if r["disposed_ms"] is not None]
    ok = deferred == 0 and all(r["admitted"] for r in rows) and (p95 is None or p95 <= (tp95 or 0) + 100)
    return {"pass": (bool(ok) if rows else None), "owner_items": len(rows), "deferred_past_first_boundary": deferred,
            "admission_ms_p50": pct(lat, 50), "admission_ms_p95": p95, "admission_ms_max": max(lat) if lat else None,
            "turn_ms_p50": pct(turn_ms, 50), "turn_ms_p95": tp95,
            "disposed_ms_p50": pct(done, 50), "disposed_ms_max": max(done) if done else None, "rows": rows}


def l6(lines):
    acc = Counter(d["item"]["id"] for d in of(lines, "stimulus.accepted"))
    disp = Counter(d["id"] for d in of(lines, "stimulus.disposed"))
    twice = [i for i, n in disp.items() if n > 1]
    open_ = [i for i in acc if i not in disp]
    return {"pass": not twice, "accepted": len(acc), "disposed": len(disp), "disposed_twice": twice,
            "open_at_stop": open_, "dispositions": dict(Counter(d["disposition"] for d in of(lines, "stimulus.disposed")))}


def l7(lines):
    asks = of(lines, "desk.ask")
    dec = [d for d in lines if d["kind"].startswith("decision.")]
    by_jev = [d for d in dec if "jev" in d.get("by", {})]
    no_prov = [d for d in dec if not d.get("by")]
    answered = [a for a in asks if a["outcome"] == "answered"]
    fam = defaultdict(lambda: Counter())
    for d in dec:
        if "agree" in d:
            fam[d["kind"].split(".", 1)[1]]["agree" if d["agree"] else "disagree"] += 1
    spent = sum(a.get("cost_usd", 0) for a in asks)
    charged = sum(max(a.get("cost_usd", 0), a.get("est_usd", 0)) if (a["outcome"] == "timeout" or a["outcome"].startswith("undecided")) else a.get("cost_usd", 0) for a in asks)
    wall = [a["wall_us"] / 1000 for a in answered]
    return {"pass": not by_jev and not no_prov and charged <= 0.25, "asks": len(asks),
            "outcomes": dict(Counter(a["outcome"] for a in asks)), "decisions": len(dec),
            "decisions_by": dict(Counter(json.dumps(d["by"], sort_keys=True) if "rule" in d["by"] else "jev" for d in dec)),
            "by_jev": len(by_jev), "per_family": {k: dict(v) for k, v in sorted(fam.items())},
            "jev_ms_p50": pct(wall, 50), "jev_ms_p95": pct(wall, 95), "jev_ms_max": max(wall) if wall else None,
            "jev_cost_usd": round(spent, 6), "jev_charged_usd": round(charged, 6)}


OPEN_RE = re.compile(r'^\d+\s+(open|openat|openat2|creat)\((?:[^,]+, )?"([^"]*)", ([^,)]*)')
PATH_CALLS = {"mkdir", "mkdirat", "rename", "renameat", "renameat2", "unlink", "unlinkat", "rmdir",
              "link", "linkat", "symlink", "symlinkat", "truncate", "chmod", "fchmodat"}
CALL_RE = re.compile(r'^\d+\s+([a-z0-9_]+)\((.*)')
STR_RE = re.compile(r'"([^"]*)"')


def l8(run, lines):
    # The state directory the host ran with (a collected copy keeps the
    # original paths in its traces), from the rendered config.
    state = str((run / "state").resolve())
    cfg = run / "rung-host.yaml"
    if cfg.exists():
        m = re.search(r"^state:\s*(\S+)", cfg.read_text(), re.M)
        if m:
            state = m.group(1)
    writes, outside = Counter(), Counter()
    for t in sorted(glob.glob(str(run / "trace" / "*.strace"))):
        for raw in open(t, errors="replace"):
            m = CALL_RE.match(raw)
            if not m:
                continue
            call, rest = m.group(1), m.group(2)
            paths = []
            if call in ("open", "openat", "openat2", "creat"):
                o = OPEN_RE.match(raw)
                if not o:
                    continue
                flags = o.group(3)
                if call != "creat" and not re.search(r"O_WRONLY|O_RDWR|O_CREAT|O_TRUNC", flags):
                    continue
                paths = [o.group(2)]
            elif call in PATH_CALLS:
                paths = STR_RE.findall(rest)
            else:
                continue
            for p in paths:
                if not p.startswith("/"):
                    p = "(relative) " + p
                writes[call] += 1
                if not p.startswith(state + "/") and p != state and p != "/dev/null":
                    outside[f"{call} {p}"] += 1
    agent_writes, agent_outside = 0, []
    ws = state + "/workspace"
    for d in of(lines, "turn.log"):
        for msg in d.get("messages", []):
            for c in msg.get("content", []) if isinstance(msg.get("content"), list) else []:
                if c.get("type") == "tool_use" and c.get("name") in ("ws_write", "ws_remove"):
                    agent_writes += 1
    refused = Counter(d["name"] for d in of(lines, "tool.refused"))
    return {"pass": not outside, "write_syscalls": dict(writes), "outside": dict(outside),
            "agent_write_calls": agent_writes, "agent_tool_refusals": dict(refused), "workspace": ws,
            "traces": len(glob.glob(str(run / "trace" / "*.strace")))}


def l9(lines):
    llm = sum(c.get("cost_usd") or 0 for c in of(lines, "llm.call"))
    jev = l7(lines)
    return {"pass": llm == 0 and jev["jev_charged_usd"] <= 0.25, "agent_usd": round(llm, 6),
            "jev_usd": jev["jev_cost_usd"], "jev_charged_usd": jev["jev_charged_usd"],
            "total_usd": round(llm + jev["jev_charged_usd"], 6)}


def extra(lines):
    calls = of(lines, "llm.call")
    lat = [c["latency_ms"] for c in calls]
    deg = Counter(d["class"] for d in of(lines, "degraded"))
    waited = defaultdict(int)
    open_deg = {}
    for d in lines:
        if d["kind"] == "degraded":
            open_deg = d
        elif d["kind"] == "degraded.ended" and open_deg:
            waited[open_deg["class"]] += d.get("waited_ms", 0)
    tools = Counter(d["name"] for d in of(lines, "tool.call"))
    return {"llm_calls": len(calls), "llm_latency_ms_p50": pct(lat, 50), "llm_latency_ms_p95": pct(lat, 95),
            "served": dict(Counter(c["model_served"] for c in calls)),
            "completion_tokens": sum(c.get("completion_tokens", 0) for c in calls),
            "reasoning_tokens": sum(c.get("reasoning_tokens", 0) for c in calls),
            "degraded": dict(deg), "degraded_waited_s": {k: round(v / 1000) for k, v in waited.items()},
            "tool_calls": dict(tools), "turns": len(of(lines, "turn.started")),
            "turn_kinds": dict(Counter(d["turn_kind"] for d in of(lines, "turn.started"))),
            "outbox": len(of(lines, "outbox.queued")), "rollovers": len(of(lines, "epoch.rollover")),
            "kernel": dict(Counter(d["kind"] for d in lines if d["kind"].startswith("kernel."))),
            "calendar_fired": len(of(lines, "calendar.fired"))}


def main():
    run = Path(sys.argv[1]).resolve()
    lines = load(run)
    res = {"run": run.name, "lines": len(lines), "L1": l1(run, lines), "L2": l2(lines), "L3": l3(lines),
           "L4": l4(lines), "L5": l5(lines), "L6": l6(lines), "L7": l7(lines), "L8": l8(run, lines),
           "L9": l9(lines), "extra": extra(lines)}
    for k in ("L1", "L2", "L3", "L4", "L5", "L6", "L7", "L8", "L9"):
        v = res[k]
        verdict = {True: "PASS", False: "FAIL", None: "NOT EXERCISED"}[v["pass"]]
        brief = {kk: vv for kk, vv in v.items() if kk not in ("pass", "rows", "switches", "listing")}
        print(f"{k} {verdict} {json.dumps(brief)[:400]}")
    if "--json" in sys.argv:
        Path(sys.argv[sys.argv.index("--json") + 1]).write_text(json.dumps(res, indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
