import json, os, subprocess, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(__file__))
import cache_probe as cp


class T(unittest.TestCase):
    def test_first_diff(self):
        self.assertEqual(cp.first_diff(b"abcd", b"abXd"), 2)
        self.assertEqual(cp.first_diff(b"abc", b"abcde"), 3)
        self.assertIsNone(cp.first_diff(b"abc", b"abc"))

    def test_probe_append_vs_prefix_change(self):
        base = {"system": "S", "messages": [{"role": "user", "content": "hi"}]}
        grow = {"system": "S", "messages": base["messages"] + [{"role": "assistant", "content": "yo"}]}
        bad = {"system": "S2", "messages": grow["messages"]}
        usage = [{"turn": 1, "call": 1, "prompt_tokens": 10, "cached_tokens": 0},
                 {"turn": 2, "call": 1, "prompt_tokens": 20, "cached_tokens": 10},
                 {"turn": 3, "call": 1, "prompt_tokens": 20, "cached_tokens": 0}]
        rows = cp.probe([base, grow, bad], usage)
        self.assertTrue(rows[0]["first"])
        self.assertGreater(rows[1]["diff"], len(b'{"messages":[{"content":"hi","role":"user"}') - 2)
        self.assertEqual(rows[2]["diff"], cp.canon(bad).index(b"S2") + 1)
        self.assertIn("10/20", cp.render(rows))

    def test_cli_files_and_wrapped_usage(self):
        with tempfile.TemporaryDirectory() as d:
            r, u = os.path.join(d, "r.jsonl"), os.path.join(d, "u.jsonl")
            open(r, "w").write('{"request":{"a":1}}\n{"request":{"a":2}}\n')
            open(u, "w").write(json.dumps({"kind": "llm.call", "body": {"turn": 1, "call": 1, "prompt_tokens": 5, "cached_tokens": 0}}) + "\n"
                               + json.dumps({"turn": 1, "call": 2, "prompt_tokens": 6, "cached_tokens": 5}) + "\n")
            out = subprocess.check_output([sys.executable, cp.__file__, r, u], text=True)
            self.assertIn("5/6", out)
            self.assertEqual(out.splitlines()[2].split()[4], "5")


if __name__ == "__main__":
    unittest.main()
