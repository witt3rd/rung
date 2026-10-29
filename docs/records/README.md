# Records

Dated, append-only history, one directory per subject. A record is never
rewritten; a correction is a new dated section at the end. CI enforces it
(`docs/_records_guard.py`: old text stays a byte-for-byte prefix; what is added
begins with a dated heading). Living specs stay
in `docs/`.

- [`rung-host/`](rung-host/README.md) — the continuous host's live runs, findings and design records.
