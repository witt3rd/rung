#!/usr/bin/env bash
# Run the workspace tests with a private, empty temp root and fail if any
# test leaves something behind in it (rung issue 160). Tests make scratch
# directories through `rung_testkit::TempDir`, which removes them on drop.
# Extra args go to `cargo test`; default is `--workspace --locked`.
set -uo pipefail
root=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/rung-tmp-root.XXXXXX")
trap 'rm -rf "$root"' EXIT
args=("$@")
[ ${#args[@]} -eq 0 ] && args=(--workspace --locked)
log="$root.log"
trap 'rm -rf "$root" "$log"' EXIT
TMPDIR="$root" cargo test "${args[@]}" 2>&1 | tee "$log"
rc=${PIPESTATUS[0]}
[ "$rc" -ne 0 ] && exit "$rc"
# A skipped verifier must be visible (rung issue 270): list every ignored test
# with its reason and raise a CI warning annotation, never pass silently.
ignored=$(grep -E '^test .* \.\.\. ignored' "$log" || true)
if [ -n "$ignored" ]; then
  n=$(echo "$ignored" | wc -l)
  echo "skipped (ignored) tests: $n" >&2
  echo "$ignored" | sed 's/^/  /' >&2
  echo "::warning title=skipped tests::$n test(s) were ignored; see the test step log for names and reasons"
else
  echo "skipped tests: 0"
fi
# Expected gates (rung issue 270): tests that must have run and passed. A gate
# listed here that is absent, ignored or filtered out fails the job. Only checked
# on the default whole-workspace run (extra args may select a subset).
EXPECTED_GATES=(
  no_proof_names_an_ignored_test
  every_decidable_proposition_names_a_proof_that_resolves
  every_derived_number_matches_the_document
)
if [ $# -eq 0 ]; then
  missing=()
  for g in "${EXPECTED_GATES[@]}"; do
    grep -Eq "^test (.*::)?$g \.\.\. ok" "$log" || missing+=("$g")
  done
  if [ ${#missing[@]} -ne 0 ]; then
    echo "expected gates did not run:" >&2
    printf '  %s\n' "${missing[@]}" >&2
    echo "::error title=expected gate missing::${#missing[@]} expected gate(s) did not run"
    exit 1
  fi
  echo "expected gates ran: ${#EXPECTED_GATES[@]}"
fi
left=$(ls -A "$root")
if [ -n "$left" ]; then
  echo "tests left files in the temp root:" >&2
  echo "$left" | sed 's/^/  /' >&2
  exit 1
fi
echo "temp root clean"
