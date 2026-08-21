"""Timestamped evidence for the Harbor validation suite.

Harbor remains the harness. This module decides *where* a run lives in
the rung tree and how we index attempts so a case can be redone.

Layout (gitignored except `.gitkeep`)::

    rung-agent/harbor-runs/
      index.jsonl
      2026-08-21T101500Z-cwd-capture/
        result.json
        <trial>/
          result.json
          trial.log
          agent/rung-agent.txt
          verifier/reward.txt

Does not import Harbor.
"""

from __future__ import annotations

import json
import os
import shutil
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def repo_root() -> Path:
    env = os.environ.get("RUNG_ROOT")
    if env:
        return Path(env).expanduser().resolve()
    return Path(__file__).resolve().parents[3]


def runs_root(root: Path | None = None) -> Path:
    return (root or repo_root()) / "rung-agent" / "harbor-runs"


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H%M%SZ")


def job_name(case_id: str, stamp: str | None = None) -> str:
    return f"{stamp or utc_stamp()}-{case_id}"


def index_path(root: Path | None = None) -> Path:
    return runs_root(root) / "index.jsonl"


def load_index(root: Path | None = None) -> list[dict[str, Any]]:
    path = index_path(root)
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(rec, dict):
            rows.append(rec)
    return rows


def append_index(record: dict[str, Any], root: Path | None = None) -> None:
    path = index_path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(record, sort_keys=True) + "\n")


def latest_by_case(index: list[dict[str, Any]] | None = None, root: Path | None = None) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for rec in index if index is not None else load_index(root):
        cid = rec.get("id")
        if isinstance(cid, str):
            out[cid] = rec
    return out


def latest_reward(latest: dict[str, dict[str, Any]], case_id: str) -> float | None:
    rec = latest.get(case_id)
    if not rec:
        return None
    reward = rec.get("reward")
    if isinstance(reward, bool) or not isinstance(reward, (int, float)):
        return None
    return float(reward)


def read_trial_result(trial_dir: Path) -> dict[str, Any] | None:
    path = trial_dir / "result.json"
    if not path.is_file():
        return None
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, dict) else None


def trial_reward(data: dict[str, Any]) -> float | None:
    rewards = (data.get("verifier_result") or {}).get("rewards") or {}
    reward = rewards.get("reward")
    if isinstance(reward, bool) or not isinstance(reward, (int, float)):
        return None
    return float(reward)


def find_trials(job_dir: Path) -> list[Path]:
    """Harbor writes job/result.json plus one subdirectory per trial."""
    found: list[Path] = []
    if not job_dir.is_dir():
        return found
    for child in sorted(job_dir.iterdir()):
        if child.is_dir() and (child / "result.json").is_file():
            found.append(child)
    return found


def newest_trial_for_task(jobs: Path, task_name: str) -> Path | None:
    """Scan a Harbor jobs tree for the newest trial of `task_name`."""
    best: tuple[float, Path] | None = None
    if not jobs.is_dir():
        return None
    for result in jobs.glob("*/*/result.json"):
        try:
            data = json.loads(result.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if data.get("task_name") != task_name:
            continue
        mtime = result.stat().st_mtime
        if best is None or mtime >= best[0]:
            best = (mtime, result.parent)
    return None if best is None else best[1]


COPY_REL = (
    "result.json",
    "trial.log",
    "config.json",
    "agent/rung-agent.txt",
    "verifier/reward.txt",
    "verifier/test-stdout.txt",
)


def copy_trial(src: Path, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    for rel in COPY_REL:
        file = src / rel
        if not file.is_file():
            continue
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(file, target)
    job_result = src.parent / "result.json"
    if job_result.is_file() and job_result != src / "result.json":
        shutil.copy2(job_result, dest / "job-result.json")


def record_from_trial(
    *,
    case_id: str,
    task_name: str,
    model: str,
    trial: Path,
    root: Path | None = None,
    stamp: str | None = None,
    harbor_exit: int | None = None,
) -> dict[str, Any]:
    """Copy a Harbor trial into `harbor-runs/<stamp>-<id>/` and index it."""
    stamp = stamp or utc_stamp()
    name = job_name(case_id, stamp)
    dest = runs_root(root) / name
    copy_trial(trial, dest)
    data = read_trial_result(trial) or {}
    reward = trial_reward(data)
    rec: dict[str, Any] = {
        "id": case_id,
        "task_name": task_name,
        "model": model,
        "reward": reward,
        "dir": str(dest),
        "harbor_trial": str(trial.resolve()),
        "stamp": stamp,
        "job_name": name,
        "harbor_exit": harbor_exit,
    }
    append_index(rec, root)
    return rec
