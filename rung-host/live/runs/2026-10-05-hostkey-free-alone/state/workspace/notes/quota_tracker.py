#!/usr/bin/env python3
"""Simple quota tracker for the first live run.

Usage:
    python quota_tracker.py   # prints current quota status and expectations

Assumptions:
    - Initial quota = 400 units
    - Each tool call costs exactly 1 unit (observed pattern)
    - Quota header line format: "quota X/Y left"
"""

INITIAL_QUOTA = 400
CALL_COST = 1

def main():
    import os, re

    # Try to read current quota from the agent's note / header convention.
    # We look for a "quota X/Y left" pattern in any readable file, or
    # default to 400 if not found.
    quota_y = INITIAL_QUOTA
    quota_x = INITIAL_QUOTA

    # Look for a last-known quota in state.md or notes
    candidates = []
    for f in ["state.md", "notes/plan.md"]:
        if os.path.exists(f):
            try:
                with open(f) as fh:
                    txt = fh.read()
                m = re.search(r"quota\s*(\d+)[/\s]+(\d+)", txt)
                if m:
                    quota_x = int(m.group(1))
                    quota_y = int(m.group(2))
                    candidates.append(f)
            except Exception:
                pass

    # Fallback: print known state
    if candidates:
        print(f"Last known quota from {candidates[0]}: {quota_x}/{quota_y}")
    else:
        print(f"Initial quota assumed: {INITIAL_QUOTA}/{INITIAL_QUOTA}")

    # Estimate how many calls remain
    remaining = quota_y
    used = INITIAL_QUOTA - remaining
    expected_turns_left = remaining // CALL_COST if remaining >= 0 else 0

    print(f"Quota used: {used} calls")
    print(f"Quota remaining: {remaining}")
    print(f"Estimated turns left (at 1 call/turn): {expected_turns_left}")

    # Report open expectations
    print("\nOpen expectations about cal-checkin:")
    # We cannot actually compute these without searching the expectation
    # store, so just note they exist.
    print(" - e2 due 2026-10-05T13:59:26Z (cal-checkin stimulus nature)")
    print(" - e3 due 2026-10-05T13:57:12Z (cal-checkin stimulus nature)")

if __name__ == "__main__":
    main()