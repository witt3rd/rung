# rung-host — live runs on the host's own key, with the free router (evidence note)

Informative. The host engine now reads its own router key,
`RUNG_HOST_OPENROUTER_API_KEY` (fleet Doppler config `dev_donald` only; its
own router workspace, which allows free endpoints that may log prompts, with a
daily limit). Jev stays on its accounted `OPENROUTER_API_KEY` (`dev_work`),
Shadow mode, cap $0.25/day per run. `live.sh` supplies both keys through
`doppler run --only-secrets`; neither is on a command line, in a file or in a
log. Key scan of every committed file below: 0 occurrences of either key.
Same harness, stimuli and analyzer as [run 3](rung-host-live-v3-qwen-2h.md).
Raw numbers per arm: `rung-host/live/runs/2026-10-05-hostkey-*/`
(`measures.txt`, `measures.json`, `arms.json`, `shadow-audit.md`,
`coherence-sample.md`).

## Which free models route on this key

Keyed probe (`route-probe.sh`, one tiny request each, 11:44Z,
`runs/2026-10-05-hostkey-2h/route-probe.txt`). No guardrail refusal: the work
key's data-policy 404s are gone.

| result | models |
|---|---|
| served (200) | `stealth/space-bunny-alpha`, `nvidia/nemotron-3-ultra-550b-a55b:free`, `nvidia/nemotron-3-super-120b-a12b:free`, `nvidia/nemotron-3.5-lightning:free`, `nvidia/nemotron-3.5-content-safety:free`, `inclusionai/ling-3.1-flash`, `inclusionai/ling-3.0-flash-sante:free`, `apodex/apodex-1.1-mini:free`, `dots-studio/dots-3-note-preview:free`, `liquid/lfm-2.5-2.6b:free`, `cohere/north-mini-code:free`, `openrouter/free` |
| 429, provider busy (routes, rate-limited upstream) | `qwen/qwen3.8-27b:free`, `google/gemma-4-31b-it:free`, `google/gemma-4-26b-a4b-it:free`, `poolside/laguna-s-2.1:free`, `poolside/laguna-xs-2.1:free` |
| upstream exhausted (200 carrying an error) | `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` |
| 403, refused | `thinkingmachines/inkling:free`, `thinkingmachines/inkling-small:free`: "only available on agentic harnesses" |

## Two defects found live, fixed

1. **An overload read as unusable output.** The router reports an
   overloaded upstream as HTTP 200 with an error object
   (`{"error":{"code":503,"metadata":{"error_type":"provider_overloaded"}}}`),
   as the whole body or as an SSE frame. `rung-std` read it as invalid
   output, a class the governor never steps down on. Arm A sat on
   `nemotron-3-ultra` for two hours with 80% of turns failed. Fixed: an
   error carrying a numeric HTTP-shaped `code` is classified as that status
   (503 → provider error → step down; 429 → rate limit).
2. **The free router could never be a rung.** `openrouter/free` lists no
   endpoints of its own (tokenizer `Router`), so the listing judged it
   `endpoint_down`. Fixed: a listed free router stands (`why: router`) and
   the keyed probe tests it.

## The arms

A ran alone. The A-fixed, B and F arms ran at the same time. B-qwen
overlapped their last 35 minutes. Platform 429s: 0 in every arm, so running
them side by side did not hit the account limit. Between A and the later
arms, two things changed on the router side: `space-bunny-alpha` lost its
expiration date, so it was listed again, and `nemotron-3-ultra` went
`endpoint_down`.

| arm | ladder | window |
|---|---|---|
| A | the ladder as built (5 named rungs), pre-fix binary | 2 h, 11:46–13:47Z |
| A-fixed | same, with fix 1 | 50 min |
| B | the same ladder + `openrouter/free` at the bottom, fix 1 only (so the bottom rung was `endpoint_down` and never reachable) | 50 min |
| F | `openrouter/free` alone | 45 min |
| B-qwen | `qwen` + `openrouter/free` at the bottom, fixes 1 and 2 (the bottom rung exercised; set against the qwen-only run) | 45 min |

