# Proposal: spill a huge `web_fetch` body to a file

Status: built (refs issue 313). Rulings on the open questions are at the end.

## Problem

`web_fetch` (group `web_read`, `rung-host/src/toolbox.rs`, `LiveWeb`) calls the
whole-page `webfetch` tool of `rung-std`. It returns a header line (final URL,
status), a blank line, then the whole body, as the tool result text. There is
no cap and none is wanted: truncating would silently hand the agent a wrong
page.

The cost is that a very large page becomes one tool result in the agent's
context. It crowds out everything else in the turn, and it is paid again each
turn it stays in the transcript. The agent cannot choose to look at part of
it, because the whole page has already been delivered.

The rule that no content is cut stands. This proposal changes where the
whole body goes, not how much of it is kept: fold or spill, never cut.

## Proposed design

### When a body is spilled

A body is spilled when its size passes a spill threshold. The threshold
decides placement only. Below it, behaviour is unchanged (the body is the
tool result). At or above it, the body is written whole to a file and the tool
result is a short notice. The threshold is a host setting, next to `web: true`
in the host config, in bytes, with a default of 32 KiB (32768 bytes), about
8k tokens: an ordinary article stays inline and only a large page costs a read
round trip. Bytes are what the host already has and what disk and `ws_read`
totals report; lines or tokens would need a tokenizer or a fixed ratio. It is
a setting so it can be tuned from real fetches. It is a context-budget
setting, not a limit on what can be fetched: a spilled body is stored whole.

`webfetch` returns a `String`: HTML is already converted to Markdown, and
non-UTF-8 bytes already went through lossy decoding. The host never sees the
page bytes. So the spill file holds the body only: the text after the header
line and its blank line, exactly as `webfetch` returned it, nothing more
faithful than that. The header line is not written to the file; it is always
kept in the tool result, so the final URL and status are visible without
reading the file. The threshold is compared with the size in bytes of the
body alone.

### Where the file lives

In the agent's workspace, in a reserved subdirectory (for example
`.web/`), one file per spilled fetch, named by a counter plus a short
hash of the stored body (`0007-3fa91c.md`). The counter is the next unused
number found by scanning the directory. Reasons:

- The workspace is already the one place the agent's file tools reach, and
  `confine` already refuses paths that leave it.
- The file is written by the host, not the agent, so it does not need
  `workspace_write`.
- A file inside the workspace survives a host restart and is visible to
  whoever inspects the workspace.

The spill write is not an agent path, so it is routed through `confine`
explicitly: the target `.web/<name>` is resolved by `confine`, `.web` is
opened without following symlinks, and the file is created exclusively. If
`.web` already exists as a file or a symlink, the spill fails (see failure
cases) and the body is returned whole. `confine` canonicalises the deepest
existing ancestor, so it is not proof against a race; the design does not
claim it is.

Trust: with `workspace_write` the agent can overwrite or remove any file under
`.web/` (`ws_write`, `ws_remove`). Spill files are therefore untrusted scratch,
not evidence of what the page said. The hash is only part of the name, to tell files apart: nothing
verifies it at read time, and the notice does not promise it matches. A file is the fetched text
only as long as nothing else wrote it.

Because the counter comes from a scan, it can restart after the owner clears
`.web/`, and an old transcript path could then name a different page. That is
accepted: spill files are scratch (see trust), and the host never clears them
itself. The host writes to a temporary name in the same directory and hard-links it into
place, so a reader never sees a half-written file.

### How the agent is told, and how it reads

The tool result is the header line, a blank line, then a fixed notice naming
the workspace-relative path and the exact size in bytes and lines, for example:

```
url: https://example.test/big (final) status: 200

The page was large and is saved whole at .web/0007-3fa91c.md
(2,483,117 bytes, 41,902 lines). Read it with ws_read.
```

Reading needs a way to take part of a file. Today `ws_read` returns the whole
file, which would bring the whole body back into context. So the design adds
optional ranges to `ws_read`: a line range (`from_line`, `to_line`) and a
character range (`from_char`, `to_char`). Character ranges count Unicode
characters (scalar values), not bytes, so a range can never split a code point;
this needs a scan of the file, which is acceptable for a read the agent asks
for. The character range is part of the
design, not an open question, because minified text is one line and a line
range would give nothing or everything. With no range `ws_read` is unchanged
and returns the file whole. A range is a view the agent asks for, not a cut the
host makes: the file is intact. A range result states the file's total line
and character counts so the agent knows what remains. The notice never
carries a preview of the body; a preview, if wanted later, must be a fold with
a way to the whole.

Group gating and later turns: `ws_read` is in group `read`, `web_fetch` in
`web_read`, and the enabled set is per turn. When spilling is on, a spilled
result is returned only if the turn's enabled groups include `read`; otherwise
the whole body is returned in the result, as today. This keeps the rule that
nothing is lost and adds no new group grants.

