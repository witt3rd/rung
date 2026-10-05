#!/usr/bin/env bash
# Reminder agent: the host's calendar fires an owner entry into the agent.
# Runs on the host's mock engine: no key, no network. Live: replace
# `engine: {kind: mock}` in reminders.yaml with `kind: agent`, `base_url`
# and `api_key_env` (see docs/rung-host-quickstart.md).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
bin=${RUNG_HOST:-$here/../../target/debug/rung-host}
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
cd "$work"
mkdir -p demo/inbox demo/sandbox
cp "$here/reminders.yaml" .
"$bin" run --config reminders.yaml --turns 3 >/dev/null 2>&1
rec=$(cat demo/state/record/*.ndjson)
grep -q '"kind":"calendar.fired"' <<<"$rec"
grep -q 'Stand-up' <<<"$rec"
# the fired entry reached the agent and a turn answered it
grep -q '"disposition":"answered","id":"cal-standup' <<<"$rec"
echo "ok: reminders"
