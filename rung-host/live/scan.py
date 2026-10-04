#!/usr/bin/env python3
"""Count occurrences of the router key's value under the given paths.

usage: doppler run ... --only-secrets OPENROUTER_API_KEY -- scan.py PATH...

Prints only counts and file names, never the value or any part of it.
"""
import os
import sys
from pathlib import Path


def main():
    key = os.environ.get("OPENROUTER_API_KEY", "").strip()
    if len(key) < 16:
        print("scan: no key in the environment", file=sys.stderr)
        return 2
    needle = key.encode()
    files = hits = 0
    for root in sys.argv[1:]:
        p = Path(root)
        for f in [p] if p.is_file() else p.rglob("*"):
            if not f.is_file():
                continue
            files += 1
            n = f.read_bytes().count(needle)
            if n:
                hits += n
                print(f"HIT {n} {f}")
    print(f"scan: {files} files, {hits} occurrences of the key")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
