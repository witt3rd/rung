# Test request: rung-peer-header-nocap

Run on the chef workspace:

- `cargo test -p rung-host --test intent_run`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --check`

New test: `a_peers_huge_text_reaches_the_header_whole_and_redacted` (3 MB peer message, whole and hash-equal in the header, canary redacted).
