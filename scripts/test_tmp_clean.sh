#!/usr/bin/env bash
# Run the workspace tests with a private, empty temp root and fail if any
# test leaves something behind in it (rung issue 160). Tests make scratch
# directories through `rung_testkit::TempDir`, which removes them on drop.
# Extra args go to `cargo test`; default is `--workspace --locked`. A first
# argument `nextest` runs `cargo nextest run` with the remaining args instead.
set -uo pipefail
root=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/rung-tmp-root.XXXXXX")
trap 'rm -rf "$root"' EXIT
args=("$@")
[ ${#args[@]} -eq 0 ] && args=(--workspace --locked)
if [ "${args[0]}" = nextest ]; then
  cmd=(cargo nextest run "${args[@]:1}")
else
  cmd=(cargo test "${args[@]}")
fi
TMPDIR="$root" "${cmd[@]}" || exit $?
left=$(ls -A "$root")
if [ -n "$left" ]; then
  echo "tests left files in the temp root:" >&2
  echo "$left" | sed 's/^/  /' >&2
  exit 1
fi
echo "temp root clean"
