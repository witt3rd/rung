"""Run the capability ladder. Harbor is the harness; this picks the next case.

    PYTHONPATH=<rung>/rung-agent/python \\
      python3 -m rung_harbor.validate list
    PYTHONPATH=<rung>/rung-agent/python \\
      doppler run -p fleet -c dev_work -- \\
      python3 -m rung_harbor.validate run cwd-capture
    PYTHONPATH=<rung>/rung-agent/python \\
      doppler run -p fleet -c dev_work -- \\
      python3 -m rung_harbor.validate next
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from rung_harbor.suite import (
    CASES,
    PHASE_2,
    SKIPPED_HARNESS,
    SUITE_MODEL,
    Case,
    case_by_id,
    live_cases,
)

AGENT = "rung_harbor.agent:RungAgent"


def harbor_root() -> Path:
    env = os.environ.get("HARBOR_ROOT")
    if env:
        return Path(env).expanduser()
    return Path.home() / "src" / "ext" / "harbor"


def jobs_dir() -> Path:
    return harbor_root() / "jobs"


def trial_rewards(jobs: Path) -> dict[str, list[float]]:
    """task_name -> rewards seen in any trial result.json under jobs/."""
    out: dict[str, list[float]] = {}
    if not jobs.is_dir():
        return out
    for result in jobs.glob("*/*/result.json"):
        try:
            data = json.loads(result.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        name = data.get("task_name")
        rewards = (data.get("verifier_result") or {}).get("rewards") or {}
        reward = rewards.get("reward")
        if not isinstance(name, str) or not isinstance(reward, (int, float)):
            continue
        out.setdefault(name, []).append(float(reward))
    return out


def latest_reward(rewards: dict[str, list[float]], task_name: str) -> float | None:
    got = rewards.get(task_name)
    if not got:
        return None
    return got[-1]


def status_row(case: Case, rewards: dict[str, list[float]]) -> str:
    if case.skip:
        return f"{case.id:16} SKIP  {case.skip}"
    r = latest_reward(rewards, case.task_name)
    if r is None:
        mark = "—"
    elif r >= 1.0:
        mark = "pass"
    else:
        mark = f"fail({r})"
    tools = ",".join(case.tools)
    return f"{case.id:16} {mark:8} {case.capability}  [{tools}]"


def cmd_list(_: argparse.Namespace) -> int:
    rewards = trial_rewards(jobs_dir())
    print(f"model  {SUITE_MODEL}")
    print(f"harbor {harbor_root()}")
    print()
    for c in CASES:
        print(status_row(c, rewards))
    print()
    print("not agent tests:")
    for line in SKIPPED_HARNESS:
        print(f"  - {line}")
    print(f"phase 2 (after this ladder): harbor run -d {PHASE_2}")
    return 0


def next_unproven(rewards: dict[str, list[float]]) -> Case | None:
    for c in live_cases():
        r = latest_reward(rewards, c.task_name)
        if r is None or r < 1.0:
            return c
    return None


def run_case(case: Case, model: str) -> int:
    if case.skip:
        print(f"skip {case.id}: {case.skip}", file=sys.stderr)
        return 2
    root = harbor_root()
    path = root / case.path
    if not path.is_dir():
        print(f"missing task {path}", file=sys.stderr)
        return 2
    env = os.environ.copy()
    py = str(Path(__file__).resolve().parents[1])
    env["PYTHONPATH"] = os.pathsep.join(filter(None, [py, env.get("PYTHONPATH")]))
    cmd = [
        "uv",
        "run",
        "--directory",
        str(root),
        "harbor",
        "run",
        "-p",
        str(path),
        "--agent",
        AGENT,
        "--model",
        model,
        "-n",
        "1",
    ]
    print(" ".join(cmd), file=sys.stderr)
    return subprocess.call(cmd, env=env)


def cmd_run(ns: argparse.Namespace) -> int:
    return run_case(case_by_id(ns.id), ns.model)


def cmd_next(ns: argparse.Namespace) -> int:
    rewards = trial_rewards(jobs_dir())
    nxt = next_unproven(rewards)
    if nxt is None:
        print("ladder green. next is phase 2:", PHASE_2)
        return 0
    print(f"next {nxt.id}: {nxt.capability}", file=sys.stderr)
    return run_case(nxt, ns.model)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="rung_harbor.validate")
    p.add_argument(
        "--model",
        default=os.environ.get("RUNG_HARBOR_MODEL", SUITE_MODEL),
        help="Harbor --model (default: suite flash)",
    )
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list", help="show the ladder and last known rewards")
    run_p = sub.add_parser("run", help="run one case")
    run_p.add_argument("id")
    sub.add_parser("next", help="run the first live case that is not yet reward 1.0")
    ns = p.parse_args(argv)
    if ns.cmd == "list":
        return cmd_list(ns)
    if ns.cmd == "run":
        return cmd_run(ns)
    if ns.cmd == "next":
        return cmd_next(ns)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
