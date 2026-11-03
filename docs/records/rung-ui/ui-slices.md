# rung-ui: what each slice built

Record, moved out of [`ui/AGENTS.md`](../../../ui/AGENTS.md) on 2026-11-03, text as it stood there:
the per-slice behaviour of the page (slice 1 the live view, slice 5 the context inspector, slice S1
the queue box). How to run and the rules the directory keeps stay in `ui/AGENTS.md`; the doors the
page reads are in [`rung-host-doors.md`](../../reference/rung-host-doors.md) and
[`rung-host-queue.md`](../../reference/rung-host-queue.md).

## Slice 1: the live view, against a contract

The same app is live when a gateway answers `api/health` beside it, and recorded when it does not (slice 0's files). Live:
the whole record is read once through `/v1/record`, then followed over `/v1/events` (H5; `ui/src/live/`: an SSE parser, an events client that
keeps the number of the last line it applied and resumes from it, a store that shows partial text only for turns whose start it saw, and the
overlay that lays the running turn's streamed text and tool over the record). The host's own `/v1/summary` decides the state word.

The doors' bodies: the gateway (H9), the list, summary and record doors (H3) and the event stream (H5) are on master and written in
`docs/reference/rung-host-doors.md`; `ui/contract/AGENTS.md` says what the page asks of them and where they are a
superset of what it first proposed. `ui/test/contract.test.ts` states them as checks and runs unchanged against
`ui/mock/` (a gateway and simulated hosts in this directory) and against a real gateway:

    npm run test:contract                                                       # against the mock
    RUNG_CONTRACT_URL=http://127.0.0.1:8787 RUNG_CONTRACT_INSTANCE=alpha npm run test:contract   # against a real one
    npm run mock -- --port 8742 --app dist     # the built app, the mock gateway, and a host writing turns on a real clock
    npm run live-proof                         # a real browser against the mock: latency, a cut with the page offline, captures
    cargo build -p rung-gateway && npm run test:contract:real   # the same contract tests against the built rung-gateway (H3 reads and H5 events; only the partial-frame check needs a running host and runs on the mock)
    cargo build -p rung-gateway && npm run real-proof           # a real browser against rung-gateway over a registry folder: the list, a stopped instance's start command, the record, a turn arriving over the stream

Checks that need to write to the host (a line within a second, a cut, a slow reader, a halt) need the target's control hooks and say so when
skipped; an external gateway (`RUNG_CONTRACT_URL`) has none, so those skip there.

## Slice 5: the strata page for past turns (the context inspector)

Every turn has a **Context** link (`#/i/<id>/turns/<n>/context?call=<k>`). It opens what was sent to the model for one model call of that
turn, as seven bands in the order the model reads them: Tools, System prompt, (cache boundary 1), Epoch summary, (cache boundary 2), History,
Message, Memory recall, This turn so far (the turn's header is one message on the wire: its message part first, its recalled memory after it). Per band: the host's estimate (marked `~`) beside the provider's own count for the call (the truth),
a cache marker (cached, new, partly cached) from the call's cached-token count laid against the bands in order, and a drill-in that opens the
band's whole text (a long text folds under Show more; nothing is cut; each tool definition and each past turn is its own row; a text with a line
of more than 50,000 characters is not wrapped, because a browser takes tens of seconds to wrap a megabyte of mixed scripts, and scrolls sideways). The bottom line is
cached N of M provider tokens. A small selector picks the call when a turn made several; a cache break at a call is named. Read-only.

- **Where it reads**: the host's context door (`GET /v1/turns/{n}/context?call=k`, host S4; see `ui/contract/AGENTS.md`). Where that is absent, fails
  or answers what cannot be read, the **record's own strata** (`ui/src/context/fold.ts`): the history rebuilt from the earlier turns' logs, the
  turn's header, the messages before the call, the epoch's title line, the provider's counts. For a record that predates the door, the door itself
  answers with its note and the bands it cannot show; the page marks each as not recorded, with the host's reason. The cache marks are the host's
  where it had the whole request; where bands are missing the page derives them and says "unknown" rather than guess.
- **Fixtures are the real door's answers** (`ui/test/fixtures/context/`): a real host's record with six of its calls, and an old run's, each as the
  built gateway answered; `prepare-data` copies them beside the record (`data/<id>/context/<turn>-<call>.json`).
- `npm run test:contract:context:real` runs the checks for the door against the built `rung-gateway`; `RUNG_CONTRACT_URL=… npm run test:contract:context`
  runs them against any gateway (every check skips, by name, on one without the door).
- On a **running** instance the page asks the door once for a past turn and keeps the bands the reader has open as the host writes record lines:
  the query is keyed on the turn, the call, the mode and whether the turn's log is written, not on the record's length (`npm run strata-live-proof`,
  against the mock gateway and a host that keeps writing).
- Every text a band shows passes the page's redactor, which is linear in the text (a port of the memoised scan in `rung-agent-core/src/redact.rs`;
  `ui/test/redact-linear.test.ts` doubles hostile inputs and bounds the time).
- `npm run strata-proof` drives the built app in a real browser at 390, 1280 and 1920 (bands, boundaries, markers, whole text, the selector, an
  old run's missing bands, no write control, and a hostile door answer: markup, a key shape, a 1 MB line of mixed scripts: nothing runs, leaks, is cut or
  freezes; nothing is written to the console, storage, the title or the address; every request is a GET).
- The plain-words line of each band (what it holds) is at the top of its opened drill-in, not on its row: the first screen stays inside the census's word budget.

## Slice S1: the queue box and list, and the owner mark

On an instance's Now page, under "Doing now": one text box, the switch *Do next / When free / Steer* and one **Send**. Only Do next is served by
the host today: the other two are shown disabled, with one sentence saying they come later. The page answers only after the host has written
the item down (it shows "Sending.", locks the box and keeps the text until the host answers); the box is emptied only on success, and a refusal
keeps the text and says why in plain words (`ui/contract/AGENTS.md` has the table). An item shows in the list only when the host's `GET /v1/queue`
has it. One press is one write: a second press while it is in flight is the same request, and a press repeated after a lost answer carries the
same key, so the host adds it once (`ui/src/queue/send.ts`). A message of nothing visible (a zero-width space) is empty. The list shows what is being
answered and what waits (position, text whole with "Show more" for a long one, what was asked, who sent it, its age); the owner's items are a
group on top with one line saying peer items can never be moved above them, and a peer item the desk holds says why. A stopped instance shows
"Start the instance first." and Send is not offered. In the turn view an item the owner sent wears a filled Owner tag, whether it was a steer or a
task and when it was sent, and one that came in while another turn was running shows the moment the agent saw it. Everything else on the page still
only reads. The doors are the host's (`docs/reference/rung-host-queue.md`, "S1 — the queue"); the mock (`ui/mock/queue.ts`) answers with the same bodies, and
`ui/test/contract-queue.test.ts` runs against the mock and against a real host behind a real gateway (`ui/mock/real-host.ts`). `npm run queue-proof` runs the
page against the mock in a real browser, and `npm run real-queue-proof` against the real host and gateway, with captures at 390, 1280, 1920 and 2878.
