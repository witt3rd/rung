"""Run the capability ladder. Evidence lands in `rung-agent/harbor-runs/`.

    PYTHONPATH=<rung>/rung-agent/python python3 -m rung_harbor.validate list
    PYTHONPATH=<rung>/rung-agent/python \\
      doppler run -p fleet -c dev_work -- \\
      python3 -m rung_harbor.validate run cwd-capture
    PYTHONPATH=<rung>/rung-agent/python \\
      doppler run -p fleet -c dev_work -- \\
      python3 -m rung_harbor.validate next
    PYTHONPATH=<rung>/rung-agent/python python3 -m rung_harbor.validate import
    PYTHONPATH=<rung>/rung-agent/python python3 -m rung_harbor.validate show cwd-capture
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

from rung_harbor.evidence import (
    append_index,
    find_trials,
    job_name,
    latest_by_case,
    latest_reward,
    load_index,
    newest_trial_for_task,
    read_trial_result,
    record_from_trial,
    runs_root,
    trial_reward,
    utc_stamp,
)
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


def status_row(case: Case, latest: dict[str, dict[str, Any]]) -> str:
    if case.skip:
        return f"{case.id:16} SKIP  {case.skip}"
    rec = latest.get(case.id)
    r = latest_reward(latest, case.id)
    if rec is None:
        mark = "—"
    elif r is None:
        mark = "ran"
    elif r >= 1.0:
        mark = "pass"
    else:
        mark = f"fail({r})"
    tools = ",".join(case.tools)
    return f"{case.id:16} {mark:8} {case.capability}  [{tools}]"


def cmd_list(_: argparse.Namespace) -> int:
    latest = latest_by_case()
    print(f"model    {SUITE_MODEL}")
    print(f"harbor   {harbor_root()}")
    print(f"evidence {runs_root()}")
    print()
    for c in CASES:
        print(status_row(c, latest))
    print()
    print("not agent tests:")
    for line in SKIPPED_HARNESS:
        print(f"  - {line}")
    print(f"phase 2 (after this ladder): harbor run -d {PHASE_2}")
    n = len(load_index())
    print(f"attempts indexed: {n}")
    return 0


def next_unproven(latest: dict[str, dict[str, Any]]) -> Case | None:
    for c in live_cases():
        r = latest_reward(latest, c.id)
        if r is None or r < 1.0:
            return c
    return None


def pythonpath_env() -> dict[str, str]:
    env = os.environ.copy()
    py = str(Path(__file__).resolve().parents[1])
    env["PYTHONPATH"] = os.pathsep.join(filter(None, [py, env.get("PYTHONPATH")]))
    return env


def harbor_run(case: Case, model: str, dest_parent: Path, name: str) -> int:
    root = harbor_root()
    path = root / case.path
    if not path.is_dir():
        print(f"missing task {path}", file=sys.stderr)
        return 2
    dest_parent.mkdir(parents=True, exist_ok=True)
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
        "-o",
        str(dest_parent.resolve()),
        "--job-name",
        name,
    ]
    print(" ".join(cmd), file=sys.stderr)
    return subprocess.call(cmd, env=pythonpath_env())


def index_job(
    case: Case,
    model: str,
    job_dir: Path,
    stamp: str,
    harbor_exit: int,
) -> dict[str, Any] | None:
    trials = find_trials(job_dir)
    trial = trials[-1] if trials else None
    data = read_trial_result(trial) if trial else None
    reward = trial_reward(data) if data else None
    rec: dict[str, Any] = {
        "id": case.id,
        "task_name": case.task_name,
        "model": model,
        "reward": reward,
        "dir": str((trial or job_dir).resolve()),
        "job_dir": str(job_dir.resolve()),
        "stamp": stamp,
        "job_name": job_dir.name,
        "harbor_exit": harbor_exit,
        "capability": case.capability,
    }
    append_index(rec)
    return rec


def run_case(case: Case, model: str) -> int:
    if case.skip:
        print(f"skip {case.id}: {case.skip}", file=sys.stderr)
        return 2
    stamp = utc_stamp()
    name = job_name(case.id, stamp)
    dest = runs_root()
    before = {p.name for p in dest.iterdir() if p.is_dir()} if dest.is_dir() else set()
    code = harbor_run(case, model, dest, name)
    job = dest / name
    if not job.is_dir():
        after = {p.name for p in dest.iterdir() if p.is_dir()} if dest.is_dir() else set()
        created = after - before
        if len(created) == 1:
            job = dest / created.pop()
    if job.is_dir():
        rec = index_job(case, model, job, stamp, code)
        if rec:
            print(
                f"evidence {rec['dir']}  reward={rec['reward']!r}  exit={code}",
                file=sys.stderr,
            )
    else:
        print(f"harbor wrote no job dir at {job}", file=sys.stderr)
    return code


def cmd_run(ns: argparse.Namespace) -> int:
    return run_case(case_by_id(ns.id), ns.model)


def cmd_next(ns: argparse.Namespace) -> int:
    nxt = next_unproven(latest_by_case())
    if nxt is None:
        print("ladder green. next is phase 2:", PHASE_2)
        return 0
    print(f"next {nxt.id}: {nxt.capability}", file=sys.stderr)
    return run_case(nxt, ns.model)


def cmd_import(ns: argparse.Namespace) -> int:
    """Copy the newest Harbor job per live case into our evidence tree."""
    jobs = harbor_root() / "jobs"
    n = 0
    for case in live_cases():
        trial = newest_trial_for_task(jobs, case.task_name)
        if trial is None:
            print(f"{case.id}: no Harbor trial for {case.task_name}", file=sys.stderr)
            continue
        rec = record_from_trial(
            case_id=case.id,
            task_name=case.task_name,
            model=ns.model,
            trial=trial,
            stamp=utc_stamp(),
        )
        print(f"{case.id}: reward={rec['reward']!r} -> {rec['dir']}")
        n += 1
    print(f"imported {n} case(s)")
    return 0 if n else 1


def cmd_show(ns: argparse.Namespace) -> int:
    recs = [r for r in load_index() if r.get("id") == ns.id]
    if not recs:
        print(f"no evidence for {ns.id}", file=sys.stderr)
        return 1
    for rec in recs:
        print(
            f"{rec.get('stamp')}  reward={rec.get('reward')!r}  "
            f"exit={rec.get('harbor_exit')!r}  {rec.get('dir')}"
        )
    last = recs[-1]
    log = Path(str(last.get("dir") or "")) / "agent" / "rung-agent.txt"
    if not log.is_file():
        log = Path(str(last.get("dir") or "")) / "rung-agent.txt"
    if log.is_file():
        print()
        print(log.read_text()[-4000:])
    return 0


def cmd_runs(_: argparse.Namespace) -> int:
    root = runs_root()
    if not root.is_dir():
        print(f"no evidence yet at {root}")
        return 0
    for rec in load_index():
        print(
            f"{rec.get('stamp')}  {rec.get('id'):16}  "
            f"reward={rec.get('reward')!r}  {rec.get('job_name')}"
        )
    return 0


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="rung_harbor.validate")
    p.add_argument(
        "--model",
        default=os.environ.get("RUNG_HARBOR_MODEL", SUITE_MODEL),
        help="Harbor --model (default: suite flash)",
    )
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list", help="ladder + latest evidence in this repo")
    run_p = sub.add_parser("run", help="run one case; always writes a new evidence folder")
    run_p.add_argument("id")
    sub.add_parser("next", help="run the first live case that is not yet reward 1.0")
    sub.add_parser("import", help="seed evidence from existing Harbor jobs/")
    show_p = sub.add_parser("show", help="print every attempt for a case id")
    show_p.add_argument("id")
    sub.add_parser("runs", help="list every indexed attempt")
    ns = p.parse_args(argv)
    if ns.cmd == "list":
        return cmd_list(ns)
    if ns.cmd == "run":
        return cmd_run(ns)
    if ns.cmd == "next":
        return cmd_next(ns)
    if ns.cmd == "import":
        return cmd_import(ns)
    if ns.cmd == "show":
        return cmd_show(ns)
    if ns.cmd == "runs":
        return cmd_runs(ns)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
