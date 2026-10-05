# Memory loop walkthrough: a Jev-Mem provider, one owner message

Informative. End to end: a `rung-memory/1` provider in a container, one
owner message per turn, the memory outcome in `_meta.rung.memory`, and the
`cached_tokens` the route reports. The commands are not verified by a live run; treat them as a sketch.
The contract is [rung-memory.md](rung-memory.md); the host is
[rung-host-quickstart.md](rung-host-quickstart.md).

**Which process.** `rung-agent` takes any provider (`--memory mcp:<url>`).
`rung-host` keeps its own `baseline` memory under its state dir
(`memory: true`) and has no `mcp:` setting, so the Jev-Mem provider is
reached through `rung-agent`; the host half is step 5.

## 1. Build and start the provider

Needs docker and a checkout of `witt3rd/rung-memory-jevmem` (the image is not
published; build it from that checkout, about 2 minutes):

```bash
cd rung-memory-jevmem
docker/run.sh build                    # builds rung-memory-jevmem:dev
mkdir -p /tmp/jevmem-data
docker run -d --name rung-walkthrough-jevmem --network host \
  --user "$(id -u):$(id -g)" -e HOME=/tmp -v /tmp/jevmem-data:/data \
  rung-memory-jevmem:dev serve --transport http --host 127.0.0.1 --port 9187 \
  --data /data --jev fake
curl -s localhost:9187/healthz         # {"ok":true,"marker":"rung-memory/1",...}
```

`--jev fake` answers Jev offline, so the provider costs nothing. (`docker/run.sh
serve --jev fake` does the same on port 9000; use `--network host` as above if
your docker cannot create bridge networks.) Check it against the contract:

```bash
cargo build -p rung-agent
target/debug/rung-agent --memory-check mcp:http://127.0.0.1:9187/mcp 2>&1 | grep -E '^(pass|fail)'
```

All seven clauses print `pass`.

## 2. One owner message, two turns

`scripts/acp_cache_probe.py` drives `rung-agent --acp` through a recording
proxy. `--seed` is an earlier session that gets retained; the two `--asks`
are turns in one new session. The key is read from the environment only:

```bash
doppler run -p fleet -c dev_work -- python3 scripts/acp_cache_probe.py \
  --model deepseek/deepseek-chat-v3.1 \
  --memory mcp:http://127.0.0.1:9187/mcp --scope walkthrough \
  --seed "Remember this: the payments service deploys from the release branch." \
  --asks "Which branch does the payments service deploy from?" \
         "Thanks. And which service did I mean?" \
  --out /tmp/walkthrough-probe
```

The model is a paid one (about USD 0.002 for this run): the workspace's data
policy refuses every `:free` model. Any key works; name it with `--key-env`.

## 3. Read `_meta.rung.memory`

Each turn prints the `_meta.rung.memory` of its prompt response:

```text
memory: {"provider":"mcp","recall":{"status":"found","records":1,"injected":["f28c92d4-…"],"calls":2,"cost_usd":3.7716e-05,"latency_ms":22},"retain":{"status":"stored","id":"e661bbb7-…","calls":2,"cost_usd":3.5616e-05,"latency_ms":26}}
memory: {"provider":"mcp","recall":{"status":"found","records":2,"injected":["e661bbb7-…","f28c92d4-…"],"calls":2,"cost_usd":4.0152e-05,"latency_ms":20},"retain":{"status":"stored","id":"9efcf5e9-…","calls":2,"cost_usd":4.9476e-05,"latency_ms":23}}
```

- Turn 1 recalls the seed (`records: 1`) and retains itself (`stored`).
- Turn 2 recalls two: the seed and turn 1 (`injected` lists the ids, in
  order). Memory carries across sessions because the scope is the same.
- `cost_usd` and `calls` are the provider's own (its Jev calls); `latency_ms`
  is rung's measure. The hooks are tabulated in [rung-memory.md](rung-memory.md#a-turn-with-a-provider).

## 4. Read `cached_tokens`

The probe then prints one line per model call, from the usage the route
reported:

```text
call 0: first    prompt=2660 cached=2432 cost=0.0007282 ...
call 1: extends  prompt=2892 cached=2560 cost=0.00078684 ...
```

`cached` is `prompt_tokens_details.cached_tokens`. Turn 2 (call 1) extends
turn 1's request and 2560 of its 2892 prompt tokens were served from cache,
with the recalled block in the context. Counts vary by route and run, and
some routes report none (see
[rung-agent-cache-live-memory.md](rung-agent-cache-live-memory.md)). Raw
requests and usage are in `/tmp/walkthrough-probe/`.

## 5. The same loop in the host

`rung-host` with `memory: true` (the mock quickstart config with
`memory: false` changed) recalls at each boundary. After the quickstart's
run, the record says what memory did:

```bash
jq -c 'select(.kind|startswith("memory"))' demo/state/record/*.ndjson
# {"cached":false,"cue":"stimulus","kind":"memory.recall","report":{"calls":1,"cost_usd":0.0,"latency_ms":0,"records":0,"status":"empty"},...}
```

With a live engine (`kind: agent`) each `llm.call` line also carries
`cached_tokens`:

```bash
jq -c 'select(.kind=="llm.call") | {turn, prompt_tokens, cached_tokens}' demo/state/record/*.ndjson
```

## 6. Clean up

```bash
docker stop rung-walkthrough-jevmem && docker rm rung-walkthrough-jevmem
rm -rf /tmp/jevmem-data /tmp/walkthrough-probe
```
