# Recall-quality fixtures

Generic personal-assistant notes (`notes.jsonl`, one `{"id","text"}` per line,
oldest first; later notes may correct earlier ones) and questions
(`questions.jsonl`, `{"q","expect":[ids]}`; the expected id is the *current*
note, e.g. the correction). No real names, ids or secrets.

Score with the baseline (BM25) provider:

    cargo test -p rung-memory --test recall_quality -- --nocapture

## Rebuilding on your own notes (private set stays uncommitted)

1. Write your notes to `notes.jsonl` and ~30 questions to `questions.jsonl` in
   any directory, in the same shape. Pick, per question, the note id that
   should answer it (the latest one when facts were corrected).
2. Run `RUNG_RECALL_FIXTURES=/path/to/dir cargo test -p rung-memory --test recall_quality -- --nocapture`.
   The table prints hit@1, hit@5 and MRR. The regression floor is only
   asserted for the committed set; custom sets just print.
