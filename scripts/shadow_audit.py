#!/usr/bin/env python3
"""Shadow-week tally: per decision family agree/disagree, plus an audit sheet.

usage: shadow_audit.py RECORD.ndjson [-n 50] [--seed S] [-o audit.md]

Reads `decision.<family>` lines that carry `agree` (shadow mode), prints the
tally table, and writes N random disagreements to a markdown sheet.
"""
import argparse
import json
import random
import sys
from collections import defaultdict


def load(path):
    out = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
            except ValueError:
                continue  # torn last line
            if str(r.get("kind", "")).startswith("decision.") and "agree" in r:
                out.append(r)
    return out


def tally(rows):
    t = defaultdict(lambda: [0, 0])
    for r in rows:
        t[r["kind"][len("decision."):]][0 if r["agree"] else 1] += 1
    return dict(sorted(t.items()))


def table(t):
    lines = [f"{'family':<20}{'agree':>8}{'disagree':>10}{'rate':>8}"]
    ta = td = 0
    for fam, (a, d) in t.items():
        lines.append(f"{fam:<20}{a:>8}{d:>10}{a / (a + d):>8.0%}")
        ta, td = ta + a, td + d
    if ta + td:
        lines.append(f"{'total':<20}{ta:>8}{td:>10}{ta / (ta + td):>8.0%}")
    return "\n".join(lines)


def sheet(rows, n, seed):
    dis = [r for r in rows if not r["agree"]]
    pick = random.Random(seed).sample(dis, min(n, len(dis)))
    out = [f"# Shadow audit sheet\n\n{len(pick)} of {len(dis)} disagreements.\n"]
    for i, r in enumerate(pick, 1):
        out.append(
            f"## {i}. {r['kind'][9:]} — seq {r.get('seq')}, turn {r.get('turn')}\n\n"
            f"- rule: `{json.dumps(r.get('choice'), sort_keys=True)}`\n"
            f"- Jev: `{json.dumps(r.get('jev_choice'), sort_keys=True)}`\n"
            f"- input_hash: `{r.get('input_hash')}`\n"
            f"- verdict (rule / Jev / neither): \n"
            f"- note: \n"
        )
    return "\n".join(out)


def main(argv=None):
    p = argparse.ArgumentParser()
    p.add_argument("record")
    p.add_argument("-n", type=int, default=50)
    p.add_argument("--seed", type=int)
    p.add_argument("-o", default="shadow-audit.md")
    a = p.parse_args(argv)
    rows = load(a.record)
    print(table(tally(rows)))
    with open(a.o, "w") as f:
        f.write(sheet(rows, a.n, a.seed))
    print(f"\nwrote {a.o}")


if __name__ == "__main__":
    sys.exit(main())
