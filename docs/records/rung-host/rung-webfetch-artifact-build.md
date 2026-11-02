# Test request: web_fetch spill to a workspace file

Chef workspace run. Not to be run locally beyond `cargo test -p rung-host -j2`.

1. `cargo test -p rung-std --lib webfetch` and `cargo test -p rung-host --lib spill`
   (the 5 MB and 50 MB pages, the 100 MB resident-set check, the failure cases).
2. Start a host with `web: true` and the `read` and `web_read` groups; fetch a
   page above 32 KiB from a local server. Expect the header line, the notice
   with path, bytes and lines, and the file under `.web/` equal to the body
   (hash). `ws_read` with `from_line`/`to_line` returns that range and totals.
3. Fetch a page below the threshold: expect it inline, no `.web/`.
4. With `web_spill_bytes: 1000` in the config, a 2 KB page spills.
5. Turn `read` off for a turn: a large fetch returns the whole body; `ws_read`
   on a `.web/` path is refused naming the path.
6. Make `.web` a file: expect the whole body, a note in words, and a
   `web.spill_failed` line in the record.
