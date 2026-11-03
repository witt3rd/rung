# Quickstart: rung-agent with memory

Informative. The contract is [rung-memory.md](../rung-memory.md). The baseline
commands and `--memory-check` against the in-tree `--memory-fixture` were
run against rung-agent built from this tree. The turn examples need an
OpenAI-compatible endpoint named in `config.yaml`; full-turn commands were not
re-run without a key. No live provider was measured; see section 5.

Memory is off by default. One setting turns it on; the first surface set wins:
`--memory X`, then `RUNG_MEMORY=X`, then `memory: { provider: X }` in
`config.yaml`.

## 1. Baseline: two lines

No model, no server, no network. BM25 over a JSON-lines file.

```bash
rung-agent --memory baseline --json --tools none "My deploy day is Thursday. Reply ok."
rung-agent --memory baseline --json --tools none "Which day do I deploy?"
```

The second turn is shown the first (see section 3). Or set it once:

```bash
export RUNG_MEMORY=baseline
```

## 2. An MCP provider

Any provider that speaks the contract works: `mcp:URL` for HTTP, or
`mcp:COMMAND` to start one on stdio. rung ships a reference provider,
`--memory-fixture`. Check it, then use it:

```bash
rung-agent --memory-check "mcp:rung-agent --memory-fixture --file /tmp/mem.jsonl"
# 7 lines, all "pass"
rung-agent --memory "mcp:rung-agent --memory-fixture --file /tmp/mem.jsonl" --json --tools none "My deploy day is Thursday. Reply ok."
```

Or in `config.yaml`: `memory: { provider: "mcp:rung-agent --memory-fixture --file /tmp/mem.jsonl" }`.
A stdio provider is started once per prompt, so `--file` is what lets records
outlive it. For a long-running provider prefer HTTP:
`--memory mcp:http://HOST:PORT/mcp`.

### A containerized provider

Any `rung-memory/1` provider shipped as a container works the same way. The
command below is illustrative; the provider's own README is the authority for
the image name, flags, and its startup log:

```bash
docker run -d --name memprov -p 127.0.0.1:9000:9000 -v memprov-data:/data \
  <provider-image> serve
```

A provider typically exposes a health endpoint; check its README. Then point
rung at it (`memory: { provider: "mcp:http://127.0.0.1:9000/mcp" }` in
`config.yaml` works too):

```bash
rung-agent --memory-check mcp:http://127.0.0.1:9000/mcp   # 7 clauses pass when conformant
rung-agent --memory mcp:http://127.0.0.1:9000/mcp --json --tools none "My deploy day is Thursday. Reply ok."
```

Stop with `docker stop memprov`; the volume keeps scopes.

## 3. See what was recalled

With `--json` the result carries a `memory` object (on ACP it is
`_meta.rung.memory` on the prompt response). The second turn above gives:

```json
{"provider": "baseline",
 "recall": {"status": "found", "records": 1, "injected": ["f340dced1f635109"],
            "calls": 2, "cost_usd": 0.0, "latency_ms": 0},
 "retain": {"status": "stored", "id": "8eca7eb0061f4c7b", "calls": 1,
            "cost_usd": 0.0, "latency_ms": 0}}
```

`injected` lists the ids of the records actually put in front of the model,
in order; it is absent unless `status` is `found`. `empty` means nothing fit;
`unavailable` carries a `reason` and the turn still ran. The ids are the
`id` of a stored record (the `retain.id` of the turn that wrote it). The
recalled text itself goes only to the model, as a `## Recalled memory` block,
and is not written to the session file.

## 4. Inspect and reset a scope

A scope is the unit of memory. By default it is `rung-scope:<hex>`, a hash of
the git `origin` (else the repo root); set `RUNG_MEMORY_SCOPE=name` to choose
one.

- **baseline**: one file per scope, `<repo root>/.rung/memory/<id>.jsonl`
  (`$RUNG_HOME/memory/` when a scope is set; `RUNG_MEMORY_DIR` overrides).
  Inspect: `cat .rung/memory/*.jsonl` (each line has `id`, `scope`, `text`,
  `observed_at`, and for a turn record `attrs`). Reset: delete the file.
- **fixture** (`--file PATH`): one JSON record per line. Inspect: `cat PATH`.
  Reset: delete the file.
- **other providers**: use the provider's own tools and documentation. Reset
  one scope by switching to a new `RUNG_MEMORY_SCOPE`.

## 5. Cost and latency

Every hook reports `calls`, `cost_usd` and `latency_ms` in the `memory` object
(section 3), so measure your own provider there.

| setting | per-turn memory cost | memory latency |
|---|---|---|
| `baseline` | `$0` | under 1 ms (`latency_ms: 0`) |
| `--memory-fixture` | `$0.0001` per hook call | local process, no model |
| other `mcp:` provider | what it declares as `max_cost_usd` at most | what it takes, up to `timeout_secs` |

A recall costing more than the provider's declared `max_cost_usd` is
`unavailable`, not evidence. A hook over `timeout_secs` (default 30) is
`unavailable` and the turn proceeds without memory. Only a turn that
completed, and passed the turn check if it is on, is retained.

## Worked example: a provider in a container, one owner message

A sketch, not verified by a live run: a `rung-memory/1` provider in a container, one owner
message per turn, the memory outcome in `_meta.rung.memory`, and the `cached_tokens` the route
reports. It needs a checkout of `witt3rd/rung-memory-jevmem`, which is not in this repository, so
its flags cannot be checked here. The host quickstart is
[rung-host-quickstart.md](rung-host-quickstart.md).

**Which process.** `rung-agent` takes any provider (`--memory mcp:<url>`).
`rung-host` keeps its own `baseline` memory under its state dir
(`memory: true`) and has no `mcp:` setting, so the Jev-Mem provider is
reached through `rung-agent`; the host half is step 5.

### Build and start the provider

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

### One owner message, two turns

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

### Read `_meta.rung.memory`

Each turn prints the `_meta.rung.memory` of its prompt response:

```text
memory: {"provider":"mcp","recall":{"status":"found","records":1,"injected":["f28c92d4-…"],"calls":2,"cost_usd":3.7716e-05,"latency_ms":22},"retain":{"status":"stored","id":"e661bbb7-…","calls":2,"cost_usd":3.5616e-05,"latency_ms":26}}
memory: {"provider":"mcp","recall":{"status":"found","records":2,"injected":["e661bbb7-…","f28c92d4-…"],"calls":2,"cost_usd":4.0152e-05,"latency_ms":20},"retain":{"status":"stored","id":"9efcf5e9-…","calls":2,"cost_usd":4.9476e-05,"latency_ms":23}}
```

- Turn 1 recalls the seed (`records: 1`) and retains itself (`stored`).
- Turn 2 recalls two: the seed and turn 1 (`injected` lists the ids, in
  order). Memory carries across sessions because the scope is the same.
- `cost_usd` and `calls` are the provider's own (its Jev calls); `latency_ms`
  is rung's measure. The hooks are tabulated in [rung-memory.md](../rung-memory.md#a-turn-with-a-provider).

### Read `cached_tokens`

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
[rung-agent-cache-live-memory.md](../records/rung-agent/rung-agent-cache-live-memory.md)). Raw
requests and usage are in `/tmp/walkthrough-probe/`.

### The same loop in the host

`rung-host` with `memory: true` (the mock quickstart config with
`memory: false` changed) recalls at each boundary. After the host quickstart's
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

### Clean up

```bash
docker stop rung-walkthrough-jevmem && docker rm rung-walkthrough-jevmem
rm -rf /tmp/jevmem-data /tmp/walkthrough-probe
```
