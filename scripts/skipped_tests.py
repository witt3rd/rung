#!/usr/bin/env python3
"""Make skipped tests visible (rung issue 270).

Lists every `#[ignore]`d test with its reason, raises a CI warning annotation
with the count, and fails when an expected gate is missing from the source or
is itself ignored. It reads the source, not a test run, so it is the same
whatever shards or runner execute the tests.

    scripts/skipped_tests.py [ROOT]
"""
import re
import subprocess
import sys
from pathlib import Path

# Tests that must exist and must not be ignored: the gates that police the
# rest. A gate listed here that is renamed, removed or ignored fails the job.
EXPECTED_GATES = {
    "no_proof_names_an_ignored_test": "rung-doctrine/tests/roundtrip.rs",
    "every_decidable_proposition_names_a_proof_that_resolves": "rung-doctrine/tests/roundtrip.rs",
    "every_derived_number_matches_the_document": "rung-doctrine/tests/roundtrip.rs",
}
ATTR = re.compile(r"^\s*#\[(?:\w+::)?test\b")
FN = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")


def attribute(lines, i):
    """The full text of the attribute starting at lines[i], and its last index."""
    text, depth = "", 0
    while i < len(lines):
        text += lines[i].strip() + " "
        depth += lines[i].count("[") - lines[i].count("]")
        if depth <= 0:
            break
        i += 1
    return text.strip(), i


def scan(path):
    """(ignored: [(line, test, reason)], all test names) in one source file."""
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    ignored, names, pending = [], set(), []
    i = 0
    while i < len(lines):
        t = lines[i].strip()
        if t.startswith("#["):
            a, i = attribute(lines, i)
            pending.append(a)
        elif t.startswith("//") or not t:
            pass
        else:
            m = FN.match(lines[i])
            if m and any(ATTR.match(a) for a in pending):
                names.add(m.group(1))
                for a in pending:
                    if a.startswith("#[ignore"):
                        r = re.search(r'=\s*"(.*)"\s*\]$', a)
                        reason = re.sub(r"\\\s+", "", r.group(1)) if r else "(no reason given)"
                        ignored.append((i + 1, m.group(1), reason))
            pending = []
        i += 1
    return ignored, names


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
    files = subprocess.run(
        ["git", "-C", str(root), "ls-files", "*.rs"], capture_output=True, text=True, check=True
    ).stdout.split()
    ignored, found = [], {}
    for f in files:
        ig, names = scan(root / f)
        ignored += [(f, *x) for x in ig]
        for n in names:
            found.setdefault(n, []).append(f)
    if ignored:
        print(f"skipped (ignored) tests: {len(ignored)}")
        for f, line, name, reason in ignored:
            short = reason if len(reason) <= 160 else reason[:157] + "..."
            print(f"  {f}:{line} {name}: {short}")
        print(f"::warning title=skipped tests::{len(ignored)} test(s) are #[ignore]d; see this step's log for names and reasons")
    else:
        print("skipped tests: 0")
    bad = []
    skipped = {(f, n) for f, _, n, _ in ignored}
    for gate, where in EXPECTED_GATES.items():
        if where not in found.get(gate, []):
            bad.append(f"{gate}: not found in {where}")
        elif (where, gate) in skipped:
            bad.append(f"{gate}: is ignored")
    if bad:
        for b in bad:
            print(f"expected gate missing: {b}", file=sys.stderr)
        print(f"::error title=expected gate missing::{len(bad)} expected gate(s) missing or ignored")
        return 1
    print(f"expected gates present and not ignored: {len(EXPECTED_GATES)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
