#!/usr/bin/env bash
# Ask the router once per ladder model whether it will route a tiny request
# for this account (a free model costs nothing; a refusal names its reasons).
#   doppler run -p fleet -c dev_work --no-fallback --only-secrets OPENROUTER_API_KEY -- rung-host/live/route-probe.sh
# Prints model, HTTP status and the router's reasons; never the key. The
# workspace name in a guardrail URL is masked.
set -u
models=("$@")
[ ${#models[@]} -gt 0 ] || models=(stealth/space-bunny-alpha nvidia/nemotron-3-ultra-550b-a55b:free
  qwen/qwen3.8-27b:free google/gemma-4-31b-it:free nvidia/nemotron-3-super-120b-a12b:free openrouter/free)
for m in "${models[@]}"; do
  body=$(printf '{"model":"%s","messages":[{"role":"user","content":"Say OK."}],"max_tokens":32}' "$m")
  out=$(curl -s -m 90 -w '\n%{http_code}' -H "Authorization: Bearer $OPENROUTER_API_KEY" -H 'content-type: application/json' -d "$body" https://openrouter.ai/api/v1/chat/completions)
  code=$(echo "$out" | tail -1)
  echo "$m $code $(echo "$out" | head -n -1 | python3 -c 'import json,sys,re
try:
  d=json.load(sys.stdin)
  if "error" in d: print("ERR", d["error"].get("message","")[:160].replace("\n"," "), re.sub(r"workspaces/[^/\"]+/", "workspaces/<workspace>/", json.dumps(d["error"].get("metadata",{}).get("ineligibility_reasons",""))))
  else: print("OK served", d.get("model"), d.get("provider"), (d["choices"][0]["message"].get("content") or "")[:40].replace("\n"," "))
except Exception as e: print("unparsed", e)')"
done
