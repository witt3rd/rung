# rung-ui

The page for watching, queueing for and configuring continuous rung hosts. Not published.
Design: `docs/reports/rung-ui-design-v1/` (the acceptance scenario, the host pieces, the five slices).

**Slice 0: a read-only observer over a recorded event log.** Instances overview, then one
instance: Now and Turns. Slice 0 alone changes nothing on a host (the queue box of S1, below, is the one control). A running host's doors do not exist yet, so the app
reads recorded files; that one module (`src/data/source.ts`) is the seam a host's read doors replace.

    npm ci
    npm test                  # folds on the recorded runs, the canary check
    npm run build             # app + prepared data in dist/
    npm run census            # the element and word census, on the built app (needs a built dist/)
    npm run census:selftest   # the census must fail the pages made to fail it
    npm run capture           # first-screen captures at 390 and 1920 into census/shots/ (--full: long pages too)
    npm run dev               # prepares the data, then Vite

Needs node 24 or later (type stripping: the data script and the tests run `.ts` directly) and Playwright's browser
(`npx playwright install chromium-headless-shell`).

## Slice 1: the live view, against a contract

The same app is live when a gateway answers `api/health` beside it, and recorded when it does not (slice 0's files). Live:
the whole record is read once through `/v1/record`, then followed over `/v1/events` (H5; `src/live/`: an SSE parser, an events client that
keeps the number of the last line it applied and resumes from it, a store that shows partial text only for turns whose start it saw, and the
overlay that lays the running turn's streamed text and tool over the record). The host's own `/v1/summary` decides the state word.

The doors' bodies: the gateway (H9), the list, summary and record doors (H3) and the event stream (H5) are on master and written in
`docs/rung-host-api.md`; `contract/README.md` says what the page asks of them and where they are a
superset of what it first proposed. `test/contract.test.ts` states them as checks and runs unchanged against
`mock/` (a gateway and simulated hosts in this directory) and against a real gateway:

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

- **Where it reads**: the host's context door (`GET /v1/turns/{n}/context?call=k`, host S4; see `contract/README.md`). Where that is absent, fails
  or answers what cannot be read, the **record's own strata** (`src/context/fold.ts`): the history rebuilt from the earlier turns' logs, the
  turn's header, the messages before the call, the epoch's title line, the provider's counts. For a record that predates the door, the door itself
  answers with its note and the bands it cannot show; the page marks each as not recorded, with the host's reason. The cache marks are the host's
  where it had the whole request; where bands are missing the page derives them and says "unknown" rather than guess.
- **Fixtures are the real door's answers** (`test/fixtures/context/`): a real host's record with six of its calls, and an old run's, each as the
  built gateway answered; `prepare-data` copies them beside the record (`data/<id>/context/<turn>-<call>.json`).
- `npm run test:contract:context:real` runs the checks for the door against the built `rung-gateway`; `RUNG_CONTRACT_URL=… npm run test:contract:context`
  runs them against any gateway (every check skips, by name, on one without the door).
- On a **running** instance the page asks the door once for a past turn and keeps the bands the reader has open as the host writes record lines:
  the query is keyed on the turn, the call, the mode and whether the turn's log is written, not on the record's length (`npm run strata-live-proof`,
  against the mock gateway and a host that keeps writing).
- Every text a band shows passes the page's redactor, which is linear in the text (a port of the memoised scan in `rung-agent-core/src/redact.rs`;
  `test/redact-linear.test.ts` doubles hostile inputs and bounds the time).
- `npm run strata-proof` drives the built app in a real browser at 390, 1280 and 1920 (bands, boundaries, markers, whole text, the selector, an
  old run's missing bands, no write control, and a hostile door answer: markup, a key shape, a 1 MB line of mixed scripts: nothing runs, leaks, is cut or
  freezes; nothing is written to the console, storage, the title or the address; every request is a GET).
- The plain-words line of each band (what it holds) is at the top of its opened drill-in, not on its row: the first screen stays inside the census's word budget.

## Slice S1: the queue box and list, and the owner mark

On an instance's Now page, under "Doing now": one text box, the switch *Do next / When free / Steer* and one **Send**. Only Do next is served by
the host today: the other two are shown disabled, with one sentence saying they come later. The page answers only after the host has written
the item down (it shows "Sending.", locks the box and keeps the text until the host answers); the box is emptied only on success, and a refusal
keeps the text and says why in plain words (`contract/README.md` has the table). An item shows in the list only when the host's `GET /v1/queue`
has it. One press is one write: a second press while it is in flight is the same request, and a press repeated after a lost answer carries the
same key, so the host adds it once (`src/queue/send.ts`). A message of nothing visible (a zero-width space) is empty. The list shows what is being
answered and what waits (position, text whole with "Show more" for a long one, what was asked, who sent it, its age); the owner's items are a
group on top with one line saying peer items can never be moved above them, and a peer item the desk holds says why. A stopped instance shows
"Start the instance first." and Send is not offered. In the turn view an item the owner sent wears a filled Owner tag, whether it was a steer or a
task and when it was sent, and one that came in while another turn was running shows the moment the agent saw it. Everything else on the page still
only reads. The doors are the host's (`docs/rung-host-api.md`, "S1 — the queue"); the mock (`mock/queue.ts`) answers with the same bodies, and
`test/contract-queue.test.ts` runs against the mock and against a real host behind a real gateway (`mock/real-host.ts`). `npm run queue-proof` runs the
page against the mock in a real browser, and `npm run real-queue-proof` against the real host and gateway, with captures at 390, 1280, 1920 and 2878.

## Where the data comes from

`scripts/prepare-data.ts` copies every recorded run's record from `rung-host/live/runs/*/state/record/` into
`public/data/<id>/record.ndjson` (gitignored; `dist/data/` in a build), plus synthetic instances from
`src/synthetic.ts`, and writes `index.json`: one summary per instance. The summary and every fold
(`src/record/folds.ts`) are the same code the page runs, so a host's summary door has one shape to match.

- **State word**: from the record and from whether the lock on the state directory is held. A recorded run has
  no process, so no lock: it is Stopped (it ended on a halt) or Down (it did not). The synthetic instances stand
  for a held lock with a flag, `lockHeld`; live, the host's `/v1/summary` supplies the state word (slice 1).
- **Credentials**: one redactor (`src/record/redact.ts`) replaces the exact value of every key a run's config
  names by environment variable (`api_key_env`), the variables listed in `RUNG_REDACT_ENVS` (the one surface
  the agent crates use too) and the well-known provider key variables, and the shapes of well-known keys. It runs when the data is
  prepared and again when the page loads the record. It is a port of the token-shape part of the workspace's one redactor (`rung-agent-core/src/redact.rs`), from the same table (a test reads the Rust
  file and fails if the tables differ); header values, URL passwords and secret-named assignments are not ported and are the host's to remove.
  It replaces; it never shortens, drops or caps.
  `test/canary.test.ts` puts a canary key in the environment, prints it from a tool into a record, and fails if
  the value is anywhere in the prepared data. A key variable a config names but the environment lacks is refused
  (exit 2, by name) unless `--allow-unset-keys` accepts a shapes-only pass, which warns; the npm scripts pass it,
  because the keys that wrote the recorded runs are not on this machine. A second check plants a phrase the real
  records contain as a secret and fails if it survives. `--out` is emptied, so it is refused unless it is strictly
  inside `ui/public`, `ui/dist` or `ui/.test-tmp` and either empty or marked by an earlier run.
