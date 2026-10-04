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
TMPDIR="$root" cargo test "${args[@]}" || exit $?
left=$(ls -A "$root")
if [ -n "$left" ]; then
  echo "tests left files in the temp root:" >&2
  echo "$left" | sed 's/^/  /' >&2
  exit 1
fi
echo "temp root clean"
