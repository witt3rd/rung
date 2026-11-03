#!/usr/bin/env python3
"""Records are append-only history: docs/records/ may grow, never be rewritten.

Compares docs/records/** with a base ref (default origin/master, or argv[1]).
A file that exists at the base must keep its old text byte for byte as a
prefix; what follows must begin with a heading that carries a date
(`## Moved, 2026-10-29`, `## 2026-11-01 correction`). New files are free.
One retargeting is allowed: this repository has no README.md anywhere, so the
file name `README.md` is read as `AGENTS.md` on both sides of the comparison (a
link follows its renamed target). Likewise a link follows a moved document:
RETARGETS maps a new link path to the old one, read as equal on both sides.
Nothing else in old text may change.
Run locally:  docs/_records_guard.py [BASE_REF]
"""
import re
import subprocess
import sys

BASE = sys.argv[1] if len(sys.argv) > 1 else "origin/master"
# new path -> old path, for documents that moved out of docs/ root
RETARGETS = {"../../reference/rung-host.md": "../../rung-host.md"}
DATED = re.compile(r"\n+#{2,6} [^\n]*\d{4}-\d{2}-\d{2}")


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, check=True).stdout


def main():
    base = git("merge-base", BASE, "HEAD").decode().strip()
    # -M: a pure rename keeps its history; its old text is compared at the new path
    changed = git("diff", "--name-status", "-M", base, "HEAD", "--", "docs/records/").decode().splitlines()
    bad = []
    for row in changed:
        parts = row.split("\t")
        status = parts[0][0]
        if status == "A":
            continue
        if status == "D":
            bad.append(f"{parts[1]}: a record was deleted")
            continue
        old_path, new_path = (parts[1], parts[-1])
        old = git("show", f"{base}:{old_path}").decode()
        new = git("show", f"HEAD:{new_path}").decode()
        old = old.replace("README.md", "AGENTS.md")
        new = new.replace("README.md", "AGENTS.md")
        for moved, was in RETARGETS.items():
            new = new.replace(moved, was)
        if new == old:
            continue
        if not new.startswith(old):
            bad.append(f"{new_path}: existing text was changed, not appended to")
        elif not DATED.match(old[-1:] + new[len(old):]):
            bad.append(f"{new_path}: appended text must begin with a heading that carries a date")
    if bad:
        print("records-guard: FAIL (docs/records/ is append-only)")
        for b in bad:
            print("  " + b)
        sys.exit(1)
    print("records-guard: ok")


main()
