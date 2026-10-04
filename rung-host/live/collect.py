#!/usr/bin/env python3
"""Copy a run's evidence into the repository: the record, exit codes,
window and driver logs, the agent's workspace and memory, and the syscall
traces cut to the write-side calls the analyzer reads (L8).

usage: collect.py RUN_DIR DEST_DIR

The result is a run directory analyze.py reads the same way. Host logs are
copied as they are (they hold no key: live.sh never passes one on a command
line, and scan.py checks the copy).
"""
import re
import shutil
import sys
from pathlib import Path

WRITE_OPEN = re.compile(r"O_WRONLY|O_RDWR|O_CREAT|O_TRUNC")
CALLS = ("mkdir", "mkdirat", "rename", "renameat", "renameat2", "unlink", "unlinkat", "rmdir",
         "link", "linkat", "symlink", "symlinkat", "truncate", "chmod", "fchmodat", "creat")
CALL = re.compile(r"^\d+\s+([a-z0-9_]+)\(")


def keep(line):
    m = CALL.match(line)
    if not m:
        return False
    c = m.group(1)
    if c in ("open", "openat", "openat2"):
        return bool(WRITE_OPEN.search(line))
    return c in CALLS


def main():
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    dst.mkdir(parents=True, exist_ok=True)
    for name in ("state/record", "state/workspace", "state/memory"):
        if (src / name).exists():
            shutil.copytree(src / name, dst / name, dirs_exist_ok=True)
    for pat in ("exit.*", "host.*.log", "windows.log", "driven.jsonl", "drive.log", "started_at", "rung-host.yaml", "window.out"):
        for f in src.glob(pat):
            shutil.copy2(f, dst / f.name)
    (dst / "trace").mkdir(exist_ok=True)
    for t in sorted((src / "trace").glob("*.strace")):
        with open(t, errors="replace") as i, open(dst / "trace" / t.name, "w") as o:
            o.writelines(l for l in i if keep(l))
    return 0


if __name__ == "__main__":
    sys.exit(main())
