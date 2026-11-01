import unittest

import perf_gate as p


def res(**kw):
    return {m: {"median": kw.get(m)} for m in p.METRICS}


class Gate(unittest.TestCase):
    def test_noisy_is_withheld(self):
        self.assertIsNone(p.summarize([10, 20, 5, 30, 8]))
        self.assertEqual(p.summarize([10, 10.5, 9.8, 10.1, 10.2]), 10.1)

    def test_threshold(self):
        base = res(cold_start_ms=100, rss_mib_10mib=100, switch_ms=100)
        ok = res(cold_start_ms=110, rss_mib_10mib=90, switch_ms=100)
        bad = res(cold_start_ms=111, rss_mib_10mib=90, switch_ms=100)
        self.assertEqual(p.verdict(p.compare(base, ok)), 0)
        self.assertEqual(p.verdict(p.compare(base, bad)), 1)

    def test_withheld_is_not_a_pass(self):
        base = res(cold_start_ms=100, rss_mib_10mib=100, switch_ms=100)
        cur = res(cold_start_ms=None, rss_mib_10mib=100, switch_ms=100)
        self.assertEqual(p.verdict(p.compare(base, cur)), 2)


if __name__ == "__main__":
    unittest.main()
