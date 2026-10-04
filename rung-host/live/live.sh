#!/usr/bin/env bash
# One window of the first live run: rung-host run --config, the owner's
# message driver beside it, and a syscall trace of the host's file writes.
#
#   rung-host/live/live.sh RUN_DIR RUN_FOR_S
#
# RUN_DIR is the run's own scratch directory (state, sandbox, inbox, traces).
# The router key comes from Doppler into this process's environment only
# (--only-secrets, --no-fallback: no secret file is written); it is never on a
# command line, in a file or in a log. The window ends at the host's run limit
# (run_for_s), on RUN_DIR/STOP, or on SIGTERM; `timeout` is the hard stop.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
run="$(mkdir -p "$1" && cd "$1" && pwd)"
for_s="$2"
bin="$repo/target/release/rung-host"
[ -x "$bin" ] || { echo "build first: cargo build --release -p rung-host" >&2; exit 2; }
mkdir -p "$run/trace" "$run/state"
[ -e "$run/started_at" ] || date +%s.%N > "$run/started_at"
sed -e "s|@RUN@|$run|g" -e "s|@RUN_FOR_S@|$for_s|g" "$here/rung-host.yaml.in" > "$run/rung-host.yaml"
shopt -s nullglob
traces=("$run"/trace/*.strace)
n=$(( ${#traces[@]} + 1 ))
python3 "$here/drive.py" "$run" "$here/stimuli.json" >> "$run/drive.log" 2>&1 &
drive=$!
echo "window $n: for ${for_s}s from $(date -u +%FT%TZ)" | tee -a "$run/windows.log"
set +e
timeout --signal=TERM --kill-after=30 "$(( for_s + 600 ))" \
  doppler run -p fleet -c dev_work --no-fallback --only-secrets OPENROUTER_API_KEY -- \
  strace -f -qq -o "$run/trace/host.$n.strace" \
    -e trace=open,openat,openat2,creat,mkdir,mkdirat,rename,renameat,renameat2,unlink,unlinkat,rmdir,link,linkat,symlink,symlinkat,truncate,chmod,fchmodat \
    "$bin" run --config "$run/rung-host.yaml" > "$run/host.$n.log" 2>&1
code=$?
set -e
echo "$code" > "$run/exit.$n"
kill "$drive" 2>/dev/null || true
echo "window $n: exit $code at $(date -u +%FT%TZ)" | tee -a "$run/windows.log"
exit "$code"
