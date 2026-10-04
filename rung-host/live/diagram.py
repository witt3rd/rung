#!/usr/bin/env python3
"""Draw the evidence report's one diagram from a run's record: where the
window's time went, rung by rung.

usage: diagram.py RUN_DIR OUT.svg
"""
import glob
import json
import sys
from pathlib import Path

W, LEFT, RIGHT, TOP, ROW = 1560, 500, 40, 20, 62
INK, SOFT, RULE, ACCENT, FILL = "#1d2433", "#4a5163", "#dcd9d0", "#5b4bd6", "#ece9fb"
OK, BAD, WAIT = "#2f7d4f", "#b4462b", "#d9d4c7"


def main():
    run, out = Path(sys.argv[1]), Path(sys.argv[2])
    lines = []
    for f in sorted(glob.glob(str(run / "state" / "record" / "*"))):
        lines += [json.loads(l) for l in open(f) if l.strip()]
    lines.sort(key=lambda d: d["seq"])
    t0 = lines[0]["at"]
    end = (lines[-1]["at"] - t0) / 1000
    ladder = next(d for d in lines if d["kind"] == "host.start")["config"]["ladder"]
    listing = next(d for d in lines if d["kind"] == "ladder.listed")
    span = W - LEFT - RIGHT
    x = lambda s: LEFT + span * s / end
    rows = ladder + ["waiting (backoff)", "owner messages"]
    h = TOP + ROW * len(rows) + 90
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {h}" font-family="Publishing Sans">',
           f'<g font-size="24" fill="{INK}">']
    for i, r in enumerate(rows):
        y = TOP + ROW * i
        svg.append(f'<line x1="{LEFT}" y1="{y + ROW / 2}" x2="{W - RIGHT}" y2="{y + ROW / 2}" stroke="{RULE}" stroke-width="2"/>')
        label = r.split("/")[-1] if "/" in r else r
        weight = "700" if i < len(ladder) else "400"
        svg.append(f'<text x="{LEFT - 16}" y="{y + ROW / 2 + 6}" text-anchor="end" font-size="22" font-weight="{weight}">{label}</text>')
    # The axis, in minutes.
    ya = TOP + ROW * len(rows) + 20
    for m in range(0, int(end // 60) + 1, 5):
        svg.append(f'<line x1="{x(m * 60)}" y1="{TOP}" x2="{x(m * 60)}" y2="{ya - 10}" stroke="{RULE}" stroke-width="1" stroke-dasharray="4 6"/>')
        svg.append(f'<text x="{x(m * 60)}" y="{ya + 16}" text-anchor="middle" fill="{SOFT}" font-size="22">{m} min</text>')
    started, wait = {}, None
    for d in lines:
        t = (d["at"] - t0) / 1000
        k = d["kind"]
        if k == "turn.started":
            started[d["turn"]] = (t, d["rung"])
        elif k == "turn.ended":
            s, r = started[d["turn"]]
            y = TOP + ROW * r + ROW / 2
            colour = OK if d["status"] == "completed" else BAD
            w = max(x(t) - x(s), 6)
            svg.append(f'<rect x="{x(s) - (3 if w == 6 else 0)}" y="{y - 16}" width="{w}" height="32" rx="4" fill="{colour}"/>')
        elif k == "degraded":
            wait = t
        elif k == "degraded.ended" and wait is not None:
            y = TOP + ROW * len(ladder) + ROW / 2
            svg.append(f'<rect x="{x(wait)}" y="{y - 14}" width="{max(x(t) - x(wait), 2)}" height="28" rx="3" fill="{WAIT}"/>')
            wait = None
        elif k == "stimulus.accepted" and d["item"].get("role") == "owner":
            y = TOP + ROW * (len(ladder) + 1) + ROW / 2
            svg.append(f'<circle cx="{x(t)}" cy="{y}" r="9" fill="{FILL}" stroke="{ACCENT}" stroke-width="3"/>')
        elif k == "stimulus.disposed" and d["disposition"] == "answered":
            y = TOP + ROW * (len(ladder) + 1) + ROW / 2
            svg.append(f'<circle cx="{x(t)}" cy="{y}" r="9" fill="{ACCENT}"/>')
    # Each rung's verdict, from the listing and the router.
    why = {}
    for d in lines:
        if d["kind"] == "turn.ended" and (d.get("failure") or {}).get("unroutable"):
            why[d["rung"]] = "listed; the router refused it for this account"
    for r in listing["rungs"]:
        if r["rung"] not in why:
            why[r["rung"]] = "listed; routes (provider 429s upstream)" if r["available"] else f"dropped by the listing ({r['why']})"
    for i in range(len(ladder)):
        y = TOP + ROW * i + ROW / 2
        svg.append(f'<text x="{LEFT - 16}" y="{y + 30}" text-anchor="end" fill="{SOFT}" font-size="17">{why.get(i, "")}</text>')
    yl = h - 22
    items = [(OK, "turn completed"), (BAD, "turn failed"), (WAIT, "backoff wait")]
    lx = 60
    for c, t in items:
        svg.append(f'<rect x="{lx}" y="{yl - 18}" width="26" height="22" rx="4" fill="{c}"/>')
        svg.append(f'<text x="{lx + 36}" y="{yl}" font-size="22">{t}</text>')
        lx += 250
    svg.append(f'<circle cx="{lx + 10}" cy="{yl - 7}" r="9" fill="{FILL}" stroke="{ACCENT}" stroke-width="3"/>')
    svg.append(f'<text x="{lx + 28}" y="{yl}" font-size="22">owner message arrives</text>')
    lx += 300
    svg.append(f'<circle cx="{lx + 10}" cy="{yl - 7}" r="9" fill="{ACCENT}"/>')
    svg.append(f'<text x="{lx + 28}" y="{yl}" font-size="22">answered</text>')
    svg.append("</g></svg>")
    out.write_text("\n".join(svg) + "\n")


if __name__ == "__main__":
    main()
