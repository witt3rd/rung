#!/usr/bin/env python3
"""Every RUNG_* variable the code reads, and every rung-agent flag, is in a guide.

A variable name written as a string literal in a crate's source or tests, and a
`--flag` in the `usage()` text of rung-agent-core/src/args.rs, must appear in
docs/guides/*.md, or be listed in INTERNAL below (test hooks and
process-to-process variables that are not for a person to set).
The GUI crates (gui/) are covered by their own AGENTS.md.
Run locally:  docs/_coverage.py
"""
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
INTERNAL = {
    "RUNG_AGENT_CHILD",        # marks a --background child
    "RUNG_ACP_WORKER",         # marks an ACP worker process
    "RUNG_H4_TEST_CANARY",     # redactor test hook
    "RUNG_H4_TEST_UNSET",      # redactor test hook
    "RUNG_HOST_WRITE_FIXTURES",  # test fixture regeneration
    "RUNG_RECALL_FIXTURES",    # test fixture regeneration
    "RUNG_PARITY_RECORD",      # parity test recording
    "RUNG_SPILL_RSS_CHILD",    # spill test child
}


def tracked(*globs):
    out = subprocess.check_output(["git", "ls-files", *globs], cwd=ROOT, text=True)
    return [ROOT / p for p in out.split()]


def main():
    guides = "\n".join(p.read_text() for p in tracked("docs/guides/*.md"))
    bad = []
    names = set()
    for p in tracked("rung-agent*/**/*.rs", "rung-host/**/*.rs", "rung-gateway/**/*.rs", "rung-memory/**/*.rs", "rung-std/**/*.rs"):
        names.update(re.findall(r'"(RUNG_[A-Z0-9_]+)"', p.read_text()))
    for n in sorted(names - INTERNAL):
        if n not in guides:
            bad.append(f"{n}: read by the code, in no guide and not in INTERNAL (docs/_coverage.py)")
    args = (ROOT / "rung-agent-core/src/args.rs").read_text()
    usage = args[args.index("pub fn usage()"):]
    usage = usage[: usage.index("\n}\n")]
    for f in sorted(set(re.findall(r"(?<![\w-])(--[a-z][a-z-]+)", usage))):
        if f not in guides:
            bad.append(f"{f}: a rung-agent flag in no guide")
    if bad:
        print("coverage: FAIL")
        print("\n".join("  " + b for b in bad))
        sys.exit(1)
    print("coverage: ok")


main()
