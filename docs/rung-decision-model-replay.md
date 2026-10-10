# Decision model replay: `typesafe/jev-1.13` vs `microsoft/microsoft-decision-1`

Informative. Evidence for [#259](https://github.com/witt3rd/rung/issues/259).
Run 2026-10-10 through OpenRouter's
`POST /api/v1/systemone` with the router key from Doppler (read-only), the
accounted path. Synthetic content only.

## What was replayed

All 10 recorded decide requests in the repository, each sent once with only
`model` changed to `microsoft/microsoft-decision-1` (served as
`microsoft/microsoft-decision-1-20261009`):

- 6 `rung-agent` turn-check fixtures (`tests/fixtures/decide/turn_check/`),
  recorded from the old model: **real old answers to compare with.**
- 4 `rung-host` desk fixtures (admit ×3, pack ×1): requests are synthetic and
  their recorded answers were **authored, not recorded**, so they are not
  scored for agreement; they exercise shape, latency and cost only.

Spend: $0.0009 in total (cap was $5). One request met a provider 429 and
succeeded on retry.

## Result

| | old model (recorded) | new model (replay) |
|---|---|---|
| answers agreeing, 6 real fixtures (noul at 0.5, choice by label) | | **26 of 30** |
| cost, the 6 real fixtures | $0.000895 | $0.000741 (−17%) |
| latency per request, all 10 | not recorded | mean 0.52 s, max 0.92 s |
| output tokens per request | ~136 | 5–14 |

Per-input-token price is identical: every request billed at
$0.042 per million input tokens for both models (e.g. 523 tokens → $0.000021966).
The saving comes from the new model counting fewer input tokens for the same
request (e.g. 873 → 523) and emitting almost no output. So
`JEV_USD_PER_INPUT_TOKEN` in `rung-host/src/desk/mod.rs` is already right for
the new default and is unchanged apart from its comment.

The 4 disagreements (of 30):

- `probe_a_decode`, `outcome`: `narrated` (0.25) → `done` (0.66).
- `long_state`, `outcome`: `narrated` (0.53) → `done` (0.85).
- `blocked_push_error`, `claims_unperformed_action`: 0.74 → 0.27.
- `info_answer`, `claims_unperformed_action`: 0.30 → 0.91.

The two `outcome` flips matter most: those turns reported work whose supporting
actions are partly outside the window the judge sees, and the old model's
reading was itself low-confidence (0.25 and 0.53). The new model is more
confident and more favourable to the turn. On the clean `narrated_note` the
two agree (`narrated`, 1.0 vs 0.99). Whether the new reading is better needs
labelled turns; this replay only shows where they differ.

## Request and response shape

No decider change was needed. The new model accepts the same body
(`{model, state, questions}`) and returns the same shape: `answers` (typed
`noul` / `choice`, with `probabilities` and `confidence` for choices),
`usage` (`input_tokens`, `output_tokens`, `cost`), `model`, `id`. `read_answers`
decodes it; one real response is committed as
`rung-agent/tests/fixtures/decide/turn_check_decision1/narrated_note.json`
and read by `the_default_model_response_reads_through_the_same_decider`.
The old fixtures stay as recorded from `typesafe/jev-1.13`.

## Switching back

`desk.model: typesafe/jev-1.13` (host), `turn_check.model:` (agent), or
`RUNG_DECIDE_MODEL` (recorder); see `docs/rung-host.md`.
