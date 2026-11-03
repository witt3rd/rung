# AGENTS.md — rung-agent/python (the Harbor validation suite)

Moved from `skills/rung/SKILL.md`. Charter for the whole repo: [`../../AGENTS.md`](../../AGENTS.md); the CLI is [`../AGENTS.md`](../AGENTS.md).

Out-of-tree adapter: `rung-agent/python/rung_harbor/agent.py`. Do not fork
Harbor. Harbor `examples/tasks` mixes **agent** tasks with **harness**
tests (network policy, verifier modes, CUA, CUDA). The suite
(`rung_harbor.suite`) is only the agent tasks, cheap-to-dear. Terminal-Bench
2.0 is phase 2 after that ladder is green.

Key: `doppler run -p fleet -c dev_work` (`OPENROUTER_API_KEY`).
Suite model: `openrouter/~deepseek/deepseek-v4-flash-latest`.

```bash
cargo build -p rung-agent --release   # when the binary changed
PYTHONPATH=<rung>/rung-agent/python python3 -m rung_harbor.validate list
PYTHONPATH=<rung>/rung-agent/python \
  doppler run -p fleet -c dev_work -- \
  python3 -m rung_harbor.validate next
python3 -m rung_harbor.validate run cwd-capture   # redo one case
python3 -m rung_harbor.validate show cwd-capture
python3 -m rung_harbor.validate import            # once: copy Harbor jobs in
```

Evidence: `rung-agent/harbor-runs/<UTC>-<id>/` plus `index.jsonl`.
Gitignored. `list` / `next` read the index in this repo, not Harbor's
`jobs/`. `run` always makes a new timestamped folder.

A skip is a **product gap** (e.g. MCP-over-ACP). A fail is a **bug or a missing
affordance**. Read `harbor-runs/<stamp>-<id>/**/rung-agent.txt` before
changing the kernel.

Alpine (`hello-alpine`) is skipped: the release binary is glibc
(`ld-linux-x86-64.so.2`). Without a loader Alpine says `required file not
found`. `gcompat` loads the ELF then dies on `__res_init` (resolver). A
musl/static build would unskip this; do not treat it as a Harbor harness
bug.
