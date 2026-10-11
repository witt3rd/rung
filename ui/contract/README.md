# The live-view contract (slice 1)

What the live instance view asks of a host, through the gateway. Names and access classes are the ones in
`docs/rung-host-api.md` (the gateway and the H3 read doors are written there, on master; the H5 event stream is on the host builder's branch).
The bodies below were proposed before H3 landed; the real H3 doors are a superset (see the last section), and the **H5 bodies are still only
proposed here**, to be agreed into that document by the host builder. Until they are, `test/contract.test.ts` is the executable statement, and it runs unchanged against the mock in
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

## `GET /v1/events?after=N&partials=on|off|only`

Written in `docs/rung-host-api.md`, H5 (on master): `open` first, then `record` frames (the only ones with an id) with replay from the cursor
(`Last-Event-ID` beats `after`), `delta` partial frames while connected (never numbered, never replayed, never in the record), an `end`
frame when the host stops (`stopped`, `down`, `damaged`, `error`), and keep-alive comments. A cursor past the record is 409 `cursor_ahead`;
a bad query is 400. Record frames are pulled as the reader takes them (a slow reader holds only its place; nothing is dropped for it);
partial frames have a bounded buffer per reader and a reader that falls behind is told with a `behind` frame. The host redacts across the
pieces of a text before it sends them.

What the page does with it (`src/live/`): it keeps the number of the last line it applied and resumes from it (applying each line once,
and treating a skipped number as a broken stream); it asks the summary where the record stood at `open` (asking again, with a growing wait, while that fails), and shows no partial frame until
the lines to that number are applied (`Following` is said then); the turn that was running at that moment, and any turn after a `behind`
frame or a cut, is not shown streaming (its start was not seen), and its finished text is on the record; answer text blocks are joined as
blocks, a tool shows while `tool_call` or `tool_run` is open and is gone at `tool_done`; it redacts again on read over the whole text
received. An `end` frame is shown as "Not running", and the host is asked again every few seconds whether it is back. A 404 (a gateway with
no event stream) is "Recorded, not followed", said once.

## `GET /api/instances`

As in the gateway section of `docs/rung-host-api.md`; `summary` is the instance's `/v1/summary` as it answered. An instance that does
not answer is listed with `reachable: false`.

## Where the real doors are (docs/rung-host-api.md, H3 and H5)

Both are on master. H3's `summary`, `record` and list are a superset of the bodies above: `summary` adds `id`, `name`, `word`, `day`,
`start_command`, `waiting_for`, `boundary`, `turn_in_progress`, `queue`, `desk`, `governor`, `config`, `fold_hash`, `redaction`, `damaged`
and `problem`, and `state` may also be `Unreadable`; `record` adds `cursor`, `order`, `after`, `damaged`, `redaction`, and takes `after` and
`order` as well. The list rows add `word`, `source`, `last_at` and `start_command`; a row with no summary says why in `error`.

`test/contract.test.ts` is the executable statement and runs against the mock (`npm run test:contract`) and against the built
`rung-gateway` (`npm run test:contract:real`, with the suite playing the host's part for the record and holding the state lock). Against the
gateway every events check runs except the partial frames, which a gateway has only from a running host (it asks the host's own door): that
check runs against the mock, whose frames are H5's. The mock is a stand-in for what the gateway cannot yet be shown to do here, not a
source of shapes: where the two differ, the gateway and `docs/rung-host-api.md` win.