- **No cap**: turns are paged ("Show 50 more", "Show all"), long text is whole and folded (see "No caps" below), a turn's decisions and
  calls fold under "Details".

## Evidence (`census/evidence/`)

- `canary-first-run.txt`: the canary check, run before the redactor existed. It failed.
- `prepare-data-first-run.txt`: the checks for an unset key variable and for an `--out` the script does not own,
  run against the script before those refusals existed. Four failed.
- `contract-first-run.txt`: the contract tests before the mock existed. Every one failed.
- `contract-mutations.txt`: five faults put into the mock on purpose (a slow reader never dropped, a replay that repeats a line, a key in an
  answer header, a delta with an id, a read-only token that may write); the tests that caught each.
- `live-redact-first-run.txt`: the redaction and torn-tail tests against the store and loaders as they were before: all six failed (summary,
  instance list, a key split across pieces, a torn tail after a cut or a join).
- `live-review3-first-run.txt`: the third round's tests against the code before: the redactor port and its parity test, and a reset that
  kept the streamed text.
- `strata-first-run.txt`: the strata tests before the code existed (the four test files could not load their modules).
- `strata-door-shape-first-run.txt`: the first reader, written to a proposed door shape, against the real door's answers: 15 of 21 checks fail.
- `strata-mutations.txt`: five faults put into the strata code on purpose (a band drawn cached where the cut passes through it, the host's marks trusted where bands
  are missing, a tool definition that loses its tail, a recall-less turn drawn as a gap, a page that cuts a long text); the checks that caught each.
- `strata-review-first-run.txt`, `strata-review-mutations.txt`: the review round's checks against the page as it was (the open band closing on a running host, the
  quadratic redactor, a 1 MB line freezing the layout), and thirteen faults put into the page, the loader and the redactor on purpose, each caught (one only after its check was tightened).
