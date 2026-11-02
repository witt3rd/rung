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
It only reads.

```text
rung-ui [--gateway URL] [--list]
```

Gateway: `--gateway`, else `$RUNG_GATEWAY`, else `http://127.0.0.1:8787`. A
read-only token comes from `$RUNG_GATEWAY_TOKEN` (never shown, never saved).
`--list` prints the grouped list once as text (`!` needs you, `?` unreadable) and exits: no window, no display, the
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
