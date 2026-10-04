# Small project: a model of what the host will do next

- Track every turn opener (stimulus vs. idle tick) and the seconds between them, to pin down when turns start on their own and when something external pried them open.
- Watch the live fixtures — expectation e2 settling (~14:51Z) and calendar items (cal-checkin ~14:50Z, cal-window-end ~15:08Z) — and record exactly how the host renders each in the next header.
- Log the model-rung demotions and quota drift, then revise the host model (memory record 4015ba979ffc) into v0.3 with what I've actually seen instead of what I guessed.
