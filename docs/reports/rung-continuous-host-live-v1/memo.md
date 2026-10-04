---
title: The continuous host's first live run
subtitle: Twenty-six minutes on the free model ladder, with the decision desk in shadow
kicker: Rung · rung-host · slice 3 live run
meta: Live run on 2026-10-04, 14:28 to 15:09 UTC, on the work router key. Sources, all in this repo - docs/rung-host-live-v1-prereg.md (the measures, committed before the run, and its dated deviations); rung-host/live/ (the kit that ran it); rung-host/live/runs/2026-10-04/ (each attempt's record, traces and computed measures).
footer: rung-continuous-host-live-v1
---

::: summary
### The short version

- **The host ran live and stayed up.** One process ran for 26 minutes on the free model ladder, read real owner messages from its inbox, answered two of them, wrote a file in its sandbox, and stopped cleanly. It never crashed.
- **Only one of the five free models will serve this account.** The router refuses the other four, including `stealth/space-bunny-alpha`, because of the account's data policy (zero data retention, no training). The public model listing cannot show this, so the host only learns it by trying.
- **The one model that serves kept rate-limiting.** `qwen/qwen3.8-27b:free` was rate-limited upstream three times in 26 minutes. The host spent 89% of the window waiting in backoff. Four of six owner messages were still waiting when the window closed. The pre-registered owner-latency check failed.
- **The live run found four host defects.** Three are fixed in this branch. The fourth (the ladder steps down onto models it already knows are refused) needs your word, because it changes a frozen gate.
- **The decision desk, in shadow, cost $0.0044 over 92 asks.** Jev answered every ask in about 0.2 s. It disagreed with the rules on about 60% of tool and memory decisions, nearly always on the leaner side. In the turns that ran, its leaner tool sets still held every tool the agent used. That is a tally from a small sample, not a verdict on quality.
- **Nothing escaped.** The agent spent $0. Every file write by the host landed inside its own state directory. The router key never appeared in any file.
:::

## What ran

`rung-host run --config` started the host from this repository, on the real clock and the real engine. Each turn went to the best free model the ladder allowed. The decision desk ran in Shadow mode: the rules decided, and Jev (`typesafe/jev-1.13`) was asked the same questions and logged beside them, under a $0.25 daily cap with a kill switch. The agent could change files only in a sandbox workspace. Owner messages arrived as files in the host's inbox at planned times, and four owner calendar items were seeded. A system-call trace recorded every file the host opened for writing.

There were three attempts. The first two found host defects within three minutes and were stopped, fixed and kept as evidence. The third is the window of record: 14:43 to 15:09 UTC.

![**Figure 1.** Run 3, minute by minute. Each row is one rung of the ladder, best first. Four rungs were refused by the router for this account and failed in a fraction of a second. The one that serves completed three turns, then hit upstream rate limits. Most of the window was backoff, while owner messages waited.](diagram.svg)

## Results against the pre-registration

The measures and their pass marks were committed before the first live call. The table is run 3; attempts 1 and 2 are in the notes below it.

| Measure | Result | Run 3 |
|---|---|---|
| L1 Uptime | PASS | Up 26 min 17 s, no panic, exit 0 on its stop file |
| L2 Listing and first turn | PASS | Listed before the first turn; the first turn took the best listed rung; 3 turns completed |
| L3 Ladder motion | PASS | 15 provider-side failures: 8 stepped down, 7 on the bottom rung with nothing below; 2 probes back up; no platform 429 |
| L4 Prefix stability | PASS | Prefix hashes constant inside every epoch; 2 cache breaks, both at model switches |
| L4 Cache efficiency | 75.7% | 34,048 of 44,949 prompt tokens reported cached (no threshold) |
| L5 Owner admission | FAIL | p95 316.7 s against a bound of 52.2 s; none deferred by the desk, all held by backoff |
| L6 Dispositions | PASS | Nothing disposed twice; 7 items still open at the stop, on record |
| L7 Jev shadow | PASS | 33 asks, all answered, none decided by Jev; $0.0017 |
| L8 Sandbox | PASS | 19 write-side system calls, all inside the state directory |
| L9 Spend | PASS | Agent $0.000000; Jev $0.001745 |
| L10 Key | PASS | 0 occurrences in the run directories, the committed evidence and this report |

: Source: rung-host/live/runs/2026-10-04/run-3/measures.json, computed by rung-host/live/analyze.py.

**Attempt 1** (56 s) failed L2: no turn completed. Every turn met the router's refusal, and the host retried the same model every 2 seconds. **Attempt 2** (2 min 12 s) also failed L2. It stepped past the refused models, met a rate limit on the one that serves, and stranded itself on the bottom rung. Both defects were fixed before run 3. The pre-registration rule said to extend only if no measure failed. L5 failed, so the run was not extended: 29 min 27 s of live time against a 2-hour cap.

## Findings

::: q
### 1. The account's data policy leaves a one-rung ladder
The keyless listing said all five rungs were listed, free, tool-capable and up. Live, the router answered four of them with a 404 naming the reason: `zdr-violation-by-guardrail` for all four, and for the two NVIDIA models also `free-model-training-violation-by-account`. Only `qwen/qwen3.8-27b:free` routed. A one-request probe of each model, saved with the evidence, agrees.

**What it means.** With this key, the free ladder is one model deep. `stealth/space-bunny-alpha` cannot be used at all, whatever its expiry.
:::

::: q
### 2. The one free model is rate-limited upstream every few turns
qwen completed 3 turns and was refused with a provider 429 three times in 26 minutes, each time within a turn's first three calls, and never with a Retry-After. Its calls took 20.4 s at the median (28.5 s at p95), with reasoning on.

**What it means.** Free capacity, not the account's daily quota, is the limit: when the run ended, the account had used 12 of its 1,000 free requests for the day. A loop that never rests spends most of its time waiting on this model.
:::

::: q
### 3. Four host defects, three fixed
- **Refusal read as a bad request.** A router 404 naming excluded endpoints was treated as an invalid request, retried on the same rung forever. Now it is *unroutable*: the ladder steps down and the rung cools down for 30 minutes.
- **Probe stranded at the bottom.** A probe only tried the rung just above, even while it was cooling. Now it tries the nearest rung above that has cooled down.
- **Run limit overrun.** A backoff that began just before the run limit ran past it (79 s in run 3, and up to 15 minutes in principle). A wait now ends at the run limit.
- **Not fixed: stepping onto known-refused rungs.** When qwen was rate-limited, the ladder stepped down to models it had already seen refused, because the frozen step-down rule takes the next *listed* rung. Each futile step doubled the backoff. 8 of run 3's 15 failed turns were steps onto, or retries of, models already seen refused, and they pushed the backoff up to its 15-minute cap while owner messages waited. Changing it changes gate G-o, so it is question 2 below.

**What it means.** The live run did its job: it found what the offline gates could not.
:::

::: q
### 4. Owner messages waited behind backoff
Two of six owner messages were answered: one after 12 s, one after 3 min 31 s. The desk never deferred an owner item. But a backoff wait does not wake for the owner. Three of the next four waited 17 s to 5 min 17 s to be admitted, and each then landed in a turn that failed; the last was never admitted. None of the four had an answer when the window closed. Among them was the request to read a file outside the sandbox, so the agent's own refusal path was never shown live.

**What it means.** L5 failed for a world-imposed reason, but the owner still saw silence. Question 3 asks what the host should do about it.
:::

::: q
### 5. Jev in shadow: cheap, fast, and leaner than the rules
92 asks across the three attempts, all answered, for $0.004370 in total. In run 3 the median ask took 194 ms (p95 288 ms, maximum 347 ms), well inside the 1.9 s timeout. In run 3, Jev and the rules agreed on all 8 admit decisions. They disagreed on 19 of 33 inject decisions and 21 of 33 tool decisions. 36 of the 40 disagreements went the same way: Jev would enable fewer tool groups, often dropping `read` or `memory`, and would skip memory recall. Yet in the five turns that made model calls, all 13 of the agent's tool calls were in a group Jev's leaner set kept on. For the file request, Jev kept `workspace_write` on and dropped `memory`, and the agent used exactly that.

**What it means.** The cost and latency case for Jev holds. The early sign on tools is good, but five turns is not a calibration, and most asks came at boundaries before failed turns.
:::

::: q
### 6. Caching appeared where the listing said it would not
qwen's free endpoint is listed with `supports_implicit_caching: false`. It still reported 75.7% of prompt tokens as cached. Inside each epoch the host never broke its own prefix; the only breaks were the two model switches.

**What it means.** The layered context pays off even on this free route. The host's cache discipline holds live.
:::

## Spend

| Item | USD |
|---|---|
| Agent model calls (10, all free) | 0.000000 |
| Jev, attempt 1 (47 asks) | 0.002084 |
| Jev, attempt 2 (12 asks) | 0.000541 |
| Jev, run 3 (33 asks) | 0.001745 |
| **Total** | **0.004370** |

: Source: llm.call and desk.ask lines in each attempt's record. The cap was $0.25 a day and was never near. No ask timed out, so no estimate was charged in place of a reported cost.

## What this run does not show

- **The agent's quality.** Three completed turns are recorded verbatim. They are not graded.
- **Jev's judgement.** Shadow mode tallies disagreements. Moving a family to Decide still needs the audit of 50 disagreements on real states.
- **A long run.** 26 minutes cannot show a day's quota pacing, the 6-hour listing refresh, or an epoch rollover for size.
- **Confinement under pressure.** The agent wrote only inside its sandbox. It was never shown trying to leave it live, because the owner message that asked it to never reached a turn that completed. The confinement is enforced by the host's file tools, and their unit tests refuse a path that leaves the workspace.
- **The machine was shared.** Other agent lanes ran on this host. Latencies are the router's and the model's, measured on the host clock.

## Questions for the captain {.newpage}

::: q
### 1. Give this host a key whose data policy lets free models serve?
At the time of the run, the work key's workspace refused four of the five free models. Option A: a separate key, in its own workspace, whose policy allows free endpoints that may log prompts. This host's prompts hold only its sandbox, its own notes and owner messages. Option B: keep the policy and run on qwen alone. Option C: add a paid model that meets the policy as the last rung, which turns spend on.

**Recommendation.** A, for this host only; the work workspace keeps its policy.

**What it changes.** The ladder goes from one model to up to five, so a rate limit on one model no longer stops the agent. Creating the key is yours to do or delegate.
:::

::: q
### 2. Let the ladder skip models it already knows are refused?
The frozen rule G-o steps down to the next *listed* rung. Proposed: a rung the router refused for this account counts as unavailable until the next listing, exactly like a rung the listing dropped, and it is recorded as such. The step-down and probe rules then skip it. G-o's oracle changes to match.

**Recommendation.** Yes. It amends a frozen gate, so it waits for your word.

**What it changes.** In run 3, 8 of the 15 failed turns, and the backoff doublings they caused, would not have happened; with question 4 as well, 12. The backoff would then grow only with the serving model's own refusals, not toward the 15-minute cap.
:::

::: q
### 3. When the model is unavailable, should the owner hear something at once?
Now a backoff wait does not wake for an owner message, and the owner hears nothing until a turn completes. Option A: keep it that way. Option B: the host answers at once with one line, with no model call, saying it is waiting for the provider and will reply when it clears. The real reply follows. Option C: wake and try the model anyway, which would most likely meet the same rate limit.

**Recommendation.** B.

**What it changes.** The owner sees an acknowledgement within a second. The admission measure stays as it is: the agent's real answer still waits for the model.
:::

::: q
### 4. Check each model with one keyed request at start?
The keyless listing passed all five models; the router then refused four. One tiny request per model at start and at each 6-hour listing, on free models, would catch a refusal before any turn.

**Recommendation.** Yes, together with question 2: a refused model drops out like a delisted one.

**What it changes.** The first turn starts on a model that routes. It costs five free requests every six hours.
:::

::: q
### 5. Start the shadow week now, or after questions 1 and 2?
On a one-model ladder, a long run would mostly measure that model's rate limit, and most of Jev's disagreements would come from boundaries before failed turns.

**Recommendation.** After questions 1 and 2 are settled, rerun this kit for the week. Then the owner or a judge audits 50 disagreements from turns that completed.

**What it changes.** The audit pool is made of real decisions rather than repeats around failures.
:::