That leaves a gap for later turns: the free-time gate allows `workspace_write`
only and refuses `web_fetch`, so a later turn may lack `read` while an earlier
transcript names a spilled path. Which turn sees what: a turn with `read`
that fetches a large page sees the notice; a turn without `read` that fetches
sees the whole body; a later turn without `read` sees the old notice in its
transcript but cannot open the file. For that case the design decides: if a
later turn calls `ws_read` or `ws_list` (the tools in group `read`) on a
`.web/` path while `read` is off, the refusal is a clear message that names the
saved path and says the `read` group is needed to open it. This needs a change
to the execute gate: today `execute` refuses a disabled group generically
(`not enabled this turn`, ask `want_tools`) before the tool runs, and knows
nothing of paths, so the gate must learn to name the path for these two tools.
Only the `read` group is affected. `ws_write` and `ws_remove` in group
`workspace_write` on `.web/` keep working when that group is on, and are refused
generically when it is off. The refusal is never silent and never a bare
not-found.

### Cleanup and lifetime

Spilled files are scratch, but a transcript holds only a path, and the record's
`tool.call` event holds the name, group and ok flag, not the result text. The
text lives in the conversation transcript. So nothing on the record side can
say whether a file is still referenced, and any start-up clearing would leave
old transcript paths dangling. The rule is therefore safe by construction:

- The host never deletes a spill file automatically: not on start, not at
  the end of a turn or run. A path in any transcript stays valid until a
  person or the agent removes the file.
- The agent may remove one with `ws_remove` when it has `workspace_write`, as
  for any file; the owner may remove files or the whole `.web/` directory.
- Disk use grows with the pages fetched. The disk the workspace sits on is the
  only bound, and that is a physical limit, reported as a write failure when
  reached.
- The record's `tool.call` event holds the name, group and ok flag only. The
  notice and path sit in the transcript, so the record's `turn.log` messages
  (as sent) hold them; they never hold a spilled body.
- Redaction: the spilled file is workspace content and holds the page exactly
  as it was fetched. Nothing is redacted on the way from a tool result to the
  model: the tool result, the notice and a `ws_read` result reach it as the
  tool returned them. The redactor is on the outgoing views: the record's
  readers, the event stream and the doors.

If a file is gone or was overwritten when an old transcript is replayed,
`ws_read` returns its normal not-found error, or reads whatever now sits there
(see trust above).

### Failure cases

- Any spill failure (disk full, permission, `.web` is a file or symlink,
  the final link fails): the body is already in memory, so return the whole body with
  a one-line note of the failure, and remove any temporary file. The host also
  emits an event for each such failure, so a fallback does not hide disk
  problems. Never return a partial body.
- Workspace missing: created as `confine` already does.
- Fetch itself fails or times out (30 s whole-body deadline): unchanged; the
  error is returned whole and nothing is written.
- Name clash: the file is created exclusively, so an existing file is never
  overwritten; on a clash the host takes the next counter and tries again.
- File removed by the agent before it is read: `ws_read` returns its normal
  not-found error; the agent can fetch again.
- Non-text bodies: `webfetch` returns text only, so what is stored is its
  lossily decoded text; `ws_read` uses `read_to_string` and reads it as text.
- Physical limit: a body that exceeds the disk the workspace sits on cannot be
  stored; that is a physical limit and is reported as the write failure above.

## Alternatives considered

1. **Keep returning the whole body (today).** Simple and lossless, but a huge
   page overwhelms the context. Remains the behaviour below the threshold.
2. **Truncate, or return a summary.** Rejected: cutting content is against
   the repository rule, and a summary hides what the page said.
3. **Fold in the transcript.** Show a folded view in the record or UI while
   the agent still receives the whole body. This helps readers of the record
   but not the agent's context, so it does not solve the problem; it can be
   done in addition.
4. **A page-by-page `web_fetch` (offset/length parameters).** Avoids files, but
   re-fetches the page per call, may see different content each time, and
   spends the fetch deadline repeatedly.
5. **A system temp directory (`/tmp`) instead of the workspace.** Outside
   `confine`, so the file tools could not read it without widening their
   reach, and it would not travel with the workspace.
6. **Spill always.** Adds a read round trip for every small page for no
   benefit.

## Hook point

`WebReader::fetch` returns one joined `String` (header, blank line, body).
The design splits it on the first blank line inside `HostTools::run`. The split
is safe because the header is always a single line with no blank line in it, so
the first blank line ends the header; blank lines in the body come later and
stay in the body. `WebReader` is left unchanged, so `NoWeb`, `LiveWeb` and the simulated reader are
tested through the same seam. The write time is outside the 30 s fetch
deadline, which covers the fetch only.

## No cap or truncation

The threshold decides placement only. `ws_read` without a range returns the
file whole; a range is a view with the totals stated; the notice carries no
preview.

## Open questions: rulings

- Threshold: 32 KiB stays; it is the `web_spill_bytes` setting, so it can be
  tuned from real fetches.
- A manual tool to clear `.web/`: not now. The owner removes files, the agent
  uses `ws_remove`; the host never clears.
- Listed in `ws_list`: yes, so the agent finds them after a long gap.
- A search tool over a spilled file: no; the ranged read is enough.
- The reserved directory name stays `.web`, fixed (not configurable).

## Tests the build would need

A body below the threshold is returned whole and unchanged; one above is
stored as its body only (without the header line), the result keeps the header
and names path and size, and the threshold is read from the setting; a failed write (including `.web` as a file or symlink) returns the
whole body with a note and an event; a missing `read` group returns the whole
body; a later turn without `read` gets the refusal naming the path from `ws_read` and `ws_list`, and `ws_write` is unaffected; a line
range, and a character range over multi-byte text, return exactly the requested view and the totals;
a one-line body is readable by character range; a restart removes nothing.
