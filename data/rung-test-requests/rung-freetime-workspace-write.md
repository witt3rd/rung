# Test request: free-time workspace write

Chef workspace run. Not to be run locally beyond `cargo test -p rung-host -j2`.

1. Start a rule-only-desk host (no decider) with the ceiling
   `core, memory, read, workspace_write, web_read`.
2. Let it run a free-time turn. Expect the tool set to be
   `core, memory, read, workspace_write` (no `web_read`).
3. Have the agent write a file (`ws_write`) in free time; expect `written`
   and the file in the workspace; a path outside the workspace is refused.
4. Expect no `tool.refused` with `why: disabled` for `ws_write` in free time.
