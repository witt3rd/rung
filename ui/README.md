# rung-ui

The page for watching, queueing for and configuring continuous rung hosts. Not published.
Design: `docs/reports/rung-ui-design-v1/` (the acceptance scenario, the host pieces, the five slices).

**Slice 0: a read-only observer over a recorded event log.** Instances overview, then one
instance: Now and Turns. Nothing here can change a host. A running host's doors do not exist yet, so the app
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
the whole record is read once through `/v1/record`, then followed over `/v1/events` (`src/live/`: an SSE parser, an events client that
keeps the number of the last line it applied and resumes from it, a store, and the overlay that lays the running turn's streamed
text and tool over the record). The host's own `/v1/summary` decides the state word.

The doors' bodies are proposed in `contract/README.md` (the gateway's names and access classes are in `docs/rung-host-api.md`,
which is on the host builder's branches, not yet on master). `test/contract.test.ts` states them as checks and runs unchanged against
`mock/` (a gateway and simulated hosts in this directory) and against a real gateway:

    npm run test:contract                                                       # against the mock
    RUNG_CONTRACT_URL=http://127.0.0.1:8787 RUNG_CONTRACT_INSTANCE=alpha npm run test:contract   # against a real one
    npm run mock -- --port 8742 --app dist     # the built app, the mock gateway, and a host writing turns on a real clock
    npm run live-proof                         # a real browser against the mock: latency, a cut with the page offline, captures

Checks that need to write to the host (a line within a second, a cut, deltas, a slow reader) need the target's control hooks and say so
when skipped; against a real host they run once it has a way to be made to write.

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
  prepared and again when the page loads the record. It replaces; it never shortens, drops or caps.
  `test/canary.test.ts` puts a canary key in the environment, prints it from a tool into a record, and fails if
  the value is anywhere in the prepared data. A key variable a config names but the environment lacks is refused
  (exit 2, by name) unless `--allow-unset-keys` accepts a shapes-only pass, which warns; the npm scripts pass it,
  because the keys that wrote the recorded runs are not on this machine. A second check plants a phrase the real
  records contain as a secret and fails if it survives. `--out` is emptied, so it is refused unless it is strictly
  inside `ui/public`, `ui/dist` or `ui/.test-tmp` and either empty or marked by an earlier run.
- **No cap**: turns are paged ("Show 50 more", "Show all"), long text wraps whole, a turn's decisions and
  calls fold under "Details".

## Evidence (`census/evidence/`)

- `canary-first-run.txt`: the canary check, run before the redactor existed. It failed.
- `prepare-data-first-run.txt`: the checks for an unset key variable and for an `--out` the script does not own,
  run against the script before those refusals existed. Four failed.
- `contract-first-run.txt`: the contract tests before the mock existed. Every one failed.
- `contract-mutations.txt`: five faults put into the mock on purpose (a slow reader never dropped, a replay that repeats a line, a key in an
  answer header, a delta with an id, a read-only token that may write); the tests that caught each.
- `live-redact-reset-first-run.txt`: the live-store tests before the review fixes (redaction of streamed lines and deltas, reset clearing the queue); the redaction test failed.
- `live-proof.txt`: the live proof against the mock, as run.
- `census-selftest.txt`: thirteen pages built to break one budget each; the census named every one.
- `census-first-run.txt`: the census on the first draft of the pages. It passed, so the first draft was judged
  by looking at the captures; what looking changed is listed below.
- `census-final.txt`: the census on the pages as committed.
- `shots/`: the first screen of every page at 390 and 1920, looked at. `turns-why` is where the "Why" link
  lands (turn 51 of a recorded run, details open), not the top of the list.

What looking changed (the census passed before each of these): a turn the host never ran was listed as the last
turn (a boundary that decided and then halted); the last turn of a stopped run read "running" (now: cut off);
one tool line printed a whole note (now the line shows a short argument and the call is whole under Details);
paragraphs carried blank lines from the model's own padding; instance names wrapped in the list; "Decided" showed
the day's empty decision, not the last one that meant something; a time in the future read as a date; "Show more"
appeared with nothing more to show.

## Rules this directory keeps

API first (nothing here is possible only through the page). No caps or truncation: folds and paging. Colour is
attention only: amber when the owner must act, red for failure, one accent for the one filled control. Plain words:
no ids or hashes in the main view. Nothing in rung depends on any consumer (`docs/_consumer_guard.py` scans this
directory too).
