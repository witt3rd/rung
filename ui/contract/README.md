# The live-view contract (slice 1)

What the live instance view asks of a host, through the gateway. Names and access classes are the ones in
`docs/rung-host-api.md` (the gateway section, on the host builder's branches until it merges); the **bodies of the H3 and H5
doors below are not written there yet**, so this file proposes them. They are meant to be agreed into that document by the host
builder; until they are, `test/contract.test.ts` is the executable statement, and it runs unchanged against the mock in
`mock/` and against a real gateway (`RUNG_CONTRACT_URL=http://… RUNG_CONTRACT_INSTANCE=id npm run test:contract`).

Everything goes through the gateway: `GET /api/health`, `GET /api/instances`, and `/api/i/{id}/v1/…` (a pass-through that adds
the instance's key and streams without buffering). A read-only token, sent in the `Authorization: Bearer …` header only (the page never puts a token in a URL, where it would reach logs and history; the gateway's own query form is not part of this contract), may only read;
a write by it is 403 `{"error":"read_only",…}`. Errors are always `{"error": code, "message": text}`. No answer, stream line or
header ever carries a key.

**The host redacts first.** A host redacts credentials at every door and on the stream, before it writes the answer (the redactor of
H4): the summary and record answers, every stream line, and the text of every delta (a host that streams text holds back what may be the start of a key until it is whole). The page redacts what it receives as well, over
the whole of a streamed text and never a piece (a key may arrive in pieces), but that is a second line, not the first: a page cannot
undo what a host has already put on the wire. The contract tests' canary checks are the host's to pass.

## `GET /v1/summary`

```json
{"contract": 1, "state": "Working", "doing": "Tidy the notes index", "needs_you": false,
 "turn": 214, "last_seq": 8123, "last_at": 1791191217095,
 "requests": 357, "quota": 1000, "spend_usd_day": 0.0}
```

`state` is one of Working, Answering, Free time, Waiting, Stuck, Down, Stopped, derived by the host from the record and from
whether it holds the lock on its state directory. `doing` is one line. `turn` is the turn running or last run, or null.
`last_seq` is the number of the last record line. `quota` is null when the host has none. The day is the UTC day.

## `GET /v1/record?offset=&limit=&order=`

```json
{"lines": [{"seq": 1, "at": 1791191086634, "kind": "host.start", "…": "…"}], "offset": 0, "limit": 500, "total": 8123, "next": 500}
```

The record's lines, oldest first (`order=desc` newest first). `offset`, `limit`, `total`, `next`: `limit` has no maximum (it may be as
large as `total`), `total` is exact, `next` is the next offset or null. Line numbers have no gaps, so oldest first `offset` is
`seq - 1`. An unknown parameter value is 400 `bad_request`.

## `GET /v1/events?after=N`

A server-sent event stream: the record, followed. `Last-Event-ID: N` resumes the same way as `?after=N` (the header wins if
both are given). Events:

| event | id | data |
|---|---|---|
| `record` | the line's `seq` | the record line, as `/v1/record` gives it |
| `caught_up` | none | `{"last_seq": N}`, once, after the replay and before the first live line |
| `delta` | none | what the running turn is doing now: `{"turn": 7, "kind": "text", "text": "…"}` (a piece of the model's text) or `{"turn": 7, "kind": "tool", "name": "ws_write", "phase": "start" \| "end"}` |

- **Replay**: every line with `seq > N` is sent first, in order, then live lines. Over a cut and a resume each line is delivered
  exactly once from the client's point of view: the client keeps the last `seq` it applied and ignores anything at or below it.
- **Deltas** are short-lived: never numbered, never replayed, never in the record. The finished text arrives in the record.
- **Keep-alive**: a comment line (`: keepalive`) at least every 15 seconds.
- **A slow reader is dropped**: when the host's unsent output for one connection passes a bound, it closes that connection. It does
  not slow the host or any other reader, and nothing is lost: the record holds it and the reader resumes from its last `seq`.
- An `after` beyond `last_seq` is 400 `bad_request`; a host that was restarted keeps its numbers, so a client's `after` is valid.

## `GET /api/instances`

As in the gateway section of `docs/rung-host-api.md`; `summary` is the instance's `/v1/summary` as it answered. An instance that does
not answer is listed with `reachable: false`.
