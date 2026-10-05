import json
import os
import tempfile
import unittest

import shadow_audit as s


def rec(fam, agree, seq):
    return {"seq": seq, "kind": f"decision.{fam}", "turn": seq, "agree": agree,
            "choice": "a", "jev_choice": "a" if agree else "b", "input_hash": "h%d" % seq}


class T(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.p = os.path.join(self.d, "r.ndjson")
        rows = [rec("tools", i % 4 != 0, i) for i in range(40)]
        rows += [rec("model", True, 100 + i) for i in range(5)]
        rows.append({"seq": 999, "kind": "decision.tools", "choice": "a"})  # no shadow
        rows.append({"seq": 998, "kind": "decision.tools", "agree": False,
                     "choice": "a", "jev_choice": None})  # Jev answer unusable
        rows.append({"seq": 1000, "kind": "turn.ended"})
        with open(self.p, "w") as f:
            f.write("\n".join(json.dumps(r) for r in rows) + "\n{torn")

    def test_tally(self):
        t = s.tally(s.load(self.p))
        self.assertEqual(t, {"model": [5, 0], "tools": [30, 10]})
        self.assertIn("tools", s.table(t))

    def test_sheet(self):
        rows = s.load(self.p)
        out = os.path.join(self.d, "a.md")
        s.main([self.p, "-n", "4", "--seed", "1", "-o", out])
        md = open(out).read()
        self.assertEqual(md.count("\n## "), 4)
        self.assertIn("4 of 10 disagreements", md)
        self.assertNotIn("model", md.split("\n", 3)[3])
        self.assertEqual(s.sheet(rows, 4, 1), s.sheet(rows, 4, 1))
        self.assertIn("10 of 10", s.sheet(rows, 50, 1))


if __name__ == "__main__":
    unittest.main()
