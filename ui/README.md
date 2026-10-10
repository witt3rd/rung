# rung-ui

The page for watching, queueing for and configuring continuous rung hosts. Not published.
Design: `docs/reports/rung-ui-design-v1/` (the acceptance scenario, the host pieces, the five slices).

**Slice 0 (this slice): a read-only observer over a recorded event log.** Instances overview, then one
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

## Where the data comes from

`scripts/prepare-data.ts` copies every recorded run's record from `rung-host/live/runs/*/state/record/` into
`public/data/<id>/record.ndjson` (gitignored; `dist/data/` in a build), plus synthetic instances from
`src/synthetic.ts`, and writes `index.json`: one summary per instance. The summary and every fold
(`src/record/folds.ts`) are the same code the page runs, so a host's summary door has one shape to match.

- **State word**: from the record and from whether the lock on the state directory is held. A recorded run has
  no process, so no lock: it is Stopped (it ended on a halt) or Down (it did not). The synthetic instances stand
  for a held lock with a flag, `lockHeld`, until slice 1 supplies the real one.
- **Credentials**: one redactor (`src/record/redact.ts`) replaces the exact value of every key a run's config
  names by environment variable (`api_key_env`), and the shapes of well-known keys. It runs when the data is
  prepared and again when the page loads the record. It replaces; it never shortens, drops or caps.
  `test/canary.test.ts` puts a canary key in the environment, prints it from a tool into a record, and fails if
  the value is anywhere in the prepared data.
- **No cap**: turns are paged ("Show 50 more", "Show all"), long text wraps whole, a turn's decisions and
  calls fold under "Details".

## Evidence (`census/evidence/`)

- `canary-first-run.txt`: the canary check, run before the redactor existed. It failed.
- `census-selftest.txt`: thirteen pages built to break one budget each; the census named every one.
- `census-first-run.txt`: the census on the first draft of the pages. It passed, so the first draft was judged
  by looking at the captures; what looking changed is listed below.
- `census-final.txt`: the census on the pages as committed.
- `shots/`: the first screen of every page at 390 and 1920, looked at.

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
