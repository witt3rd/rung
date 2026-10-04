#!/usr/bin/env python3
"""One-way rule guard: rung takes no dependency on any consumer.

Fails when a tracked file names a consumer-specific term (code, tests, docs,
error text). Run locally with one command:  docs/_consumer_guard.py
Paths exempt from the scan live in docs/_consumer_guard.allow, one
`path-prefix  # reason` per line. Patterns are case-insensitive, except the
camelCase `venue` identifier form. Prose "venue" (publishing venue) is not a
hit; `venue` counts only inside identifiers (snake_case, SCREAMING_CASE, camelCase).
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PATTERNS = [
    re.compile(r"spire|host-gw|janus|HOST_TOKEN|HOST_VENUE_KEY|agent-binding", re.I),
    # named consumer systems, and real people's names in test text/fixtures
    re.compile(r"animus|mohak|donald@|thompson <|downstream host", re.I),
    # consumer/product/agent names (whole-word, so `forged`/`unforgeable` stay clean)
    # and personal names used as principal ids or attributions
    re.compile(r"\b(augur|cookie|forge|donald|thompson|outer-loop)s?\b", re.I),
    re.compile(r"[a-z0-9]_venue|venue_[a-z0-9]", re.I),
    re.compile(r"[a-z]Venue|venue[A-Z]"),  # camelCase identifiers, case-sensitive
]


def allowed_prefixes():
    out = []
    for line in (ROOT / "docs/_consumer_guard.allow").read_text().splitlines():
        entry = line.split("#", 1)[0].strip()
        if entry:
            out.append(entry)
    return out


def main():
    allow = allowed_prefixes()
    files = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    hits = []
    for f in filter(None, files):
        if any(f == a or f.startswith(a) for a in allow):
            continue
        try:
            text = (ROOT / f).read_text()
        except (UnicodeDecodeError, OSError):
            continue
        for n, line in enumerate(text.splitlines(), 1):
            if any(p.search(line) for p in PATTERNS):
                hits.append(f"{f}:{n}: {line.strip()[:120]}")
    for h in hits:
        print(h)
    if hits:
        print(f"consumer-guard: {len(hits)} hit(s); rung must not name consumers (AGENTS.md)", file=sys.stderr)
        return 1
    print("consumer-guard: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
