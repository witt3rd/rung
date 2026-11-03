# AGENTS.md — rung-host/live (bounded live runs)

Moved from `skills/rung/SKILL.md`. The scripts here run one bounded live window; their evidence is in `runs/` (kept in git, not checked by the doc guards). The host is [`../AGENTS.md`](../AGENTS.md).

`rung-host/live/` runs one bounded live window from the repo
(`live.sh RUN_DIR SECONDS`, keys from Doppler into the process env only,
host file writes traced with strace). Two keys: the host engine's
`RUNG_HOST_OPENROUTER_API_KEY` (fleet `dev_donald` only; its own router
workspace, free endpoints allowed) and Jev's accounted `OPENROUTER_API_KEY`
(`dev_work`). `arms.py` adds served models, malformed tool calls and a
coherence sample; `fallback-probe.py` tests the router's `models` array. Pre-register the measures in a
committed doc first; `analyze.py` computes them, `collect.py` copies the
evidence into `rung-host/live/runs/<date>/`, `scan.py` proves no key
landed, `diagram.py` draws the report figure. Reports build with the
pinned house tool (`docs/report.toml`): `uvx --from 'publishing[render] @
git+https://github.com/witt3rd/publishing@v<pin>' publishing build
docs/reports/<topic>-vN`.

Gotchas seen live (2026-10-04): the keyless listing cannot see the
account's data policy — on the work key four of five free rungs return a
404 with `ineligibility_reasons` (now `unroutable`, stepped past);
`qwen/qwen3.8-27b:free` serves but 429s upstream every few turns; Jev
answers in ~0.2 s at ~$0.00005 an ask. Run `route-probe.sh` before a run.
On the host key (2026-10-05): no guardrail refusals; the router sends an
overloaded upstream as HTTP 200 carrying `error.code: 503` (now classified
by code); `openrouter/free` is an opt-in bottom rung, not in the default
ladder (a listed router stands with `why: router`); listings flap within hours (space-bunny's
expiry vanished, nemotron-ultra went `endpoint_down`). Evidence:
`docs/records/rung-host/live-v4-hostkey.md`; named-ladder idle-rule soak:
`docs/records/rung-host/live-v5-idle-rule.md`.
