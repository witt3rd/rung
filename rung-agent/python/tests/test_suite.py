import unittest

from rung_harbor.suite import CASES, case_by_id, live_cases
from rung_harbor.validate import latest_reward, next_unproven, trial_rewards


class SuiteTests(unittest.TestCase):
    def test_ids_unique(self):
        ids = [c.id for c in CASES]
        self.assertEqual(len(ids), len(set(ids)))

    def test_live_excludes_mcp(self):
        live = {c.id for c in live_cases()}
        self.assertIn("write-file", live)
        self.assertNotIn("mcp", live)
        self.assertIsNotNone(case_by_id("mcp").skip)

    def test_unknown_id(self):
        with self.assertRaises(KeyError):
            case_by_id("nope")

    def test_next_skips_proven(self):
        rewards = {
            "harbor/hello-world": [1.0],
            "harbor/hello-user": [1.0],
        }
        nxt = next_unproven(rewards)
        self.assertIsNotNone(nxt)
        self.assertEqual(nxt.id, "cwd-capture")

    def test_latest_reward_uses_last(self):
        self.assertEqual(
            latest_reward({"harbor/hello-world": [0.0, 1.0]}, "harbor/hello-world"),
            1.0,
        )

    def test_empty_jobs_dir(self):
        self.assertEqual(trial_rewards(__import__("pathlib").Path("/no/such/jobs")), {})


if __name__ == "__main__":
    unittest.main()
