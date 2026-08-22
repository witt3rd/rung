import json
import os
import tempfile
import unittest
from pathlib import Path

from rung_harbor.evidence import (
    copy_trial,
    latest_by_case,
    latest_reward,
    load_index,
    record_from_trial,
    runs_root,
)
from rung_harbor.suite import CASES, case_by_id, live_cases
from rung_harbor.validate import next_unproven


class SuiteTests(unittest.TestCase):
    def test_ids_unique(self):
        ids = [c.id for c in CASES]
        self.assertEqual(len(ids), len(set(ids)))

    def test_live_includes_mcp(self):
        live = {c.id for c in live_cases()}
        self.assertIn("write-file", live)
        self.assertIn("mcp", live)
        self.assertIsNone(case_by_id("mcp").skip)
        self.assertNotIn("alpine-write", live)
        self.assertIsNotNone(case_by_id("alpine-write").skip)

    def test_unknown_id(self):
        with self.assertRaises(KeyError):
            case_by_id("nope")


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)
        os.environ["RUNG_ROOT"] = str(self.root)

    def tearDown(self):
        os.environ.pop("RUNG_ROOT", None)

    def _trial(self, task_name: str, reward: float) -> Path:
        n = getattr(self, "_n_trials", 0) + 1
        self._n_trials = n
        trial = self.root / "harbor-job" / f"trial-{n}"
        trial.mkdir(parents=True)
        (trial / "agent").mkdir()
        (trial / "verifier").mkdir()
        (trial / "result.json").write_text(
            json.dumps(
                {
                    "task_name": task_name,
                    "verifier_result": {"rewards": {"reward": reward}},
                }
            )
        )
        (trial / "trial.log").write_text("ok\n")
        (trial / "agent" / "rung-agent.txt").write_text("did the thing\n")
        (trial / "verifier" / "reward.txt").write_text("1\n" if reward >= 1 else "0\n")
        return trial

    def test_record_and_latest(self):
        trial = self._trial("harbor/hello-world", 1.0)
        rec = record_from_trial(
            case_id="write-file",
            task_name="harbor/hello-world",
            model="openrouter/~deepseek/deepseek-v4-flash-latest",
            trial=trial,
            stamp="20260821T000000Z",
            root=self.root,
        )
        dest = Path(rec["dir"])
        self.assertTrue((dest / "agent" / "rung-agent.txt").is_file())
        self.assertEqual(rec["reward"], 1.0)
        latest = latest_by_case(root=self.root)
        self.assertEqual(latest_reward(latest, "write-file"), 1.0)
        self.assertEqual(len(load_index(self.root)), 1)

    def test_redo_appends(self):
        t1 = self._trial("harbor/hello-world", 0.0)
        record_from_trial(
            case_id="write-file",
            task_name="harbor/hello-world",
            model="m",
            trial=t1,
            stamp="20260821T000000Z",
            root=self.root,
        )
        t2 = self._trial("harbor/hello-world", 1.0)
        record_from_trial(
            case_id="write-file",
            task_name="harbor/hello-world",
            model="m",
            trial=t2,
            stamp="20260821T000100Z",
            root=self.root,
        )
        latest = latest_by_case(root=self.root)
        self.assertEqual(latest_reward(latest, "write-file"), 1.0)
        self.assertEqual(len(load_index(self.root)), 2)
        self.assertEqual(len(list(runs_root(self.root).glob("*-write-file"))), 2)

    def test_next_skips_proven(self):
        t = self._trial("harbor/hello-world", 1.0)
        record_from_trial(
            case_id="write-file",
            task_name="harbor/hello-world",
            model="m",
            trial=t,
            stamp="a",
            root=self.root,
        )
        t2 = self._trial("harbor/hello-user", 1.0)
        record_from_trial(
            case_id="shell-user",
            task_name="harbor/hello-user",
            model="m",
            trial=t2,
            stamp="b",
            root=self.root,
        )
        nxt = next_unproven(latest_by_case(root=self.root))
        self.assertIsNotNone(nxt)
        self.assertEqual(nxt.id, "cwd-capture")

    def test_copy_skips_missing(self):
        src = self.root / "empty-trial"
        src.mkdir()
        dest = self.root / "out"
        copy_trial(src, dest)
        self.assertTrue(dest.is_dir())
        self.assertEqual(list(dest.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
