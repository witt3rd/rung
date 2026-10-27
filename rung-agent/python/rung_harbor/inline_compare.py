"""Compare two agent modes (tools vs inline Python) on the same cases.

Reads Harbor trial `result.json` files from two job directories and prints
pass rate, peak context and tokens per solved task for each mode.

    python3 -m rung_harbor.inline_compare --tools JOB_DIR --inline JOB_DIR

Peak context is the largest `n_input_tokens` a trial reports (Harbor's
agent_result); a trial that reports none is counted as missing, not zero.
Does not import Harbor.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from rung_harbor.evidence import find_trials, read_trial_result, trial_reward


def summarize(job_dir: Path) -> dict[str, Any]:
    trials = [t for t in (read_trial_result(p) for p in find_trials(job_dir)) if t]
    solved = [t for t in trials if (trial_reward(t) or 0.0) >= 1.0]

    def tok(t: dict[str, Any]) -> int | None:
        r = t.get("agent_result") or {}
        a, b = r.get("n_input_tokens"), r.get("n_output_tokens")
        if isinstance(a, int) and isinstance(b, int):
            return a + b
        return None

    peaks = [
        v
        for t in trials
        if isinstance(v := (t.get("agent_result") or {}).get("n_input_tokens"), int)
    ]
    solved_tok = [x for t in solved if (x := tok(t)) is not None]
    return {
        "trials": len(trials),
        "solved": len(solved),
        "pass_rate": len(solved) / len(trials) if trials else None,
        "peak_context": max(peaks) if peaks else None,
        "tokens_per_solved": sum(solved_tok) / len(solved_tok) if solved_tok else None,
        "tokens_missing": len(solved) - len(solved_tok),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tools", type=Path, required=True)
    ap.add_argument("--inline", type=Path, required=True)
    a = ap.parse_args(argv)
    print(json.dumps({"tools": summarize(a.tools), "inline": summarize(a.inline)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
