# Anticipate Model — Initial Hypotheses (2026-10-05 14:05Z)

## Working Hypotheses about Host Behavior

1. **Stimulus Delivery Mechanism**
   - Calendar stimuli (cal-*) arrive via a dedicated channel? Possibly "calendar" or owner?
   - They are delivered as structured messages with a type field matching the calendar event name.
   - Content may be empty or contain a status flag.

2. **Timing Patterns**
   - Events appear at exact Zulu times as scheduled (e.g., 14:10:11Z).
   - There may be a small jitter (<1s) due to processing.

3. **Owner Interaction**
   - Owner messages arrive via channel "owner".
   - They are queued (send returns "queued for owner") and not delivered instantly.
   - The owner may send periodic check-ins (like the opening question) and then wait for response.

4. **Expectation Settlement**
   - Expectations are settled by the host at their due_in_s time.
   - Settlement likely produces a stimulus (maybe via a channel) indicating success/failure.

5. **Tool Gating**
   - Write and web tools are gated behind groups that become available after certain stimuli or time.

## Open Questions to Validate
- What channel carries cal-checkin, cal-firm-status, etc.?
- What is the exact format (JSON? plain text?) of those messages?
- Does cal-firm-status contain a payload? If so, what?