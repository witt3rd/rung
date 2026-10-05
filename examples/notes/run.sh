#!/usr/bin/env bash
# Note-taking assistant: retain in one session, recall in the next.
# Mock (default): no key, no network. Live: LIVE=1, with RUNG_BASE_URL,
# RUNG_MODEL and RUNG_API_KEY (or api_key_env config) set for a real model.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
bin=${RUNG_AGENT:-$here/../../target/debug/rung-agent}
work=$(mktemp -d); trap 'kill ${mock:-0} 2>/dev/null; rm -rf "$work"' EXIT
cd "$work"
export RUNG_HOME=$work HOME=$work XDG_CONFIG_HOME=$work RUNG_CONFIG=$work/none.yaml
export RUNG_MEMORY=baseline
if [ -z "${LIVE:-}" ]; then
  "${RUNG_MOCK_LLM:-$here/../../target/debug/mock-llm}" port "Noted." "release/x" & mock=$!
  while [ ! -s port ]; do sleep 0.1; done
  export RUNG_BASE_URL=http://127.0.0.1:$(cat port) RUNG_MODEL=m RUNG_API_KEY=k RUNG_PROTOCOL=openai
fi
"$bin" --tools none "Remember this: the deploy branch is release/x"
second=$("$bin" --tools none "Which deploy branch do we use?")
echo "$second"
if [ -z "${LIVE:-}" ]; then
  # the second request carried the note from the first session
  sed -n 2p port.requests | grep -q "Recalled memory"
  sed -n 2p port.requests | grep -q "deploy branch is release/x"
fi
echo "ok: notes"
