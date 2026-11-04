#!/usr/bin/env bash
# Tier 1: the fast gate. It runs LOCALLY under scripts/land-direct on the exact combination about to land
# (the change rebased on master), and in CI as the `tier1` job of ci.yml on every push to master.
# Its Tier 1 job result IS the definition of red (scripts/land-direct: main is red when the `tier1`
# job of master's latest push run failed). Target: a few minutes, with the cargo work selected by changed crate.
#   scripts/tier1.sh [--plan]      --plan only prints the selection (what would run) and exits
# What runs: formatting, the records guard, the always-cheap Python guards (the CI planner's own tests,
# skipped-test gates, proposition numbering and citations, consumer-name guard, links, coverage), then for
# the CHANGED crates plus their reverse dependencies (scripts/ci_plan.py): strict clippy and the unit tests
# (`--lib --bins`; one process-wide peak-RSS measurement is skipped here: it reads
# VmHWM, which parallel threads of `cargo test` disturb. Tier 2 runs it under nextest, one process per test). `render --check` runs when a crate it reads is in scope. The lander's own self-test
# runs when the lander or this script changed. Everything slow is Tier 2 (.github/workflows/tier2.yml:
# integration tests, doctests, rustdoc, trybuild UI suites, examples, MSRV, the GUI workspace, cargo-deny, release check).
# Environment: TIER1_BASE (default origin/master): the commit the change is measured from. An unreachable
# or all-zero TIER1_BASE (first push, forced history) selects the whole workspace. TIER1_JOBS caps cargo's jobs.
export LC_ALL=C
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
base=${TIER1_BASE:-origin/master}
t0=$(date +%s)
step() { printf 'tier1: [%3ss] %s\n' "$(( $(date +%s) - t0 ))" "$*"; }
if [[ "$base" =~ ^0+$ ]] || ! git rev-parse --verify -q "$base^{commit}" >/dev/null; then
  plan=$(scripts/ci_plan.py --event push)
  files=
else
  plan=$(scripts/ci_plan.py --event pull_request --base "$base" --head HEAD)
  files=$(git diff --name-only --no-renames "$base...HEAD")
fi
echo "$plan" | sed 's/^/tier1: plan: /'
if [ "${1:-}" = --plan ]; then exit 0; fi
get() { sed -n "s/^$1=//p" <<<"$plan"; }
scope=$(get scope); rust=$(get rust)
jobs=(); [ -z "${TIER1_JOBS:-}" ] || jobs=(-j "$TIER1_JOBS")
touched() { grep -qE "$1" <<<"$files"; }

step "fmt"
cargo fmt --check
# The cargo-free guards run beside the cargo work (they are the same seconds either way); a failure in
# either surfaces, the guards' output is shown after the cargo steps.
glog=$(mktemp); trap 'rm -f "$glog"' EXIT
(
  set -euo pipefail
  step "records are append-only"
  docs/_records_guard.py "$(git rev-parse --verify -q "$base^{commit}" >/dev/null && ! [[ "$base" =~ ^0+$ ]] && echo "$base" || echo origin/master)"
  step "planner, skipped-test and post-merge-review tests"
  python3 scripts/test_ci_plan.py
  python3 scripts/test_skipped_tests.py
  python3 scripts/test_post_merge_review.py
  scripts/skipped_tests.py
  step "propositions, consumer names, links, coverage"
  docs/_props.py check
  docs/_props.py cited
  docs/_consumer_guard.py
  python3 scripts/test_links.py
  docs/_links.py
  docs/_coverage.py
  if [ -z "$files" ] || touched '^scripts/(land-direct|tier1\.sh|test_land_direct\.sh|generated-paths\.txt)$'; then
    step "land-direct self-test"
    scripts/test_land_direct.sh
  fi
) >"$glog" 2>&1 &
gpid=$!

if [ "$rust" = true ]; then
  # SCOPE is a list of -p flags: split on purpose
  # shellcheck disable=SC2086
  { step "clippy ($scope)"; cargo clippy "${jobs[@]}" $scope --all-targets --locked -- -D warnings
    step "unit tests ($scope)"
    # One retry: a unit test that is flaky under cargo's shared-process threads (seen once:
    # rung-host registry idle test) must not stop every landing; a real failure fails twice. Tier 2 runs
    # each test in its own process (nextest).
    unit=(scripts/test_tmp_clean.sh "${jobs[@]}" $scope --lib --bins --locked -- --skip whole_50mb_pages_one_copy_in_memory)
    "${unit[@]}" || { step "unit tests failed: ONE retry (a repeat failure is real)"; "${unit[@]}"; }; }
  case " $scope " in
    *" -p rung-doctrine "*|*" --workspace "*)
      step "generated documents match their encoding"
      cargo run -q "${jobs[@]}" -p rung-doctrine --bin render --locked -- --check;;
  esac
else
  step "no Rust input changed: clippy and unit tests skipped"
fi

step "guards (ran beside the cargo steps)"
gok=0; wait "$gpid" || gok=$?
cat "$glog"
[ "$gok" = 0 ] || { step "a guard failed"; exit "$gok"; }
step "ok in $(( $(date +%s) - t0 )) s"
