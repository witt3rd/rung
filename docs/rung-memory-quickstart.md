# Quickstart: rung-agent with memory

Informative. The contract is [rung-memory.md](rung-memory.md). The baseline
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
  `observed_at`). Reset: delete the file.
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
`unavailable`, not evidence. A hook over `timeout_secs` (default 10) is
`unavailable` and the turn proceeds without memory. Only a turn that
completed, and passed the turn check if it is on, is retained.
