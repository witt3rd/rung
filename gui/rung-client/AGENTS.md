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
- `Summary` carries what the screens read of the host's summary (turn, spend,
  queue, desk, governor, what it waits for); each field is optional and a summary in
  an unexpected shape reads as absent, so one odd row never takes the list down.
- `Client::events(id, after)` is one event-stream connection (an iterator of
  `Frame`s: open, record with its number, delta, end, unknown kept);
  `follow` is the loop for a worker thread: it keeps the last record number,
  reconnects with `Last-Event-ID`, backs off (`Follow`: 1 s doubling to 30 s,
  reset after a healthy minute), starts over on a 409, and stops on a flag. No
  record line is interpreted here.
- Queue and owner controls (`queue.rs`): `queue` reads (read-only role may);
  `queue_add`, `queue_move`, `queue_cancel`, `stop`, `release` write: they
  succeed only for the owner role (a client with no token), and a read-only token
  is refused 403 in the gateway's words. Ids in paths are percent-encoded; a retried
  call takes a key. Nothing here sends on its own.
- `Client::context(id, turn, call)` reads the context door (S4): the bands of a
  call with the host's sizes and cache marks and the provider's counts; texts
  whole and already redacted; a turn's messages are counted, not kept.
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
