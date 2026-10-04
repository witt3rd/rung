#!/usr/bin/env python3
"""Write the owner's planned messages into the host's inbox when they fall due.

usage: drive.py RUN_DIR STIMULI_JSON

Offsets are seconds after the run's first start (RUN_DIR/started_at, written
by live.sh). Each message is written to a temporary name and renamed to
`<id>.msg`, so the host never reads half a file. What was written, and when,
goes to RUN_DIR/driven.jsonl. The driver stops when every message is out, or
when RUN_DIR/STOP or RUN_DIR/DRIVE_OFF exists.
"""
import json
import os
import sys
import time
from pathlib import Path


def main():
    run, plan = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text())
    start = float((run / "started_at").read_text().strip())
    inbox = run / "state" / "inbox"
    log = run / "driven.jsonl"
    done = set()
    if log.exists():
        done = {json.loads(l)["id"] for l in log.read_text().splitlines() if l.strip()}
    pending = sorted((m for m in plan["messages"] if m["id"] not in done), key=lambda m: m["at_s"])
    while pending:
        if (run / "STOP").exists() or (run / "DRIVE_OFF").exists():
            return 0
        now = time.time()
        while pending and now - start >= pending[0]["at_s"]:
            m = pending.pop(0)
            inbox.mkdir(parents=True, exist_ok=True)
            tmp = inbox / f".{m['id']}.tmp"
            tmp.write_text(json.dumps({"role": "owner", "text": m["text"]}))
            os.rename(tmp, inbox / f"{m['id']}.msg")
            with log.open("a") as f:
                f.write(json.dumps({"id": m["id"], "at_s": m["at_s"], "written_ms": int(now * 1000)}) + "\n")
        time.sleep(1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
