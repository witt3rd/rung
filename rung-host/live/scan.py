#!/usr/bin/env python3
"""Count occurrences of the router keys' values under the given paths.

usage: doppler run ... --only-secrets RUNG_HOST_OPENROUTER_API_KEY -- scan.py PATH...

Scans for every key in SCAN_KEYS (comma-separated env var names, default
RUNG_HOST_OPENROUTER_API_KEY,OPENROUTER_API_KEY) that the environment holds;
at least one must be present. Prints only counts, env var names and file
names, never a value or any part of one.
"""
import os
import sys
from pathlib import Path


def main():
    names = os.environ.get("SCAN_KEYS", "RUNG_HOST_OPENROUTER_API_KEY,OPENROUTER_API_KEY").split(",")
    needles = {}
    for name in names:
        key = os.environ.get(name.strip(), "").strip()
        if len(key) >= 16:
            needles[name.strip()] = key.encode()
    if not needles:
        print("scan: no key in the environment", file=sys.stderr)
        return 2
    files = hits = 0
    for root in sys.argv[1:]:
        p = Path(root)
        for f in [p] if p.is_file() else p.rglob("*"):
            if not f.is_file():
                continue
            files += 1
            data = f.read_bytes()
            for name, needle in needles.items():
                n = data.count(needle)
                if n:
                    hits += n
                    print(f"HIT {n} {name} {f}")
    print(f"scan: {files} files, keys {','.join(needles)}, {hits} occurrences")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
