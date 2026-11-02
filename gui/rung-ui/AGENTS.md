# AGENTS.md — rung-ui

The rung desktop UI: a GPUI Kit window onto the instances behind a gateway.
One screen, the fleet: the instance list grouped by role (Chiefs, Stewards,
Other instances; a role with none is left out), each with its name, the host's
state word and what it is doing, refreshed every three seconds. `●` (red) marks
a host that needs you, `○` (amber) one the gateway cannot read; the header
counts both. A role is the first segment of the id (`chief-`, `steward-`), read
by `rung-client`. The id does not say which chief a steward serves and the
registry has no field for it, so stewards are grouped by role and never nested
under a chief; the real edge comes with the fleet's records, not from a guess.
Clicking an instance opens its page beside the list: Now (state, doing,
what it waits for, turn, last activity), Queue, Spend, Desk and Governor, laid
out from the host's own summary that the list already carries (no second
door, no fold here); an instance the gateway cannot read says why. **Crew and receipts, on fixtures.** The host serves no crew records yet (the
fleet's R2 settles their kinds), so there is no real source. `--crew-fixture
[FILE]` shows a Crew section on the steward the file names, from sample data
(`fixtures/crew.json`, or FILE): the ladder (spawned, assigned, active, held,
stalled, done, retired; held and stalled marked as needing someone), each
member's brief (build, complete when, delivery, autonomy), its rung's facts (what
a hold waits on, how long a stall has been silent, who judged it, whether it
landed) and its receipts (each names a record line; a repeat is one line). It is
labelled **SAMPLE DATA, not from the host** on its face and is never shown
without the flag. The file's shape is the fixture's own and claims nothing about the
`crew.*` kinds; when R2 lands the section reads the real lines instead.

**Panels, keyboard, theme.** The list and the instance page are two resizable
panels (drag the handle between them). Chords work anywhere in the window,
the message box included (all are chords, never a bare key): Alt+Down / Alt+Up
select the next / previous instance in the order the list shows (chiefs,
stewards, the rest; they stop at the ends), Ctrl+T switches light and dark
(the theme's own mode change). `step_selection` is the tested helper.

The page's Context section is the inspector: the request of one call of one
turn as stripes in the order the model sees them (tools, system, epoch, history,
message, recall, this turn), widths and cache marks (cached / partly / new) from
the host's own `context` door, plus the provider's counts and cache breakpoints
and anything the record cannot give back whole; ◀ ▶ step turns and calls; a click
on a band opens its whole text (the first 20,000 characters, then Show more / Show less; never cut; tools list their names).
Reading only; opened by a click (the latest turn) or `--turn N`. No tokenizing
or recomputing here: sizes are the host's estimates, marks the host's.

The page's Turns section is the conversation: the host's own turns (its `turns`
door, S8; nothing is folded here), newest first with each turn's reply headline
and small print, refreshed when the record says a turn began or ended; "Older
turns" pages back. A click opens a turn above the list: what it was asked (the
owner's messages with their intent, and which turn was running when they were
sent), its reply whole (drawn in part past 20,000 characters with Show more /
Show less, never cut), its tool calls (a click opens one's whole input and
result) and its facts (status, kind, model, times, failure, model use). `--open
N` opens turn N at the start.

The page also follows the instance's record live: the newest
lines (newest first, kept to 300) and the running turn's text and tools as they
arrive, from the gateway's event stream. The stream starts 100 lines behind the
newest, is resumed from the last line seen after a drop (back-off 1 s doubling to
30 s), is stopped when another instance is chosen, and says in the page when it
is reconnecting or the instance's stream ended. `--select
ID` opens with one chosen.

The page shows the instance's queue (read: refreshed every 3 s). **The window
starts read-only**: send, reorder (↑ ↓), cancel, stop and release appear only
after "Turn controls on" is clicked (or `--controls` at the start). Writes go
to the gateway through a client with no token (the owner role; the gateway
holds the write key by name per instance, `instances[].key_env`), each with a
fresh key so a retry changes nothing twice; a stop or release needs a second
click; what the gateway answers (done, or its refusal in its own words) is shown
under the buttons. A read-only connection is refused by the gateway, not by
the UI.

```text
rung-ui [--gateway URL] [--select ID] [--turn N] [--open N] [--controls] [--crew-fixture [FILE]] [--crew-open ID] [--list | --show ID]
```

Gateway: `--gateway`, else `$RUNG_GATEWAY`, else `http://127.0.0.1:8787`. A
read-only token comes from `$RUNG_GATEWAY_TOKEN` (never shown, never saved).
`--list` prints the grouped list once as text (`!` needs you, `?` unreadable) and `--show ID` one instance's page; both exit: no window, no display, the
same data the window shows.

## Shape

- `src/lib.rs`: the data the window shows (rows, link state, one `look` of the
  gateway) with no GUI in it. Everything testable lives here.
- `src/main.rs`: the window. A worker thread looks every few seconds and
  hands the result over a channel; the view never waits on the network, keeps
  the last good rows when the link drops, and says so.
- The words come from the host (through `rung-client`). Do not fold a record or
  remove secrets here; the gateway already does.

## Build and run

GPUI is heavy to build. Never on roger: use the build lane (chef or x) under
`flock /tmp/fm-heavy-<host>.lock` and
`systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=0`, `-j 4`. Cold
build on x (22 cores, capped scope): about 2m10s for `cargo build -p rung-ui`.
Needs Rust 1.92 or newer (`gui/Cargo.toml`) and, on Arch: `fontconfig`,
`libxkbcommon-x11`, `libx11`, `libxcb`, `wayland`, `clang`, `pkgconf`,
`vulkan-icd-loader` with a GPU driver (`vulkan-intel`, `nvidia-utils`, ...).
Ubuntu names are in the `gui` job of `.github/workflows/ci.yml`. Opening the
window needs a Wayland or X11 session with a working Vulkan driver; the tests
and `--list` do not.

## Tests

`( cd gui && cargo test -p rung-ui )`: the rows, the text list and `look`
against a loopback stub. The window itself is proved live (a gateway in front
of real hosts, the window, a screenshot), not in CI.
