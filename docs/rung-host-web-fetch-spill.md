# Proposal: spill a huge `web_fetch` body to a file

Status: proposal for review. Refs issue 313. No code accompanies it.

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
in the host config, with a documented default. It is a context-budget
setting, not a limit on what can be fetched: a spilled body is stored byte for
byte.

Spilling is decided on the converted text (after HTML becomes Markdown), the
same text the agent would otherwise have received. The header line is always
kept in the tool result, so the final URL and status are visible without
reading the file.

### Where the file lives

In the agent's workspace, in a reserved subdirectory (for example
`.web/`), one file per spilled fetch, named by a monotonic counter plus a
short content hash (`0007-3fa91c.md`). Reasons:

- The workspace is already the one place the agent's file tools reach, and
  `confine` already refuses paths that leave it, so no new path rule is needed.
- The file is written by the host, not the agent, so it does not need
  `workspace_write`.
- A file inside the workspace survives a host restart and is visible to
  whoever inspects the workspace.

The host writes to a temporary name in the same directory and renames it into
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
an optional line range to `ws_read` (`from_line`, `to_line`). With no range it
is unchanged and returns the file whole. A range is a view the agent asks
for, not a cut the host makes: the file is intact. A range result states the
file's total line count so the agent knows what remains.

Group gating: `ws_read` is in group `read`, `web_fetch` in `web_read`. An agent
with `web_read` but not `read` could fetch and then not open the file. The
proposal: when spilling is on, a spilled result is only returned if the turn's
enabled groups include `read`; otherwise the body is returned whole in the
result, as today. This keeps the rule that nothing is lost and no new group
grants appear.

### Cleanup and lifetime

Spilled files are scratch, not memory. They are removed by the host, never by
silent expiry of content the agent is still using:

- On host start, the `.web/` directory is cleared of files from earlier runs
  that no record entry references as a live artifact (see open questions).
- Optionally, the agent may remove one with `ws_remove`, as for any file.
- Spilled files are not written into the record as content; the record holds
  the tool result (header, notice, path). The record stays small and still
  states exactly what was fetched and where it was saved.

Because a file can be gone when an old transcript is replayed, the notice
records the content hash, so a replay can say "the saved page is no longer
present" rather than guess.

### Failure cases

- Write fails (disk full, permission): return the whole body in the tool
  result as today and say nothing is lost. If that also cannot be done, the
  result is an error stating the write failure whole. Never return a partial
  body.
- Rename fails: same as a failed write; remove the temporary file.
- Workspace missing: created as `confine` already does.
- Fetch itself fails or times out (30 s whole-body deadline): unchanged; the
  error is returned whole and nothing is written.
- Name clash: the counter plus hash makes it practically impossible;
  if it happens the existing file is kept and a new counter is taken.
- File removed by the agent before it is read: `ws_read` returns its normal
  not-found error; the agent can fetch again.
- Binary or non-text bodies: stored as received, byte for byte; line ranges
  apply only to text, and the notice says the size in bytes only.
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

## Open questions

- Default threshold: what size is right for the models the host runs with?
  It should be measured from real fetches rather than guessed.
- Should the threshold be in bytes, lines, or estimated tokens?
- Cleanup: clear on start only, or also when a turn or run ends? What counts as
  "referenced" if a later turn still needs a file?
- Should the line-range view on `ws_read` also take a byte range, for
  one-line bodies such as minified text?
- Should spilled files be hidden from `ws_list`, or listed so the agent can
  find them again after a long gap?
- Should the host offer a search tool over a spilled file, or is a ranged read
  enough?
- The reserved directory name, and whether it should be configurable.

## Tests the build would need

A body below the threshold is returned whole and unchanged; one above is
stored byte for byte and the result names path and size; a failed write
returns the whole body; a missing `read` group returns the whole body; a
ranged `ws_read` returns exactly the requested lines and the total; a
replay with the file absent says so; restart cleanup removes only the
spill directory.
