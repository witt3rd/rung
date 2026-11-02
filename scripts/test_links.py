"""Self-test for docs/_links.py. Run: python3 scripts/test_links.py

Builds a throwaway git repo with one good and several broken documents and
checks that the guard reports each defect and nothing else."""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "docs"))
import _links as L  # noqa: E402


def repo(files):
    d = Path(tempfile.mkdtemp())
    subprocess.check_call(["git", "init", "-q"], cwd=d)
    for name, body in files.items():
        p = d / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body)
    subprocess.check_call(["git", "add", "-A"], cwd=d)
    return d


class Links(unittest.TestCase):
    def run_check(self, files):
        d = repo(files)
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        return L.check(d)

    def test_clean_repo_has_no_problems(self):
        self.assertEqual(self.run_check({
            "README.md": "# Top\n[a](docs/a.md#sec-one) [self](#top) [web](https://x.y/z)\n"
                         "See `docs/a.md` and `src/lib.rs`.\n",
            "docs/a.md": "# A\n## Sec one\n<a id=\"named\"></a>\n[up](../README.md#top) [n](a.md#named)\n",
            "src/lib.rs": "// see docs/a.md\n",
        }), [])

    def test_broken_link_and_anchor(self):
        p = self.run_check({
            "README.md": "[a](docs/missing.md)\n[b](docs/a.md#nope)\n[c](#nowhere)\n",
            "docs/a.md": "# A\n",
        })
        self.assertEqual(len(p), 3, p)
        self.assertIn("broken link: docs/missing.md", p[0])
        self.assertIn("broken anchor", p[1])
        self.assertIn("broken anchor: #nowhere", p[2])

    def test_backtick_path(self):
        p = self.run_check({"README.md": "`src/gone.rs` and `src/here.rs` and `a/b` and `x/y.rs`\n",
                            "src/here.rs": ""})
        self.assertEqual(len(p), 1, p)
        self.assertIn("src/gone.rs", p[0])

    def test_citation_in_source(self):
        p = self.run_check({"src/a.rs": "// docs/gone.md\n", "tool.py": "'docs/ok.md'\n", "docs/ok.md": "# ok\n"})
        self.assertEqual(len(p), 1, p)
        self.assertIn("docs/gone.md", p[0])

    def test_hand_counts_forbidden_outside_records(self):
        p = self.run_check({
            "docs/a.md": "# A\n`x.rs` (12 tests)\n(3 tests, H3b)\n",
            "docs/records/r.md": "# R\n(12 tests)\n",
        })
        self.assertEqual(len(p), 2, p)

    def test_code_fences_and_inline_code_are_not_links(self):
        self.assertEqual(self.run_check({
            "a.md": "# A\n```\n[x](nope.md) `src/none.rs`\n```\nuse `[t](url)` here\n",
        }), [])

    def test_skipped_trees(self):
        self.assertEqual(self.run_check({
            "rung-host/live/runs/r/a.md": "[x](nope.md)\n",
            "ui/node_modules/p/README.md": "[x](nope.md)\n",
        }), [])

    def test_a_workspace_member_needs_an_agents_file(self):
        probs = self.run_check({
            "Cargo.toml": '[workspace]\nmembers = ["a", "b"]\n',
            "a/Cargo.toml": "", "a/AGENTS.md": "# a\n", "b/Cargo.toml": "",
        })
        self.assertEqual(probs, ["b/Cargo.toml:1: workspace member has no AGENTS.md (b/AGENTS.md)"])

    def test_real_repo_is_clean(self):
        root = Path(__file__).resolve().parent.parent
        self.assertEqual(L.unallowed(root, L.check(root)), [])


if __name__ == "__main__":
    unittest.main()