- `strata-proof.txt`, `strata-census.txt`: the browser proof (at 390, 1280 and 1920) and the census (390 and 1920) with the strata pages.
- `live-review4-first-run.txt`: the word-boundary tests (a Bearer inside a word, a word after a JSON escape) against the redactor before.
- `h5-first-run-mock.txt`, `h5-first-run-ui.txt`: the contract tests written for H5's frames against the mock as it was, and the page's live code against the mock
  once it spoke H5: what failed before the page followed the real stream.
- `h5-summary-retry-first-run.txt`: the summary-retry test against the store before the retry.
- `status-first-run.txt`: the shared-status tests, and a proof step where the host stops under two open pages with a stale summary, against the code before: the Now page did not say Not running.
- `review-round5-first-run.txt`: the tests for the two inspector threads (a partials=only stream never ended; a reset left the old stream's cursor) against the code before.
- `nocaps-first-run.txt`, `nocaps-mutations.txt`: the no-caps tests and the 1 MB reply proof against the app as it was on master (a long text printed whole and open: 810,473 px tall; the record read in one request with a limit of a hundred million; a long argument left out of its line), `pr302-review-first-run.txt` and `pr302-review2-first-run.txt` (the review rounds' tests against the code before), and seventeen faults put into the new code, none surviving.
- `live-mutations.txt`: forty-one faults put into the live code one at a time; every one is caught by a test.
- `live-proof.txt`: the live proof against the mock, as run.
- `real-gateway-first-run.txt`: the tests for the real gateway's rows and a missing event stream, against the UI source as it was on master.
- `real-proof.txt`: the proof against the real gateway binary (the list, a stopped instance, the record), as run before the queue page; `contract-real.txt`: the contract tests against it, as run. `real-proof-queue.txt` is the same proof run again with the queue page's checks added, against a gateway built from master; it supersedes `real-proof.txt`.
- `queue-first-run.txt`: the first S1 tests (owner mark, queue wire and send, queue contract) before any queue code existed. 19 of the 20 listed tests failed (the three suites for the new modules could not load); the one that passed is the owner-mark check that a text is kept whole, which the fold already did.
- `queue-real-first-run.txt`: the same tests rewritten for the host's real doors (items with a state, `not_running`, `key_reused`, an invisible-only message, the draft kept), run before the code and the mock changed. 19 of 28 listed tests failed and 9 passed (the checks that the old and the real doors share).
- `queue-review-first-run.txt`: the browser proof with the navigation-race checks added, run against the page before the fix: the two new checks failed (and the row count, which only counted the added item).
- `ocr-check.txt`: the OCR of every capture the queue slice added or changed, searched for paths, names and addresses: no match.
- `queue-mutations.txt`: faults put into the S1 code one at a time (the sender, the reader, the fold, the mock's doors, the panel, the per-instance state); the test or proof check that caught each.
- `queue-proof.txt`: the queue proof against the mock, as run. `contract-queue.txt`: the queue contract tests against the mock and against a real `rung-host` behind a real `rung-gateway` built from master (all 15 run, none skipped); `real-queue-proof.txt`: the browser proof against the same real pair.
- `census-selftest.txt`: thirteen pages built to break one budget each; the census named every one.
- `census-first-run.txt`: the census on the first draft of the pages. It passed, so the first draft was judged
  by looking at the captures; what looking changed is listed below.
- `census-final.txt`: the census on the pages as committed.
- `shots/`: the first screen of every page at 390, 1280, 1920 and 2878, looked at. `turns-why` is where the "Why" link
  lands (turn 51 of a recorded run, details open), not the top of the list.

What looking changed (the census passed before each of these): a turn the host never ran was listed as the last
turn (a boundary that decided and then halted); the last turn of a stopped run read "running" (now: cut off);
one tool line printed a whole note (now the line shows a short argument and the call is whole under Details);
paragraphs carried blank lines from the model's own padding; instance names wrapped in the list; "Decided" showed
the day's empty decision, not the last one that meant something; a time in the future read as a date; "Show more"
appeared with nothing more to show.

## No caps

Nothing the app shows is cut. A long text (a reply, a long argument, a tool result, the text of a message) is folded for the eye: every character is in the
page, the fold keeps it to a screenful, "Show more" opens it and "Show less" folds it again (`src/pages/LongText.tsx`). A summary line (a turn in the Recent
turns list, the line a stopped instance last said) is one line, with "Show whole" for the rest. A list is paged ("Show 50 more", "Show all", "Show less").
A long argument is not on its call's line and is offered whole under it. The record is read a page at a time by following `next`; a page size is how much is
asked for at once, not how much there may be.

## Rules this directory keeps

API first (nothing here is possible only through the page). No caps or truncation: folds and paging. Colour is
attention only: amber when the owner must act, red for failure, one accent for the one filled control. Plain words:
no ids or hashes in the main view. Nothing in rung depends on any consumer (`docs/_consumer_guard.py` scans this
directory too).
