# AGENTS.md — rung-client

A typed, blocking client of the rung-gateway's doors. No GUI, no async runtime:
a window calls it from a worker thread. Slice 1 reads two doors:
`GET /api/health` and `GET /api/instances`.

- Holds no host logic: the state word and "doing" are the host's own, taken
  from the summary the gateway lists per instance (`Instance::state_word`,
  `Instance::doing`).
- The only credential is an optional read-only token, sent as
  `Authorization: Bearer`. It is never printed: `Debug` of `Client` says only
  that one is set, and error text is scrubbed of it.
- `Instance::role()` reads chief/steward from the id's first segment
  (`Role::of_id`); `Instance::unreachable()` is the gateway not reading it.
- Rows are read leniently (unknown fields ignored; registry rows and
  configured rows share only `id`), so the gateway may grow fields.

## Tests

```bash
( cd gui && cargo test -p rung-client )
```

- `tests/stub.rs`: an in-process stub gateway (std sockets): the bearer, both
  row kinds, every failure kind.
- `tests/gateway.rs`: the real `rung-gateway` and two real `rung-host`
  processes (chief and steward, mock engine, no network or key), read-only
  token. It needs their binaries from the main workspace: build them first with
  `cargo build -p rung-gateway -p rung-host` at the repository root, or set
  `RUNG_BIN_DIR`. A missing binary fails the test; it never skips.
