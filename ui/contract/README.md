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

## `GET /v1/turns/{n}/context?call=k` (the strata page)

On master (host S4; the body and its errors are in `docs/rung-host-api.md`, "S4"). The page reads it in `src/context/wire.ts`, the one place
that knows the body. What the page relies on: `turn`, `call`, `calls`, `epoch`, `model.{requested,served}`, `provider.{prompt_tokens,
cached_tokens,cache_write_tokens}` (the provider's own count, the truth for the hit rate), `cache_event`, `rewritten`, `note`, `missing`
(bands the host cannot show, with `note` the reason), `problems`, `boundaries`, and `bands` in wire order (tools, system, epoch, history, message,
recall, this turn), each with `est_tokens` (the host's estimate) and `cache`. Text is whole; the page never shortens it. The recall band is absent
from a turn that had no recall, and that is an empty band, not a gap. For an old record the door answers 200 with `recorded: false`, the note
"this turn predates context recording", and the bands it does have.

- **Fixtures are the door's own answers**: `test/fixtures/context/ctx-demo/` is a real host's record (the mock engine, a recall, an owner message,
  five epochs) with the door's answer for six of its calls, taken through the built `rung-gateway`; `test/fixtures/context/2026-10-05-qwen-2h/` is
  the door's answer over a recorded run that predates the door. `prepare-data` serves them beside the record.
- **Where the page does not take the door's word**: the door lays the cached count against the bands it has; where it is missing bands
  (an old record), those marks are not the request's, and the page derives its own (which says "unknown" rather than guess).
- **When there is no answer** (no such door, a failure, a body that does not fit, a turn the host has not logged yet) the page draws what the
  record itself holds (`src/context/fold.ts`) and says why. A test lays that fold beside the door's answers over the same records.
- `test/context-contract.test.ts` (`npm run test:contract:context:real` against the built gateway, with the suite playing the host's part for
  the record; `RUNG_CONTRACT_URL=… npm run test:contract:context` against any gateway). Every check is skipped, by name, on a gateway without the door.

## The queue doors (S1)

`GET` and `POST /api/i/{id}/v1/queue`. The bodies are the host's own, written in `docs/rung-host-api.md` ("S1 — the queue", rung PR 297); this
file only says what the page reads of them. `test/contract-queue.test.ts` is the executable statement. It runs against the mock (`mock/queue.ts`,
which answers with the same bodies), and against a real host behind a real gateway (`mock/real-host.ts`: `cargo build -p rung-host -p
rung-gateway`, then `RUNG_CONTRACT_TARGET=real-host npm run test:contract:queue`); against a gateway without the door (404) its checks skip, by name.

**`GET /v1/queue`** is computed from the record, so a stopped instance lists too. It answers `items`, each with a `state` (`waiting`,
`answering`, `answered`, `withdrawn`), the page (`total`, `offset`, `limit`, `next`), `counts` and `record_lines`. The page asks for
`?state=open` (what is being answered and what waits) and follows `next` until it is null: the host pages the list, the page never cuts it.
An item: `id`, `text` (whole, redacted), `role` (`owner`, `peer`, `host`), `name`, `channel`, `kind`, `intent` (`next`), `order`, `position`,
`accepted_at` (the time sent), `state`, `turn`, `disposition`, `held` (the desk's reason in words, or null). The list is ordered answering,
then waiting (owner items first, then every other sender's, each in order of acceptance), then settled.

**`POST /v1/queue`** `{"text", "key"?, "intent"?}` answers only after the item is written and fsynced; the key becomes the item id, so a
repeat adds once: `201 {"id", "repeat": false, "item"}`, then `200 {"id", "repeat": true, "item"}`. Only `intent: "next"` is served; `steer`
and `free` are 400 until their slice, so the page shows them disabled. What the page does with the answers:

| answer | the page says |
|---|---|
| 201 / 200 | "Sent." / "It was already there; it is added once." The box is emptied only now, and only if nothing was typed meanwhile |
| 409 `not_running` | "Start the instance first." The draft stays |
| 409 `key_reused` | "That press was already added. Nothing new was written." The draft stays; the next press has a new key |
| 400, 403 `read_only`, 503 `no_owner_key` / `no_address` | the host's own words (or "This view can only read, so it cannot send."); the draft stays |
| 502, 503 `instance_not_answering`, a lost answer | the host's words ("... send it again with the same key"); the press keeps its key, so a second press adds once |
| 404 | "This instance cannot take messages from the page yet." (a gateway without the door); Send is disabled |

A message of nothing but white space or characters that draw nothing (a zero-width space, direction marks, a byte-order mark, the blank
fillers) is empty: Send is disabled, and `Sender` does not send it. The host refuses only an all-white-space text, so it would take a
zero-width space as a message; that is the page's rule on top of the host's.

**The owner mark** needs nothing new: a turn's asked items come from the record (`stimulus.accepted` carries `role` and `at`, and an `intent`
once the host writes one; `stimulus.admitted` is the moment the agent took them). An item that arrived while another turn was running is
shown with that turn and the moment of admission. The record has no line for the start of a tool call, so the page says "while turn N was
running", not "during a tool call".
