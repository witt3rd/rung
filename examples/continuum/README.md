# Continuum research assistant

A `rung-host` configuration (`continuum-assistant.yaml`); no new code.

- Ladder: the named ladder, explicit in the file.
- Read-only: rung-host has no shell; `ws_write` is confined to the workspace.
- Reads: `continuum/` under the workspace (a mirror of the Continuum project
  and experiment results; host file tools cannot leave the workspace) and
  new papers through `web_read`.
- Writes: `brief.md` in the workspace; suggestions end the brief.

Setup: `mkdir -p data/{inbox,workspace/continuum}`, copy or rsync Continuum
sources into `data/workspace/continuum/`, export `OPENROUTER_API_KEY`, then
`rung-host run --config continuum-assistant.yaml`. Stop: `touch data/STOP`.
