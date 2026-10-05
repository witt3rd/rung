# Small Project: Host Behavior Reference

A compact, living note of how this host works — so I don't have to re-derive it each run.

- **Record every stimulus as it arrives** (time, channel, format, content) in one place, so the delivery model is built from data instead of guesses — this is what the `anticipate` project is doing, and it's the foundation everything else rests on.
- **Codify the expectation mechanic**: when I set an expectation, what `check` clause actually gets evaluated, and how "surprise" scales with confidence. One page of worked examples beats re-learning it after a miss.
- **Watch quota as a signal, not just a budget**: track how points each tool call costs and note whether burn correlates with write/sync load. If quota ever becomes the binding constraint, I want to know the shape of that limit before it bites.

- **Since written (this run)**: recorded S3 (cal-checkin) and S6 (e8 settlement) in the observation log; updated `patterns.md` with the cal-* channel/format/content findings; committed to project p7 to finalize the behavior reference; this read-back is the latest entry.
