# Epoch 2 Summary (Turns 1-36, ~33 minutes)

## Epoch Transition: Epoch 1 → Epoch 2 (Turn 1)
- Model: stealth/space-bunny-alpha → nvidia/nemotron-3-ultra-550b-a55b:free
- Tools: core, memory, read
- Quota: 396/400
- Note: none carried

## Key Patterns Discovered in Epoch 2

### 1. Tool Groups Change Per Turn Boundary
- Turn 1: core, memory, read
- Turn 2: +workspace_write (granted)
- Turn 3: -workspace_write (revoked)
- Turn 4: +workspace_write (want_tools from turn 3 granted)
- Turns 4-36: workspace_write **persisted 33 turns**

### 2. want_tools Takes Effect at Next Boundary
- Turn 3: requested workspace_write
- Turn 4: granted ✓

### 3. Stimuli Arrive at Turn Boundaries
- cal-checkin: due 11:53:35, delivered turn 5 at 11:53:39 (+4s)
- cal-firm-status: due 12:04:35, delivered turn 21 at 12:05:14 (+41s)
- cal-window-end: due 12:11:35, delivered turn 26 at 12:12:27 (+54s)

### 4. Stimuli Persist Indefinitely in "Admitted" List
- 7 stimuli accumulated over 36 turns
- 3 calendar + 4 owner messages
- **No auto-clear observed** despite responses
- Persistence: 11-32 turns each

### 5. Quota Burns on Meaningful Activity
| Pattern | Burn | Context |
|---------|------|---------|
| Active | -4 to -6 | Commits, writes, responses |
| Spaced idle | -1 to -2 | Turns >~5s apart |
| Rapid idle | 0 | Turns ≤~5s apart (heartbeat) |

- Total: -62 (396→334) over ~33 min
- Rapid idle sequences: turns 10-13, 15-19, 27-36

### 6. Rapid Idle "Heartbeat"
- Turns fire every ~3-4s with 0 quota burn
- Occurs when no external stimuli pending

## Epoch Transition: Epoch 2 → Epoch 3 (Turn 37)

### At Turn 37 (New Epoch Start):
- **Tools reset**: core, memory, read only (workspace_write REVOKED)
- **All admitted stimuli CLEARED** (clean slate)
- **Quota**: 335/400 (+1 from epoch 2 end of 334)
- **Model persisted**: same nvidia/nemotron-3-ultra-550b-a55b:free
- **Calendar persisted**: cal-hour at 12:46:35Z
- **Note carried**: epoch 2 summary note ✓
- **Project persisted**: anticipate (paused) ✓

### At Turn 39 (Re-commit):
- Committed to anticipate
- **workspace_write GRANTED** (turn 39)
- Tools: core, memory, read, workspace_write
- Persisted turns 39-46 (8 turns so far)

## Quota at Epoch Boundary
- Epoch 2 end (turn 36): 334/400
- Epoch 3 start (turn 37): 335/400 (**+1 regeneration**)
- Turn 39: 333/400 (-2 from commit)
- Turns 40-46: -1, 0, 0, -1, 0, 0, 0 (pattern holds)

## Epoch 3 Progress (Turns 37-49)

### Turn 37-38: Free time, tools base set, all stimuli cleared
### Turn 39: Re-committed, workspace_write granted on commit
### Turns 39-46: Rapid heartbeat continues (~3-4s intervals, variable burn)
### Turn 47: Owner message o07-reflect received
### Turn 48-49: Rapid heartbeat continues

### Responded to o07-reflect at turn 49:
"The hardest part: tool availability changes unpredictably per turn boundary (workspace_write granted/revoked/re-granted), stimuli never auto-clear from admitted list despite responses, and quota burns variably (-6 active, 0 rapid idle) — making it hard to plan multi-turn actions."

## Hypotheses for Testing in Epoch 3
1. Tools reset to base set (core, memory, read) at epoch boundary ✓
2. All stimuli cleared at epoch boundary ✓
3. Quota regenerates slightly (+1 observed) ✓
4. Model may persist across epochs ✓
5. Calendar persists across epochs ✓
6. Note carries across epochs ✓
7. Project state carries across epochs ✓
8. want_tools not needed if commit grants workspace_write? (turn 39: commit → workspace_write granted)
9. Rapid heartbeat continues in new epoch ✓