| measure | qwen-only (run 3, 2 h) | A | A-fixed | B | F | B-qwen |
|---|---|---|---|---|---|---|
| models served (response `model`) | qwen | nemotron-ultra | space-bunny | space-bunny | 12 free models, 205 switches in 226 calls | 12 models; qwen served 25 of 239 calls |
| failed turns | **42%** (65/154) | 80% (180/225, all 200-carried 503s) | 0% (0/159) | 0% (0/175) | 1.4% (1/71) | **9.2%** (7/76) |
| L5 owner admission p50 / p95 | 3.4 s / **105 s** FAIL | 2.2 s / 2.3 s | 0.23 s / 0.34 s | 0.21 s / 0.92 s | 0.19 s / 4.3 s FAIL (1 deferred) | 0.21 s / **3.8 s** |
| turn p50 / p95 | 3.6 s / 95 s | 3.2 s / 108 s | 2.6 s / 53 s | 1.7 s / 23 s | 22.7 s / 104 s | 24.9 s / 108 s |
| cache efficiency (Σ cached / Σ prompt) | **0.949** | 0.679 | 0.950 | 0.971 | 0.565 | **0.319** |
| cache, same served model as the previous call / after a switch | — | — | — | — | 0.85 / 0.54 | 0.45 / 0.31 |
| ladder step-downs / probes up | 0 / 0 (one rung) | 0 / 0 (the defect) | 0 / 0 | 0 / 0 | — (one rung) | 5 / 4 (cooldown doubling: 2.5, 5, 9, 16 min) |
| tool calls / malformed JSON / unknown tool / bad arguments | 139 / 0 / 0 / 0 | 112 / 0 / 0 / 0 | 84 / 0 / 0 / 2 | 46 / 0 / 0 / 2 | 176 / 0 / 2 / 2 | 203 / 1 (cut off) / 1 / 0 |
| Jev shadow: asks, agree rate, cost | 317, 57%, $0.0212 | 460, 37%, $0.0268 | 197, 20%, $0.0110 | 289, 72%, $0.0181 | 108, 48%, $0.0072 | 92, 54%, $0.0053 |
| agent spend | $0 | $0 | $0 | $0 | $0 | $0 |
| L8 file writes outside the sandbox | not traced | 0 (traced) | 0 | 0 | 0 | 0 |

Every bad-argument call was the `expect` tool's `check` given as a string
where a variant is expected; every model made this mistake, including
space-bunny. Unknown tools (`read`, `write`) appeared only when the router
picked the model. The cut-off call came from a router-picked model. Jev's
total across these arms was $0.068, under the $0.25 cap even summed. F's
L2 FAIL is defect 2: F ran before that fix. L8 is exercised this time
(`strace` present): no write outside the workspace in any arm.

Shadow tally (agree / disagree) for the 2-hour arm A: admit 60/9, consolidate
26/29, inject 156/248, pack 33/13, tools 90/314. The per-family tallies for the
other arms are in each `shadow-audit.md`. The audit sheets hold 50 random
disagreements each. No principal disjoint from the rule has filled in a
verdict.

**Coherence when the model changes mid-session.** In F and B-qwen, nearly
every turn spans 2 to 6 served models. 12 sampled turns per arm were judged
by this run's agent, which is a model disjoint from the served ones
(`coherence-sample.md`). Persona, project and plan carried over in 22 of 24
turns. The two exceptions: in one B-qwen turn, `cohere/north-mini-code`
echoed the host's turn header back as its answer, and one turn in each arm
repeated an earlier summary. The thread carries the agent, not the model.
The style varies, but the facts and commitments held.

## Server-side fallbacks (`models` array)

`fallback-probe.py` sent 40 request pairs, alternating: `qwen` alone against
`models: [qwen, gemma-4-31b, nemotron-super]`. Each request had a fixed
2.8k-token prefix and one tool (`runs/2026-10-05-hostkey-fallback/`).

| | single | fallback |
|---|---|---|
| served | 29/40 (11 × 429) | **40/40** (qwen 29, nemotron-super 11; gemma never served) |
| cached / prompt | 0.950 | 0.692 (each fallback call is cold on the other model; qwen's cache survives the detour) |
| tool calls parsed | 28/28 | 34/34 |
| latency p50 / p95 | 1.2 s / 47 s | 1.1 s / 6.1 s |

It works with this key and the free models. It removes the 429s inside a
call. The cost: every call that falls back is cold, and the model that
served it is visible only in the response `model` field. The governor does
not see a step.

## Reading against the qwen-only run

- On the named ladder, now that the overload fix is in, failures fell to 0
  and L5 passes with a large margin (p95 0.3–0.9 s against 105 s). Cache
  held at 0.95–0.97. The credit goes to a healthy top rung (`space-bunny`)
  and the new key, not to the bottom rung, which was never reached.
- Holding qwen fixed, a free-router bottom rung cut failed turns from 42% to
  9.2% and L5 p95 from 105 s to 3.8 s. The price: cache efficiency fell from
  0.949 to 0.319, and turns got slower (p50 25 s), because a random model per
  call is cold almost every time. The two runs were made hours apart, and
  qwen was busier today (27.5% of single probe requests drew a 429).
- `openrouter/free` alone serves but is the worst on cache (0.57) and turn
  latency, and it is the only setting that produced invented tool names.

## Recommendation

1. Keep the named ladder best-first with `openrouter/free` as the **bottom**
   rung, never the top. This change is made: it is now the default free
   ladder and the live template.
2. Keep both fixes. Without fix 1 a router overload pins the host on a dead
   rung. Without fix 2 the bottom rung cannot exist.
3. Do not adopt the `models` array for now. It works, but it hides the switch
   from the ladder and pays the same cold cache. The ladder already absorbs
   429s with a recorded switch. Revisit it if the 429s inside a turn come to
   dominate turn latency.
