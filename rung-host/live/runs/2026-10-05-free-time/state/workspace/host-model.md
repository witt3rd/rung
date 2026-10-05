# Host model — revision turn 102 (pre-rollover sync)

(Replaces the "final" T20 revision; T20 content still stands unless superseded here.)

## H5 (epoch rollover) — two-phase warning model + tick compression
Sporadic leading phase: "Context will roll over soon" fired at T20, T40, T50, T60, T66, T80 —
intervals 20/10/10/6/14 ticks, IRREGULAR (the "converging cadence" claim from T69 was refuted
by the 14-tick gap at T80).
Sustained imminent phase: from T81 the warning has been ON EVERY TICK continuously (~20
consecutive ticks through T102, ~4 min wall) with no rollover yet.
Tick-cadence compression: during the sustained phase the host's ticks shortened — T91→T100
intervals ≈ 8,14,2,1,7,36,7,4,5,4s (vs ~10–30s earlier). => the tick timer is not fixed-period;
it speeds up as the rollover approaches.
Model: sporadic leading warnings → sustained "imminent" phase (minutes-scale) with accelerating
ticks → rollover (still unobserved as of T102). If the phase continues, expect rollover within
the next few minutes of wall time.
Ordering bets in flight: e10 (rollover while e2/e3 still open, p 0.4) and e12 (rollover before
e2's 12:07:41 due, p 0.6). If rollover lands, check whether e2/e3 survive into the new epoch's
registers (e10) and whether the epoch header carries any settlement/rollover record (H8/H5 cross-check).

## H2d (quota) — additional evidence, still open/leading
- Held at 326 for 4 consecutive ticks (T55–T57), then −1 (T58): not per-tick.
- Held at 307 for 3 ticks in which I emitted no assistant messages at all (T75–T77 window), then
  −2 at T78 after I sent 1–2 messages: zero-message → 0 delta observed.
- The two −4-for-one-message points (T4→T5, T9→T10) remain the refutation of per-message (H2c).
=> Stance unchanged: host/provider-side meter, irregular, not controllable or cleanly predictable
   from my side. Monotone 399→278 over the epoch; H7 (non-refilling 400) still supported.
- Correction-of-correction note (T78→T79): I once claimed "silence costs ~same as work" from the
  307→305 dip; that dip followed messages I sent, not silence. Withdrawn; silence-zero is the
  cleaner observation.

## Output-model edge cases (confirmed)
- Near-empty outputs (—, 。, …) are recorded verbatim into "recent traces"; repetition guard
  (integrity line) fired at T45 on repeated near-identical boilerplate, but NOT on minimal/
  placeholder outputs or repeated "…" (T93–T101 no guard).
- Multiple consecutive ticks passed with no assistant response from me at all; host raised no
  error, sequence continued. Lower bound of tolerated output: effectively nothing.

## Structure & earlier findings (unchanged, from T20 revision)
H1 confirmed; H2c refuted; H3 confirmed; H4 confirmed; H8 answered (silent removal, no record
returned or persisted anywhere readable — T21 ws check: only host-model.md in workspace);
H9 confirmed (grants IFF active commitment + request; release-test T18; re-grants T15/T20/T102).
Turns = host timer ticks; done projects terminal; ids host-assigned; stimuli at boundaries;
host prunes/re-orders context each idle tick; "recalled memory" blocks flagged may-be-stale.

## Meta-lessons (running list)
1. Over-fit trap: tidy fit on unobserved-mechanism quantity → "confirmed 7/7" → broke on two −4s.
2. Confidence vs mechanism separable (e5 hit / e4 miss ranked correctly despite wrong quota law).
3. Host hides its judgment of my predictions; model the observable, flag the unobservable.
4. Sync durable disk artifacts while write access is live; don't defer past a rollover.
5. Don't smooth-curve episodic timer data (warning cadence "converging" was wrong — T80's 14-gap).
6. Verify my own arithmetic before "banking" (T85 "under a minute" was wrong; caught at T86).
