# Quickstart: rung-agent with memory

Informative. The contract is [rung-memory.md](rung-memory.md). The baseline
and `--memory-fixture` commands below were run as written against rung-agent
built from this tree. The turn examples need an OpenAI-compatible endpoint
named in `config.yaml`. No live provider was measured on this host; see section 5.

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

### Jev-Mem container (one command)

Jev-Mem is a rung memory provider shipped as a container from
`witt3rd/rung-memory-jevmem`. Build the image once with `docker/run.sh` in
that repository, then start it:

```bash
docker run -d --name jevmem -p 127.0.0.1:9000:9000 -v jevmem-data:/data \
  rung-memory-jevmem:dev serve --jev fake
```

The log prints `rung-memory-jevmem: ready; scope store /data; Jev fake (read
control on); cap none (no spend)` and the rung config line
`--memory mcp:http://127.0.0.1:9000/mcp` (or
`memory: { provider: "mcp:http://127.0.0.1:9000/mcp" }`). `/healthz` answers
`{"ok":true,"marker":"rung-memory/1","backend":"fake","store":"/data","cap_usd":null,"spent_usd":null}`.
Then:

```bash
rung-agent --memory-check mcp:http://127.0.0.1:9000/mcp   # 7 clauses pass
rung-agent --memory mcp:http://127.0.0.1:9000/mcp --json --tools none "My deploy day is Thursday. Reply ok."
```

Live: add `-e OPENROUTER_API_KEY` and `serve --jev live`, under
`doppler run -p fleet -c dev_work --`. Stop with `docker stop jevmem`; the
volume keeps scopes. After stopping, `docker rm jevmem` and
`docker volume rm jevmem-data` reset everything.

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
- **Jev-Mem container**: scopes live in the `jevmem-data` volume; reset all
  with `docker volume rm jevmem-data` after stopping.
- **other providers**: use the provider's own tools and documentation. Reset
  one scope by switching to a new `RUNG_MEMORY_SCOPE`.

## 5. Cost and latency

Every hook reports `calls`, `cost_usd` and `latency_ms` in the `memory` object
(section 3), so measure your own provider there.

| setting | per-turn memory cost | memory latency |
|---|---|---|
| `baseline` | `$0` | under 1 ms (`latency_ms: 0`) |
| `--memory-fixture` | `$0.0001` per hook call | local process, no model |
| Jev-Mem, live | about $0.00005 per ask | Jev about 0.2 s an ask; unloaded recall p95 2.83 s (W4) |
| other `mcp:` provider | what it declares as `max_cost_usd` at most | what it takes, up to `timeout_secs` |

The Jev-Mem figures are cited from the provider README (live OpenRouter);
none were measured on this host, which has no doppler token.

A recall costing more than the provider's declared `max_cost_usd` is
`unavailable`, not evidence. A hook over `timeout_secs` (default 10) is
`unavailable` and the turn proceeds without memory. Only a turn that
completed, and passed the turn check if it is on, is retained.
