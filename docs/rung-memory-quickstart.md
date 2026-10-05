# Quickstart: rung-agent with memory

Informative. The contract is [rung-memory.md](rung-memory.md). Every command
below was run as written (rung-agent built from this tree, a local
OpenAI-compatible endpoint named in `config.yaml`, the Jev-Mem provider image
in its offline `--jev fake` mode).

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

## 2. An MCP provider (Jev-Mem container)

Clone [`witt3rd/rung-memory-jevmem`](https://github.com/witt3rd/rung-memory-jevmem),
build the image once (`docker/run.sh build`, about 2 minutes), then one command:

```bash
docker/run.sh serve --jev fake      # offline, zero spend
# live Jev through OpenRouter, capped by the provider's spend ledger:
doppler run -p fleet -c dev_work -- docker/run.sh serve --jev live
```

It listens on `127.0.0.1:9000` (`/mcp`, health at `/healthz`) and keeps its
store in `./data` (or `$JEVMEM_HOST_DATA`). Check it, then point rung at it:

```bash
curl -s 127.0.0.1:9000/healthz
# {"ok":true,"marker":"rung-memory/1","backend":"fake"}
rung-agent --memory-check mcp:http://127.0.0.1:9000/mcp     # 7 lines, all "pass"
rung-agent --memory mcp:http://127.0.0.1:9000/mcp --json --tools none "My deploy day is Thursday. Reply ok."
```

Or in `config.yaml`: `memory: { provider: "mcp:http://127.0.0.1:9000/mcp" }`.
Prefer HTTP to stdio: a stdio provider is started once per prompt and reloads
its model each time.

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
- **Jev-Mem**: stores live under `<data>/scopes/<salted-hash>/`, so a
  directory cannot be mapped back to a scope name. Inspect from the agent with
  its `memory_search` and `memory_stats` tools. Reset one scope by switching
  to a new `RUNG_MEMORY_SCOPE`; reset everything by stopping the container and
  removing the data directory (`rm -rf ./data`).

## 5. Cost and latency

Measured here on the offline provider and local endpoint; the live figures are
the provider repository's (its README and W4 report), not re-measured here.

| setting | per-turn memory cost | memory latency |
|---|---|---|
| `baseline` | `$0` | under 1 ms (`latency_ms: 0`) |
| `mcp:` Jev-Mem, `--jev fake` | about `$0.00003` (simulated) | 12 to 15 ms per hook |
| `mcp:` Jev-Mem, `--jev live` | Jev asks, about `$0.00005` each, capped by `--cap-usd` (default `0.05`) | Jev about 0.2 s an ask; W4 unloaded recall p95 2.83 s against rung's 10 s hook timeout |

A recall costing more than the provider's declared `max_cost_usd` is
`unavailable`, not evidence. A hook over `timeout_secs` (default 10) is
`unavailable` and the turn proceeds without memory. Only a turn that
completed, and passed the turn check if it is on, is retained.
