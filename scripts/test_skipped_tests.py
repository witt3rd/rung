#!/usr/bin/env python3
"""Tests for skipped_tests.py: ignored tests are found, with their reasons."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import skipped_tests as st


def scan(src):
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "t.rs"
        p.write_text(src)
        return st.scan(p)


class Scan(unittest.TestCase):
    def test_finds_an_ignored_test_and_its_reason(self):
        ig, names = scan('#[test]\n#[ignore = "needs a kernel"]\nfn a() {}\n#[test]\nfn b() {}\n')
        self.assertEqual(names, {"a", "b"})
        self.assertEqual([(n, r) for _, n, r in ig], [("a", "needs a kernel")])

    def test_a_reason_over_several_lines_is_joined(self):
        ig, _ = scan('/// doc\n#[test]\n#[ignore = "one \\\n   two"]\nfn a() {}\n')
        self.assertEqual(ig[0][2], "one two")

    def test_an_ignore_without_a_reason_says_so(self):
        ig, _ = scan("#[test]\n#[ignore]\nfn a() {}\n")
        self.assertEqual(ig[0][2], "(no reason given)")

    def test_a_plain_function_is_not_a_test(self):
        ig, names = scan("#[ignore]\nfn helper() {}\n")
        self.assertEqual((ig, names), ([], set()))


if __name__ == "__main__":
    unittest.main()
