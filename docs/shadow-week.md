# Shadow week runbook

Goal: run the host a week in `desk.mode: shadow` (the rule decides, Jev is
asked and logged), tally disagreements per family, audit 50 of them.

1. Config: set `mode: shadow` (see `docs/rung-host.md`); keep the Jev spend cap.
2. Run the host for the week. Every answered ask writes `decision.<family>`
   lines with `choice` (the rule's), `jev_choice` and `agree` in the record.
3. At week's end, on a copy of the record:

   ```bash
   python3 scripts/shadow_audit.py path/to/record.ndjson -n 50 -o shadow-audit.md
   ```

   It prints agree/disagree per family and writes 50 random disagreements
   (`--seed S` makes the draw repeatable).
4. Fill each entry's `verdict` (rule / Jev / neither) and `note` in the sheet.
5. Report: the per-family table, the verdict counts, and the notes.

Decision lines without `agree` (non-shadow, or Jev did not answer) are not
counted. Tests: `cd scripts && python3 -m unittest test_shadow_audit`.
