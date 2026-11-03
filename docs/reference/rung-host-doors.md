# rung-host API — the doors a UI or gateway uses

The host API is in five files; section codes (H1, S4, …) are the build slices
and are kept so that code comments and records still find their section:

| file | sections |
|---|---|
| this file | H1 state lock, H2 instance registry, H3 read doors, H5 event stream, S4 request record and context door, S8 turns door |
| [`rung-host-queue.md`](rung-host-queue.md) | S1 queue list and add, S3 reorder and cancel, S2 intent, S7 stop and release |
| [`rung-gateway.md`](rung-gateway.md) | H9 the gateway: config, roles, routes |
| [`rung-redaction.md`](rung-redaction.md) | H4 the credential redactor |

This is the written shape of what the host offers to something
that is not the agent: a command, files and HTTP doors.
It is the contract between the host and the multi-instance UI
(`docs/reports/rung-ui-design-v1/`). Each section is kept stable once written: changes inside version 1 only add; a
removed or renamed field needs version 2.

Rules that hold for every door:

- **API-first.** Nothing is possible only through the UI.
- **Nothing is capped or cut.** A list is paged (`offset`, `limit`, `total`,
  `next`; `limit` has no maximum; `total` is exact), a stream is followed,
  a long line is stored whole.
- **The record stays the one truth.** A door reads it or asks the host to
  write to it; it never writes it.

## State lock (H1)

A state directory has one live host.

- **The file.** `<state>/host.lock`, taken with an exclusive advisory lock
  (`flock`, non-blocking) before the host writes anything else into the state
  (the record, the memory store), and held for the host's whole life. The
  file holds the holder's pid as one decimal line. It is never removed.
- **A crash frees it.** The kernel drops the lock when the holder dies, by
  any means, so a killed host never wedges the directory. A leftover file
  whose pid is dead is not a lock: only a live holder refuses a start.
  The descriptor is close-on-exec, so a child process cannot keep the
  directory locked after the host is gone.
- **The refusal.** A second host started on a held state (`rung-host sim` or
  `rung-host run`) exits with the bad-start code, **2**, and prints one line
  to stderr:

  ```text
  rung-host: state directory <dir> is in use by a live host (pid <pid>); one host per state
  ```

  (`pid unknown` replaces `pid <pid>` only if the holder had locked but not
  yet written its pid for more than a second.) The refused start writes
  nothing into the record or memory.
- **In the library.** `rung_host::statelock::StateLock::acquire(dir)` returns
  the hold or `LockError::Held { dir, pid }`; `Host::open` takes it when the
  builder has none, so any embedding gets the same guarantee. The lock is
  held by the `Host` and released when it is dropped.

## H2 — the instance registry and `rung-host ls`

Which hosts exist on this machine, and whether each still lives.

### Where

The registry folder is `$RUNG_HOME/instances/` (`~/.rung/instances/` when
`RUNG_HOME` is not set). For an instance with id `ID` it holds:

| file | what |
|---|---|
| `ID.json` | the entry: who the instance is. Written when the host starts; **never removed**, so a stopped or dead instance stays listed. |

(`.registry.lock` in the same folder only serializes changes to the folder;
it says nothing about any instance.) Liveness is the **state directory's
lock** (`STATE_DIR/host.lock`, the H1 section above), not a file here.

### Who registers

- `rung-host run --config FILE` always registers. The name is `name:` in the
  file; without it, the last component of `state:`.
- `rung-host sim` (the test harness) registers only when given `--name NAME`.
- A host registers only **while it holds its state directory's lock**, so a
  start the lock refuses registers nothing, and two starts can never
  register one state. For `run` and for `sim` it happens right after the lock is
  taken and **before anything else is created** (the memory store, the
  inbox, the record). A
  refusal exits with the bad-start code `2` and names the entry it collides
  with:
  - the id is registered for **another state directory** (the message says
    whether that host is running, with its pid, or not running);
  - this state directory is registered under **another id**.
  A restart (same id, same state) renews the entry. Nothing is ever
  replaced behind the operator's back. The check against the other entries
  and the write of this one are one step under the folder lock, so two
  starts for different states cannot both take one id.
- The id is the name as a slug: ASCII letters and digits lowercased, every
  other run of characters one `-`, none at the ends (`Deep Thought` →
  `deep-thought`). A name with no letter or digit is refused.
- **A fleet names its members by role** (a convention, not a rule the
  registry checks): `chief` or `steward`, then `-` and the session or project,
  as in `chief-main` and `steward-alpha`. The role is the id's first segment;
  the entry has no role field. See
  [rung-fleet-props.md](../rung-fleet-props.md#a-member-is-named-by-its-role).
- A host that serves HTTP (`acp.http:` / `--acp-http`) may ask for port `0`.
  It then writes the **real, bound address** into `address` as soon as the
  listener is up.

### The entry (`ID.json`, schema 1)

```json
{
  "schema": 1,
  "id": "deep-thought",
  "name": "Deep Thought",
  "state_dir": "/srv/rung/deep-thought/state",
  "config": "/srv/rung/deep-thought/rung-host.yaml",
  "workspace": "/srv/rung/deep-thought/workspace",
  "address": "127.0.0.1:41873",
  "pid": 20481,
  "started": 1791661135383,
  "version": "0.2.3"
}
```

| field | type | meaning |
|---|---|---|
| `schema` | number | entry format version, `1` |
| `id` | string | the slug; also the file's name |
| `name` | string | the name as the operator wrote it |
| `state_dir` | string | absolute; holds `record/`, the inbox, the memory |
| `config` | string or null | absolute path of the config file; null for `sim` |
| `workspace` | string | absolute |
| `address` | string or null | `host:port` the HTTP listener is bound to; null until bound, or when the host serves none |
| `pid` | number | the process that last registered |
| `started` | number | wall time of that start, ms since the epoch |
| `version` | string | the host's version |

The entry is written by a temporary file and a rename, so a reader sees the
old entry or the new one, never half of one.

### Is it alive: the state word

An entry never says whether its host lives; a dead host cannot say anything.
A reader derives the **state word** from the lock and the record:

1. Open `STATE_DIR/host.lock` and take a shared, non-blocking `flock`. If
   it **fails because the lock is held** the host lives: **`running`**.
   Otherwise release it at once. (A missing lock file means no host ever
   held it.)
2. The lock is free. Read the **last line** of `STATE_DIR/record/` (the
   newest segment's last complete line; a torn tail is ignored). If its
   `kind` is `halted` the host stopped itself: **`stopped`**. Anything else
   (or no line): it died: **`down`**.
3. An entry file that cannot be parsed, or a lock that cannot be probed, is
   **`unreadable`**, listed with its `problem`. So is an entry whose `id` is
   not a registry id (not exactly what the slug rule makes: `../x`, a
   separator, a NUL, spaces, empty) or is not the entry's own file name: ids
   are joined into paths, so such a file is refused and its other fields are
   never trusted. It is still a row (named by its file), not dropped.

| word | when |
|---|---|
| `running` | the lock is held |
| `stopped` | lock free; the record's last line is `halted` |
| `down` | lock free; the last line is not a halt (killed, crashed, power cut) |
| `unreadable` | the entry or its lock cannot be read |

The kernel drops the lock when the process ends, however it ends, so a
crash cannot leave a false `running`. `running` is refined (working,
answering, free time, waiting, stuck) from the record by the summary door;
the three words above are all the registry can say.

A dead or stopped instance is **never dropped** from a listing.

### `rung-host ls [--json]`

Lists every entry in the registry folder, ordered by id, with its word. It
reads only; it starts nothing. Exit `0` (a down instance is a row, not an
error); `2` when the folder cannot be read. An absent folder lists nothing.

Text, one row per instance; a problem sits on its own line under its row:

```text
ID            STATE    PID    ADDRESS          LAST            STATE DIR
alpha         running  20481  127.0.0.1:41873  turn.ended #212 /srv/rung/alpha/state
beta          down     -      -                turn.started #7 /srv/rung/beta/state
gamma         stopped  -      -                halted #31      /srv/rung/gamma/state
```

`PID` is shown only for `running`. `LAST` is the record's last line, kind
and number.

`--json` prints one object, for the gateway and scripts:

```json
{"instances": [
  {"schema": 1, "id": "alpha", "name": "alpha", "state_dir": "…", "config": null,
   "workspace": "…", "address": "127.0.0.1:41873", "pid": 20481,
   "started": 1791661135383, "version": "0.2.3",
   "word": "running",
   "last": {"seq": 212, "at": 1791661189022, "kind": "turn.ended"},
   "problem": null}
]}
```

Every entry field above, then:

| field | type | meaning |
|---|---|---|
| `word` | string | `running`, `stopped`, `down` or `unreadable` |
| `last` | object or null | the record's last line: `seq`, `at` (ms), `kind`; null when there is no record |
| `idle_until` | integer or null | ms: when a running host that is resting (idle mode) wakes on its own, read from the last 64 KiB of its record by the rule the summary's fold uses (an `idle` line, not yet followed by the line that ends the rest, a boundary, a halt or a start); null for a host that is not resting and for one that is not running. `rung-host ls` shows `idle until <time>` in its table for such a row; its `word` stays `running` |
| `problem` | string or null | why something could not be read (an unparseable entry, an unreadable record); null otherwise |

An `unreadable` entry carries only `id` (the file's name), `word`, `last`
(null), `idle_until` (null) and `problem`.

A `down` host's `pid` is the pid that **was** its process; `ls` shows it as
`-` and the JSON still carries it. Do not signal it: the number may have
been reused.

## H3 — the read doors and folds

Three read-only doors over the native instances of a registry folder
(`registry_dir` in the gateway config, the H2 folder): the list, one
instance's **fold**, and its recorded events, paged. They are computed from
the instance's own files, never ask a host anything and never write: a stopped
instance reads the same as a running one, and the state directory is byte for
byte what it was (the one probe is H2's, a shared non-blocking `flock` on
`host.lock`, released at once). Code: `rung_host::read` (the folds and paging,
no HTTP) and the gateway's routes.

### Who, and what is refused first

They are the gateway's doors, so the gateway's rules come first and are not
repeated here: both roles may read (owner with no token; the read-only role
with `Authorization: Bearer <token>` or `?token=`); an unknown token is 401
`unauthorized`; an owner-role request is Host-checked (403 `bad_host`); a write
by the read-only role is 403 `read_only`; any method but GET or HEAD on these
doors is 405 `method_not_allowed` for the owner.

**Every outgoing text is redacted** with the shared redactor (H4) before it is
written, and that includes every error message, every name, id and path in an
answer, and the `registry_error`: a key in a directory name, an entry's name, an
invalid entry's file name or a query value never comes back. The secrets it
removes: the gateway's own (its read-only tokens and instance keys); the value
of every variable that **any** registered instance's config file names (`*_env`
keys, `acp.tokens`), because a host can have been shown another's key, not only
its own; and every known key shape. The record on disk stays verbatim. A
replaced value is `[redacted]`; nothing is shortened otherwise.

**The gateway must hold the variables.** A value is found by value only if the
gateway process has the variable the config names, so run the gateway with the
same environment source as the hosts. A name it cannot read is listed, never
its value, in every summary and record answer as
`redaction.env_unset` (a sorted list of names across the registry; `[]` when
every named variable is held). For such a variable nothing can be matched by
value: a copy of it in a record is removed only if it has a known key shape or
sits under a secret-looking name, so treat a non-empty `env_unset` as a
configuration fault to fix, not as a safe state.

Errors are the gateway's `{"error": <code>, "message": <text>}`:

| status | `error` | when |
|---|---|---|
| 400 | `bad_request` | a query that is not as below |
| 404 | `no_such_instance` | no entry in the registry folder has that id |
| 404 | `state_dir_missing` | the entry's state directory is gone |
| 404 | `not_found` | a door of a later slice (`config`, `stop`, …) on a native instance that the gateway config does not also name (the registry holds no address or key). If the config names the same id, those doors pass through to its `url` with its key, and only `summary`, `record` and `queue` are answered from the state directory (S1) |
| 500 | `unreadable` | the entry file, the state directory or a record file cannot be read (permissions, not a directory, an unparseable entry) |

A record that is **damaged** is not an error: it is read as far as it is
intact (see `damaged` below). A record directory that is not there yet is an
empty record, not an error.

### `GET /api/instances` — the list

The gateway's overview, with the registry's instances added. Every row, native
or configured, has `id`, `name`, `source`, `reachable`, `summary`, `error`; one
id is one row (an id the registry lists is the registry's row, even when the
config names it too). Top level: `instances`, and `registry_error` (null, or the
reason the registry folder could not be listed; the configured instances are
still listed).

A native row (`source: "registry"`) is the H2 row (`rung-host ls --json`: the
entry's fields, `word`, `last`, `idle_until`, `problem`) plus:

| field | type | meaning |
|---|---|---|
| `name` | string | the name as written (the file name for an unreadable entry) |
| `source` | string | `registry` (`config` for an instance the gateway config names) |
| `reachable` | bool | false only for an `unreadable` entry |
| `last_at` | number or null | wall time (ms) of the record's last line: the last event |
| `start_command` | string or null | `rung-host run --config <file>` (the config path shell-quoted) when the entry has a config file; null for a `sim` host. Showing it is the page's job; nothing here starts a host |
| `summary` | object or null | the fold, as `GET /v1/summary` below; null when it could not be read, with `error` the reason (and `problem` carrying it too if the row had none) |
| `error` | string or null | why `summary` is null |

`word` is the registry's: `running` (the state lock is held), `stopped` (free,
last line `halted`), `down` (free, any other last line), `unreadable`. No
instance is ever dropped from the list. The list folds every instance's whole
record, so its cost grows with the records; the event stream (H5) is the way to
follow one live.

### `GET /api/i/{id}/v1/summary` — the fold

No parameters (400 if one is given). The record replayed through the host's own
state fold (`State::apply`, the code the live host runs), plus what the lines
say. The fold of a recorded run equals what the live host computed at each turn
end: every `turn.ended` line carries the hash of the state replayed up to it,
and `fold_hash` is the same hash over the whole record.

```json
{"contract": 1, "id": "alpha", "name": "Alpha", "word": "running",
 "state": "Working", "doing": "Committed to notes: Tidy the notes index",
 "needs_you": false, "turn": 2, "last_seq": 9, "last_at": 1791208194000,
 "requests": 1, "quota": 400, "spend_usd_day": 0.0125, "day": "2026-10-05",
 "start_command": "rung-host run --config /srv/alpha/rung-host.yaml",
 "waiting_for": {"kind": "turn", "turn": 2, "since": 1791208193000},
 "boundary": {"n": 2, "at": 1791208192000, "mode": "committed:notes", "pending": 2},
 "turn_in_progress": {"turn": 2, "turn_kind": "committed", "started_at": 1791208193000,
                      "model": "model-a", "project": "notes", "mode": "committed:notes"},
 "queue": {"waiting": 2, "in_flight": 0,
           "head": {"id": "m-1", "kind": "peer", "role": "owner", "channel": "owner",
                    "at": 1791208190000, "firm": false, "text": "please look at the build", "order": 5}},
 "desk": {"backend": "jev", "mode": "shadow", "spent_today_usd": 0.0,
          "interrupts_last_hour": 0, "enabled_groups": ["core"],
          "limits": {"cap_usd_day": 0.5, "cap_usd_ask": 0.002, "max_interrupts_per_hour": 6,
                     "max_deferral": {"peer": 1800000}, "pack_ceiling": 0.85, "...": "every knob"}},
 "governor": {"rung": 0, "model": "model-a", "failures": 0, "blocked": false,
              "ladder": [{"rung": 0, "model": "model-a", "available": true}]},
 "config": {"file": {"path": "/srv/alpha/rung-host.yaml", "content": {"...": "the YAML as read"}, "problem": null},
            "started_with": {"engine": "agent", "ladder": ["model-a"], "...": "the latest host.start's config"}},
 "fold_hash": "…", "damaged": null, "problem": null}
```

| field | meaning |
|---|---|
| `contract` | `1`; the UI contract's summary fields (`state`, `doing`, `needs_you`, `turn`, `last_seq`, `last_at`, `requests`, `quota`, `spend_usd_day`) are all here |
| `word` | the registry's word (above) |
| `state` | `Working`, `Answering`, `Free time`, `Waiting`, `Stuck` (the lock is held), or `Down`, `Stopped`, `Unreadable` (from `word`). Idle mode adds `Idle until <time>` (the time as `YYYY-MM-DDTHH:MM:SSZ`): a sleeping host, distinct from degraded `Waiting`; it is not `needs_you`, and `waiting_for` names the scheduled wake. Order of the held ones: **Stuck** when the governor is blocked (a refused key) or the copy guard has seen `COPY_LOOP_TURNS` copying turns in a row; **Idle until** when the fold's last rest line (an `idle` line) has not been followed by the line that ends the rest, a boundary, a halt or a start (a resting host may still write a line or two, a channel opened, a calendar entry added, without waking); **Waiting** when a degraded wait is open (`paced`, `quota`, `backoff`); **Answering** when an item is in a turn or the turn running is `responding`; **Working** when committed to a project; else **Free time**. Thresholds that need a clock (a turn past its bound, a missed watchdog, no progress for a set number of turns) are not derived here |
| `doing` | one line: the commitment, `Free time`, or why it is not running |
| `needs_you` | true for `Stuck` and `Down` (what the record can say; an unread message to the owner is not recorded) |
| `turn` | the last turn started, or null |
| `last_seq`, `last_at` | the last intact line's number and time (ms); `0` and null for an empty record |
| `requests`, `spend_usd_day` | the llm calls and their paid cost for the UTC `day` of the last line (counters are those of that day, not of the day the page is read); `quota` is the config's requests-a-day or null |
| `idle_until` | integer or null | ms: the scheduled wake, while the state is `Idle until <time>`; null otherwise |
| `waiting_for` | what it waits for: `idle` (`until`: the scheduled wake), `degraded` (`class`, `until`, `why`), `turn` (running now), `boundary` (items waiting for the next one), `calendar` (the next entry due), or null |
| `boundary` | the last `boundary` line: `n`, `at`, `mode`, `pending` (items waiting then), or null |
| `turn_in_progress` | the turn started and not ended (cleared by `recovered`), or null |
| `queue` | `waiting` and `in_flight` counts, and `head`: the oldest waiting item whole, with `order` (the number of its `stimulus.accepted` line), or null. The full list is H6's door |
| `desk` | the decision desk: `backend`, `mode` (from the latest `host.start`), `spent_today_usd`, `interrupts_last_hour`, `enabled_groups`, and `limits`: the code-owned thresholds (every `Knobs` field) plus `cap_usd_day`, `cap_usd_ask` (the config file's `desk:` values, else the defaults) |
| `governor` | the ladder rung in use, its model, failures, `blocked`, and the ladder with each rung's availability |
| `config` | the config **as read** (nothing here writes it): `file` is the instance's config file parsed as YAML (`content`, null with `problem` when it cannot be read or parsed; all null for a `sim` host), `started_with` is the config the latest `host.start` line recorded |
| `fold_hash` | `State::hash` of the fold |
| `redaction` | `{"env_unset": [names]}`: the config-named variables the gateway does not hold (see above) |
| `damaged`, `problem` | null for an intact record; see below |

### `GET /api/i/{id}/v1/record` — the recorded events, paged

The record's lines, as the host wrote them (`seq`, `at`, `kind` and the body),
oldest first, **paged by the caller**. The same paging rule as every list
(`offset`, `limit`, `total`, `next`), plus a line-number cursor.

| query | meaning |
|---|---|
| `limit` | lines in the page, a whole number ≥ 1; default 500 (a default, not a bound). **No maximum**: a number too big for 64 bits is the biggest one |
| `offset` | lines to skip, default 0 |
| `after` | the cursor: the page starts after this line number (`after=N` is `offset=N`: line numbers count from 1 with no gap). Not with `offset`; oldest first only |
| `order` | `asc` (default) or `desc`; with `desc` the offset counts from the newest line and the page is newest first |

A name not in the table, a repeated name, a value that is not a whole number,
`limit=0`, `offset` with `after`, or `after` with `order=desc` is 400
`bad_request`.

```json
{"lines": [{"seq": 11, "at": 1791208196000, "kind": "note.written", "…": "…"}],
 "offset": 10, "limit": 10, "total": 25, "next": 20, "cursor": 20,
 "order": "asc", "after": null, "damaged": null, "redaction": {"env_unset": []}}
```

| field | meaning |
|---|---|
| `lines` | the page: whole lines, none cut, however long |
| `offset`, `limit` | as asked (`offset` is `after` when that was used) |
| `total` | the exact number of intact lines |
| `next` | the next `offset`, or null on the last page |
| `cursor` | the `after` that continues from this page (the last line's number; the requested position for an empty page); `null` for `desc`. A poller keeps it and asks again |
| `order`, `after` | as asked |
| `damaged` | null, or where the record stops being intact (below) |
| `redaction` | as in the summary |

**The page is streamed.** There is no cap on `limit`, so a page can be the whole
record; the answer is written a line at a time (each line redacted and sent
before the next is read, to a client that reads at its own pace), so the
gateway's working memory does not grow with the page: it holds a few chunks and
the one line in hand, however large the page and however long the line. The
object is one JSON document with `lines` first and the fields that need the
whole read (`total`, `next`, `damaged`, …) after it, so a client that parses
the whole answer sees no difference. All refusals (an unknown instance, a
missing state directory, a bad query) are answered before the first byte; the
status is 200 once the page starts. If the record becomes unreadable part-way,
the body ends with an error and the connection is closed, so a client sees a
truncated answer, never a complete-looking short one. A reader that stops reading is dropped after a **stall deadline**
(30 s with no chunk taken by the client; `Tuning::stall_deadline` sets it for
a test): the one blocking thread that writes its page is freed, and the body
ends in an error, so the client sees the same truncated answer as above. The
deadline is on a stall, not on the page: a client that keeps taking chunks is
never cut off, however large the page or long the read. Without it, a few
hundred clients that connect and read nothing would hold every slot of the
blocking pool and starve every other request. `total` is exact, and
exactness needs the whole intact prefix, so the first page asked of a record
after the process starts reads the record once (a cold first page costs the
record, not the page). The host keeps what that read learned (the length, the
first damage, and a seek mark every 1024 lines) and a later page reads only what
was appended since, then seeks to its place: it costs about its own size. An
index unused for ten minutes is dropped (the next page builds it again), and
one is rebuilt when a segment is replaced or shrinks. A newest-first page reads
backwards from where the record ends, using the same index.

- **A cursor past the end** (`after` or `offset` beyond `total`) is an empty
  last page: 200, `lines: []`, `next: null`, the exact `total`.
- **An empty instance** (no record directory, or no lines) is `lines: []`,
  `total: 0`, `next: null`, `cursor: 0`.
- **Walking the pages** with `after=cursor` until `next` is null yields every
  line once, in order.

### A damaged record

The host writes one line per `write`, gap-free. A reader reads the **intact
prefix**: lines in order until one is not a record line, is not the next
number, or is not UTF-8 (a gap in the numbers is damage). Everything before it
is served, `total` counts it, and `damaged` says where it stopped:

```json
{"after_seq": 3, "segment": "seg-00000001.ndjson", "line": 4, "problem": "the line is not a record line"}
```

(`line` is the bad line's number in that file.) The summary is then the fold of
those lines, with `damaged` set and `problem` saying so. A torn last line of
the newest segment is a write in flight, not damage, and is ignored. Nothing is
repaired: reading never writes.

### An id that is both configured and native

An id may be named by the gateway config (with a `url` and a key) and also have
an entry in the registry folder. The two are one instance with two sources, and
the gateway splits the doors between them: the **registry wins what it can
answer**. The list has one row for the id, the registry's (`source: "registry"`),
and `summary` and `record` are read from the entry's state directory, running or
not, and `queue` is read from it too (S1: a write to it goes to the configured `url` with its key once the state word says a host runs). Every other `/v1` door (the later slices' config, stop, ...) passes
through to the configured `url` with its key, as for any configured instance;
an id with no configured entry answers those with 404 `not_found`, since the
registry holds no address or key. Writes never come from the state directory.

### Proof

`rung-gateway/tests/native_doors.rs` and
`rung-gateway/tests/native_doors_memory.rs`, written first against the
gateway as it was and kept red until the code made them green: every door's
shape; the state words from the record and the lock; paging (page one, the
cursor, the last page, an empty instance, a cursor past the end, a huge page
size, newest first across segments, long lines, blank lines, a torn tail and a
damaged record, a full walk); a stopped instance reading the same as a running
one; a corrupt, gapped, torn or non-UTF-8 record, a dropped state directory, an
unreadable entry; the fold of a recorded live run equal to its own `projection`
at every turn end; a sentinel secret in an event body, a config value, a queue
item, a tool result, an entry's name, an invalid entry's file name, the registry
and state paths and a query value absent from every door and every error body
for both roles; a value named by another instance's config absent from every
door, and the unset names reported; both roles allowed, an unknown token
refused, the Host check applied; the state directory byte-identical (contents
and times) after every door has answered; and a page of the whole record
streamed with a counting allocator showing its peak a small fraction of the page.

`rung-gateway/tests/native_doors_stall.rs` (H3b), written first and
kept red until the stall deadline existed: 520 clients that ask for the whole
record and read nothing (more than the 512 blocking-pool slots) while a normal
request still completes; a dropped client's answer ends short, never
complete-looking; and a client that reads a ~12 MB page slowly, taking several
times the deadline in all but a chunk well inside it each time, gets the whole
page.

## H5 — the event stream with replay

An instance's record, followed, with a cursor to resume from, and the
short-lived partial text and tool progress of the turn that is running. Two
doors carry it, with one shape:

| Door | For | Served from |
|---|---|---|
| `GET /api/i/{id}/v1/events` | a native instance of the gateway's `registry_dir` | the instance's own state directory, so a **stopped** instance streams too; partial frames are relayed from the running host's door (below) |
| `GET /v1/events` on the host's HTTP listener (`acp.http`), beside `/acp` | the host itself (and an instance the gateway config names, which the gateway passes through with its key) | the host's own record and its running turn |

Code: `rung_host::stream` (the stream, no HTTP), `rung_host::acp::EventsDoor`
(the host's door), the gateway's `native_events`. The gateway's rules come
first and are not repeated (H9): both roles may read (the owner with no token,
the read-only role with `Authorization: Bearer <token>` or `?token=`, which an
event stream needs since it cannot set headers); an unknown token is 401; an
owner-role request is Host-checked (403 `bad_host`); any other method is 405
for the owner and 403 `read_only` for the read-only role (a write also meets the
Origin check first). On the host's own door the bearer must be one of the
host's role tokens (any role may read; 401 `unauthorized` otherwise).

### The request

| query | meaning |
|---|---|
| `after` | the cursor: a record number; the stream starts with the line after it. Default 0 (from the first line). |
| `partials` | `on` (default): record and partial frames; `off`: record frames only; `only`: partial frames only (no record is read; this is what the gateway asks of a host) |

The `Last-Event-ID` header, if sent, **is** the cursor and beats `after`: a
browser's `EventSource` reconnects with the id of the last record frame it saw
while the address it was given still carries its first `after`. An empty
`Last-Event-ID` is ignored. A name not in the table, a repeated name, a value
that is not a whole number or `partials` that is not `on`/`off`/`only` is 400
`bad_request`. A cursor past the record's last line is 409 `cursor_ahead`
(a record only grows, so such a cursor is from another instance). An unknown
instance, a missing state directory and the other refusals are the H3 errors,
answered **before the first byte**; the status is 200 once the stream starts.
A bound of 64 bits is the biggest number, as in H3: `after` has no maximum.

The answer is `text/event-stream` (`cache-control: no-cache, no-transform`,
`x-accel-buffering: no`). `HEAD` answers the headers and ends.

### The frames

Each frame is one server-sent event whose `data` is one line of JSON. Only
**record** frames have an `id:`, so a reconnect's `Last-Event-ID` is always the
last record line seen, never a partial frame.

| `event` | `id` | `data` | when |
|---|---|---|---|
| `open` | – | `{"contract": 1, "after": N, "live": bool, "partials": bool, "redaction": {"env_unset": [names]}}` | first frame. `live`: the host's state lock is held. `partials`: this stream will carry partial frames (false for `partials=off`, or when the gateway has no door to the host: see below). `env_unset` as in H3. |
| `record` | the record number | the record line, as the host wrote it (`seq`, `at`, `kind` and the body), redacted | every line after the cursor, in order, once |
| `delta` | – | a partial frame (below), redacted | only while the client is connected; never replayed |
| `end` | – | `{"reason": "stopped"\|"down"\|"damaged"\|"error", "last_seq": N, "problem": null\|text}` | last frame, then the body ends |
| comment `: keepalive` | – | – | every 15 s when idle, so a proxy keeps the path open |

`end` reasons: `stopped` (the state lock is free and the record's last line is
`halted`), `down` (free, any other last line: killed, crashed, never
started), `damaged` (the record stops being intact, as in H3: everything
intact was sent first; `problem` says where), `error` (a read failed). A stream
open when its host stops sends the lines up to and including `halted`, then
`end`. An empty record has nothing to replay and ends at once if no host holds
it.

**The seam.** Replay and live are one read: a reader per client reads the
record's files from the cursor to the end and then as lines are appended, in
file order, never from a second source. So a line appended during the replay is
neither missed nor sent twice. A line still being written (no newline yet) is
not sent until it is whole. A segment rotation is followed. A line is never cut
or capped, however long.

### The partial frames (`delta`)

Short-lived: they exist only in the host's memory and only for the clients
connected when they happen. They are **never written to the record, never
numbered and never replayed**: the record keeps the final text and the
messages as it always did, so it stays reproducible, and a reconnecting client
gets the finished turn from the record. Every `delta` has `turn` (the
`turn.started` line's `turn`), `call` (the model call within the turn, from 0)
and `kind`:

| `kind` | more fields | meaning |
|---|---|---|
| `text` | `text` | more of the answer text |
| `tool_call` | `id`, `name` | the model began a tool call |
| `tool_input` | `id`, `delta` | more of that call's arguments (JSON text, in pieces) |
| `tool_run` | `name`, `input` | the tool is being run with these arguments (a JSON value) |
| `tool_done` | `name`, `ok`, `output` | the tool finished; `output` is its whole text (or the error when `ok` is false) |
| `block_start` | `id` (null: the answer text) | an answer-text block begins (a call's arguments begin with its `tool_call`). Only text of a block whose start the client saw is sent (below) |
| `block_end` | `id` (null for the answer text) | that text block or that call's arguments is complete |
| `turn_end` | – | the turn's engine returned; everything of the turn has been sent |
| `behind` | `skipped` | this client read too slowly and missed `skipped` partial frames (see flow control); `turn` and `call` are 0. Every block open at that moment is cut for this client: its remaining text is not sent, and the stream resumes at the next block |

The model's reasoning ("thinking") is not streamed. Within a call the frames
are in the order the model made them; a call's `tool_run`/`tool_done` come
after its `tool_input` and `block_end`.

**The streaming engine adapter.** `rung_host::adapter::AgentEngine` publishes
these frames through the `TurnRequest.live` handle as the model's stream arrives
(text and argument pieces as they come; `tool_run` and `tool_done` around each
tool). With no client connected no frame is built, but the state of the open blocks (a flag and the open tool-call ids, never text) is still tracked, so a client that connects in the middle of a block finds it already open and is not told it began there. A turn of any other engine
that publishes nothing has only the `turn_end` frame, which the host sends
after every engine turn. The host's `HostBuilder.live` is the bus; a sim
scenario can hand its own (`Scenario.live`) to subscribe.

### Redaction on every frame

Every frame, of every kind, passes through the shared redactor (H4) before it is
written: the record line, the `open` and `end` frames, every field of a partial
frame. The record on disk stays verbatim. A replaced value is `[redacted]`;
nothing is shortened otherwise.

Text that arrives **in pieces** is redacted across the pieces: a key split
between two `text` frames (or two `tool_input` frames) would be in neither. So the
stream holds back the part of a block's text that could still be the start of
something that is removed, and sends the rest as soon as it is settled:

- the last word (a key shape is a word: it is complete when something follows),
- the last `longest secret − 1` bytes (an exact value is found whole),
- a line, or a private-key block, a pattern has left open (`Authorization:`
  waiting for its value, `NAME=`, `BEGIN … ` without its `END`).

What is held is released, redacted, at that block's `block_end` or at the turn's
`turn_end`. Nothing is dropped or cut: the text of
all `text` frames joined is what the redactor makes of the whole text at once
(the tests check this for every split of a text into two pieces and for one
character at a time). The cost is latency: text appears a word, and up to the
longest configured secret, behind the live edge, and a long line a pattern
holds open appears when it closes. Frames that are complete in themselves (`tool_call`,
`tool_run`, `tool_done`, `open`, `end`) are redacted whole.

**The redactor's state is per client and starts at a block's start.** The text of
a block is sent only if this client's redactor saw the block begin (a
`block_start`, or a `tool_call` for a call's arguments). A client that connects
in the middle of a block, or that fell behind (`behind`) while one was open,
has not seen the text a secret may have begun in, and the redactor cannot know
the rest of a secret whose front it never saw. So the rest of that block is not
sent to that client, what the redactor held for it is **discarded, not released**
(a held piece may be the front of a secret), and the stream **resumes at the next
block**, which the client sees from its start (the safe boundary: redaction state
never crosses a block). The block's whole text is in the record. A `block_end`
whose start the client did not see is not sent either. The gateway's relay
applies the same rule to the frames it gets from a host, so it holds across
both hops.

Where the secrets come from, at each door: the gateway door uses the H3
redactor (the gateway's own tokens and keys, and the value of every variable any
registered instance's config names); the host's own door uses the variables its
own config names, its role tokens and the known key shapes.

### Flow control: nothing is dropped silently

- **Record frames are pulled, not pushed.** A client's reader reads the next line
  only when the transport can take it (the body is written one frame at a time),
  so a slow client simply reads later: nothing is dropped, nothing queues (what a
  client has consumed is let go as it is consumed: its memory is a read chunk and
  the unread part of one line, however long the record and however many clients
  resume at once), no
  thread waits (the stream is async; the file reads are short and the wait for a
  new line holds no thread). A client that disconnects drops its reader, file and
  subscription at once; a stalled client holds only its own position.
- **Partial frames** go through a bounded buffer per client (4096 frames). A
  client that falls further behind is **told**: its next frame after the gap is
  `behind` with the count it missed (what that does to the blocks open at the
  gap is under "The redactor's state is per client"), and the stream goes on. The record is unaffected: to see the finished turn, reconnect
  with the last record id.
- **New lines** wake a host's own streams at once (the host notes each line it
  writes); a gateway reading a state directory looks again every 100 ms, so a
  line reaches a client within about a tenth of a second of being written.

### The gateway's door to a running host's partial frames

For a native instance whose state lock is held and whose entry has an
`address`, the gateway asks that host's own door for its partial frames
(`GET /v1/events?partials=only`, bearer: the host's own `acp.tokens` value, the
owner's first, read from the variable its config names; so the gateway must
hold that variable, as H3 already asks for the redaction ones) and relays them
through its own redactor. If the host has no address, takes no token the
gateway holds, does not answer within 5 seconds or refuses, the stream carries
the record only and `open.partials` is false. A host that goes away later ends
the relay; the record goes on. The gateway holds that connection only while the
client holds the stream. For an instance the gateway config names (not in the
registry folder) the stream is the host's own, passed through with its key.

### Proof

`rung-host/tests/events_stream.rs`, `rung-host/tests/events_adapter.rs`
and `rung-host/tests/events_memory.rs`, then `rung-host/tests/events_door.rs`
and `rung-gateway/tests/events.rs`, written first against the code as
it was and kept red until it made them green: replay then live in order; a
line appended during the replay neither missed nor repeated, with no poll to
rescue a missed wake; reconnect from a cursor, by query and by `Last-Event-ID`,
across segments and across a rotation; a cursor ahead refused; a torn line held
back; a 3 MB line whole; a damaged record sent as far as intact; a stopped, a
dead and an empty instance ending cleanly, and a host that stops under a client;
partial frames flagged, in order, never in the record (the record of a watched
run equals that of an unwatched one) and never replayed; a sentinel secret absent
from every kind of frame, from a key split at every point between two pieces, from
a private-key block sent a character at a time; a lagging client told it fell
behind and losing no record line; 100 clients (one stalled) and a runtime with
two blocking threads (thirty waiting streams do not starve the summary door);
a hundred clients replaying one record at once holding memory for what they have
not consumed (counting allocator, not a clock); a secret split across the frames
a lagging client missed, held across a gap, or split by a client's joining in the
middle of a block, never reaching it, at the stream and through the gateway's
relay; disconnects freeing the subscription and the gateway's connection to the host;
both roles, an unknown token, the Host and Origin checks, and the 405s; the
adapter against a loopback provider that streams text split across a secret and
a tool call whose arguments arrive in two pieces.

## S4 — the request record and the context door

What was sent to the model, for one model call of one turn, from the record
alone: a stopped instance answers like a running one, and a turn of any size is
streamed. Two parts: the lines the host now writes, and the door that joins
them. Code: `rung_host::presence` (the lines), `rung_host::context` (the door,
no HTTP), `rung_host::acp::EventsDoor` (the host's listener), the gateway's
`native_context`.

### What the host records

The request is recorded **in layers, once, where each layer is made**, never
once per call (a turn of fifty calls writes the same lines as a turn of one).
Every text is passed through the shared redactor (H4: the value of each variable
the config names, the host's own role tokens, the known key shapes) **before the
line is written**, so a replaced value reads `[redacted]` in the record file
itself. Nothing is cut or shortened; a layer is one whole string in one line.

| line | new fields | written |
|---|---|---|
| `pack.layers` | `cause` (`start`, `swap`), `s_hash`, `tools` (the tool definitions, whole, as the provider is sent them), `system` (the system text, whole) | at every start, in the same write as `host.start`; and at every swap of the stable layer (`Host::swap_stable`, with its `pack.swap`, in one write) |
| `epoch.rollover` | `l_hash`, `slow` (the epoch layer: the one message the epoch's requests start with, whole) | at every rollover (and at a start or wake, which roll over) |
| `turn.layers` (S6) | `turn`, `s_hash`, `l_hash`, `header` (the header without recall), `recall`, `recall_text` (the recall block, whole, **once**; null when the turn had none), `recall_hash` (the hash of the text as stored) | at the start of every turn, in the same write as `turn.started`, so the layers are readable while the turn runs; the event stream carries it like every line |
| `turn.log` | (S4 added `recall_text` and `recall_hash` here; from S6 they are in `turn.layers` and `turn.log` does not repeat them) | when the turn ends, as `turn.log` always was |
| `host.start` | `config` now passes the redactor too (a secret in the identity or the pinned lines is also in the system text) | at every start |

`s_hash` and `l_hash` are the hashes the pack already computes of the
**unredacted** layers (the ones each `llm.call` carries in `prefix`), so a layer
is joined to its calls whether or not something in it was replaced.
`turn.log.header` (the header without recall; redacted at write), `recall` and `messages` (verbatim) keep
their shape. The request bytes sent to the provider, the cache breakpoints and
the pack are unchanged: only what is written changes.

**The recall is written once**, in the turn's `turn.layers` (S6; S4 wrote it in
`turn.log`, and the door still reads a record written that way). `header` (the
header without recall, as it always was) and `recall_text` are the whole of what
the model was shown at the turn's start: the header as sent is `header`, a line break and `recall_text`
(the pack builds it the same way), so the door rebuilds **both** views, the
header as sent and the recall text, from the one copy of each, byte for byte.
The redaction is decided once, on the single copy of the recall: a secret inside
it reads `[redacted]` in both views by construction. `recall_hash` is the hash
of the text as stored; the door checks it and says in words if the text was
damaged (below). A turn recorded before S4 has no `recall_text` key at all (on neither line).

**What it costs.** Measured on a seeded busy run of the mock engine with the
baseline memory (`rung-host/tests/context_record.rs`,
`the_record_grows_by_layers_not_by_calls`: 200 turns, about 500 model calls,
16 epochs, 22 recalls; the run varies a little from one execution to the next):
the record was 1.21 MB without the new lines and 1.87 MB with them (+54%).
`pack.layers` was 9 KB for the run; the 16 epoch layers 97 KB (6 KB each);
the 200 `turn.layers` lines 551 KB: 22 recalls of about 16 KB each (once, not
repeated in `turn.log`) and the header of every turn again (about 180 KB; S4
alone, with the recall in `turn.log`, grew the same run by +34%). Keeping the
recall twice (once in a whole copy of the header as sent, once alone) would have
added far more: the single copy of the recall is the point. The fixture's turns
are short, so the ratio is not a live run's; what carries over is the shape: the
cost is per epoch, per turn (the header) and per recalled turn, not per call. A
live run that recalls large blocks grows by about the size of its recalls, once.

**What stays repeated.** From S6 `turn.layers` also holds the header (the same text `turn.log.header` holds; the recall is not repeated). The header (`turn.log.header`, 156 KB over these 200
turns, already in the record before S4 and not new here) is mostly the same text
turn after turn: 65% of its lines are the same as the previous turn's (the
registers, quota and mode lines). It could be shared (a header written as the
change from the last one, or its unchanging lines once per epoch); this slice
does not change it.

### `GET /v1/turns/{n}/context?call=k`

| Served by | From |
|---|---|
| the host's listener (`acp.http`), beside `/v1/events` | its own record files |
| `GET /api/i/{id}/v1/turns/{n}/context` of the gateway, for a native instance of `registry_dir` | the instance's state directory |

The gateway's rules come first and are not repeated (H9): both roles may read
(the owner with no token; the read-only role with `Authorization: Bearer
<token>`); an unknown token is 401; an owner-role request is Host-checked (403
`bad_host`); a write by the read-only role is 403 `read_only`, by the owner 405.
On the host's own door the bearer must be one of the host's role tokens (any
role may read; 401 `unauthorized` otherwise; 405 for any method but GET and
HEAD). For an instance the gateway config names, the door is the host's, passed
through with its key. Nothing is written, and no host need be running.

| query | meaning |
|---|---|
| `call` | the model call of the turn, counting from 1; the first when not given |

An unknown name, a repeated name, or a value that is not a whole number is 400
`bad_request`; so is a turn that is not a whole number.

| status | `error` | when |
|---|---|---|
| 404 | `no_such_turn` | the record has no such turn |
| 404 | `no_such_call` | the turn has no such call (`call=0` too); the message says how many it has |
| 409 | `turn_not_logged` | the turn has not ended and the record holds no `turn.layers` for it yet (or never will: it began before they were recorded), or `call` is above 1 for a turn that has not ended: its later requests are written when it ends. A client that is watching a turn retries |
| 404, 500 | `no_such_instance`, `state_dir_missing`, `unreadable` | as the H3 doors |

Every refusal is answered before the first byte.

```json
{"turn": 2, "call": 2, "calls": 3, "epoch": 3, "turn_kind": "responding",
 "model": {"requested": "m", "served": "m", "provider": "p"},
 "provider": {"prompt_tokens": 41880, "cached_tokens": 39400, "cache_write_tokens": 0,
              "completion_tokens": 120, "expected_cached_tokens": 39000},
 "cache_event": null, "rewritten": false,
 "recorded": true, "note": null, "missing": [],
 "estimate_tokens": 9800, "scale": 4.27,
 "boundaries": [{"after": "system", "why": "the end of the stable layer: tools and system text",
                 "est_tokens": 1900, "tokens": 8100},
                {"after": "epoch", "why": "the end of the epoch layer", "est_tokens": 2400, "tokens": 10250}],
 "bands": [ … ],
 "damaged": null, "redaction": {"env_unset": []}}
```

| field | meaning |
|---|---|
| `calls` | the model calls the record holds for the turn |
| `provider` | the provider's own counts for this call (`llm.call`): the truth for hit rate. `expected_cached_tokens` is the host's expectation |
| `cache_event` | a `cache.break` or `cache.cold` the host recorded for this call: `{kind, cause}`, else null |
| `rewritten` | the engine rewrote what it was given inside the turn (a context overflow's elision): the bands are then what the host gave it, not what the last calls saw |
| `live` | true while the turn has not ended (S6). The record then holds the turn's layers (`turn.layers`) and no model call or message: the answer is call 1 as assembled, with `calls` 0, a null `provider`, every `cache` mark `unknown` and `this_turn` empty. When the turn ends the same bands settle at the final record: the same text, plus the provider's counts and cache marks. A turn whose host went away before it ended is answered the same way with `live` false and a `problems` entry |
| `recorded` | true when every layer of this call is in the record and nothing in `problems` |
| `problems` | in words, what the record cannot give back whole for this turn: a recall the turn was shown but whose text is not in the record, or a stored recall that does not match its hash (it is then still shown, as recorded, with `intact: false`). Empty normally. Never an error: the answer is 200 |
| `note`, `missing` | for a turn that **predates context recording**: `note` is `this turn predates context recording` and `missing` names the bands that cannot be shown, from `tools`, `system`, `epoch`, `recall` (a turn recorded before S4 has no `recall` band, though it may have had one). The answer is still 200 and holds every band that exists; it is never an error |
| `estimate_tokens` | the host's own estimate of the request: the sum of the bands' |
| `scale` | `prompt_tokens` over `estimate_tokens`; null without a provider count |
| `boundaries` | the two cache breakpoints the engine sets: the end of the stable layer (after `system`) and the end of the epoch layer (after `epoch`), each at its host-estimated token position and at that position scaled to the provider's count (`tokens`); only where the bands before it are in the record |

**The bands** are in the order the model sees them. Each has `band`,
`est_tokens` and `cache`. All text is whole.

| `band` | holds | when |
|---|---|---|
| `tools` | `count` and `items`: one `{name, text}` per tool definition, `text` its canonical JSON | the layers are recorded |
| `system` | `text`: the system text | the layers are recorded |
| `epoch` | `text`: the epoch layer | the rollover is recorded whole |
| `history` | `turns` and `rows`: one `{turn, header, header_complete, messages}` per earlier turn of the epoch, oldest first. `header` is the header as the model saw it; `header_complete` is false for a turn whose recall block is not in the record (one recorded before S4 that was shown a recall, or one whose text is missing) | always |
| `message` | `text`: the turn's header without its recall (the admitted items, the registers it shows) | always |
| `recall` | `text`: the recall block, and `intact` (false when its text does not match the recorded hash) | the turn had a recall |
| `this_turn` | `calls_so_far` and `messages`: this turn's own messages the model had at this call (none at call 1; the earlier calls and their tool results after) | always |

The turn's header is **one** message on the wire: its `message` part first and
its `recall` block after it. They are two bands because they are two things to
the reader; `message` + a line break + `recall` is the header as sent
(the header the model was sent). Joining the bands rebuilds the request byte for byte: the epoch
layer as the first message, each history row's header and messages, the header,
then `this_turn`.

**Sizes.** `est_tokens` is the host's own rule (`canon::tokens`: a token for
four bytes of the band's canonical form, as the pack counts it), not a
tokenizer's: bands may not sum to the provider's count, which is why both are
given. `history` is the sum of its rows.

**Cache marks.** `cache` is `cached`, `partly`, `new` or `unknown` (no provider
count). The provider's `cached_tokens` is laid against the bands in wire order
(tools, system, epoch, history, message, recall, this turn), the host's estimates
scaled to sum to the provider's `prompt_tokens`: a band wholly inside the cached
count is `cached`, one the count ends inside is `partly` (providers cache in
coarse steps), the rest are `new`. No new data is recorded for this.

**Joining.** A call carries `prefix.s_hash` and `prefix.l_hash`; the door takes
the `pack.layers` of that `s_hash` and the `epoch.rollover` of that `l_hash`.
The history is the `turn.log` of every earlier turn since that epoch's rollover.
A turn's messages are sliced at the call: call *k* held the messages before the
turn's *k*-th model message.

**What the record keeps verbatim, and what it redacts.** The record file keeps
history verbatim for the fields the host may replay from or that are the agent's
own words: the `turn.log` `messages`, the stimulus text, and tool calls and
results. Every outgoing view (every door, the stream, the page) is redacted on
read. The content S4 adds (the `pack.layers` text, the epoch layer, the recall
text and the `turn.log` `header`) is redacted once, at write. Messages stay
verbatim so that a rebuild of the context from the record could never differ
from what the model was shown.

**The answer is streamed.** A first read of the record keeps only line numbers
and sizes (a few numbers per turn of the epoch); a second writes each band as its
record line is read, redacted, one line in memory, to a client that reads at its
own pace (a stall deadline of 30 s frees the writer, as in H3). The gateway's
working memory and the host's do not grow with the turn, the epoch or the
history, however long. The object is one JSON document with the small facts
first, then `bands`, then `damaged` and `redaction`; if the record becomes
unreadable part-way, the body ends in an error (the gateway) or short (the host's
listener), and a client sees a document that does not parse, never a complete
short one.

**Redaction.** The record's layers are redacted when written (above); every band
is redacted again on the way out with the door's redactor (the gateway's, as in
H3; the host's own, as for H5), which also covers values only that process
knows, and the turn's `messages` and every history row, which the record keeps
verbatim. A replaced value is `[redacted]`.

### Proof

`rung-host/tests/context_record.rs`, `context_door.rs`,
`rung-gateway/tests/context.rs` and `context_memory.rs`, written first
against stubs and kept red until the code made them green: the layers whole
(a complete tool-definition set, a 100,000-character epoch layer, a
200,000-character recall) through the record and the door; the bands rebuilding
the request the engine was handed, byte for byte, at several calls; a 200,000-character recall that appears once in the record file and comes back whole as both views, a recall that does not resolve (a hash that does not match, text missing or not text) answered in words; a planted
secret (a plain value the redactor was told, and a known key shape) in the
system text and in the recall absent from every byte of the record files and
from the door's answer; one `pack.layers` for a run of 150 model calls, one more
only for a swap, the epoch layer once per epoch; the cache marks and the
provider's counts; an old record answering what it has; the refusals; both roles
over HTTP on a stopped instance, at the host's listener and through the gateway;
a seven-megabyte answer streamed with a counting allocator showing its peak a
small fraction of it; and the request bytes of two seeded runs (one with
rollovers) pinned to the digests taken before this slice.

## S8 — the turns door

A **turn** is one pass of the agent: what it was asked, what it did, what it
said, what it cost. The record holds a turn as a run of lines (`turn.started`,
`stimulus.admitted`, `tool.call`, `llm.call`, `turn.log`, `turn.ended`, …); a
screen that wants the turns used to fold those lines itself, which is a second
copy of the host's reading of its own record. This door is the host's reading:
the turns, folded once, here. A client lays them out and folds nothing.

**Status: served** by the gateway for the native instances of its registry
folder (not by the host's own listener). The proof is listed at the end.

Two doors, for the native instances of a registry folder, read from the state
directory so a **stopped** instance answers the same as a running one (code:
`rung_host::turns`, the gateway's `native_turns`):

| Door | Access | Answers |
|---|---|---|
| `GET /api/i/{id}/v1/turns` | read | the turns, newest first, paged by turn number |
| `GET /api/i/{id}/v1/turns/{n}` | read | one turn, whole |

The gateway's rules come first and are not repeated (H9, H3): both roles may
read; an unknown token is 401; an owner-role request is Host-checked; any
method but GET or HEAD is 405 for the owner and 403 `read_only` for the
read-only role. Errors are H3's (`no_such_instance`, `state_dir_missing`,
`unreadable`), answered before the first byte. **Every text in an answer is
redacted** with the door's redactor (H4); a replaced value is `[redacted]` and
nothing is shortened. The record on disk stays verbatim. The host's own
listener does not serve this door in this slice.

### What a turn is

A turn exists when the record holds its `turn.started` line (a boundary that
the host halted at is not a turn). Turn numbers are the record's own and
ascend; they need not be contiguous. Its facts come from the lines that carry
its `turn` number:

| field | from |
|---|---|
| `turn`, `kind`, `mode`, `project`, `model`, `epoch` | `turn.started` (`turn_kind` is `kind`); `kind` and `model` are then overwritten by `turn.ended`'s when it has them |
| `started_at`, `ended_at`, `elapsed_ms` | `turn.started`'s and `turn.ended`'s `at`, and `turn.ended`'s `elapsed_ms`; `ended_at` and `elapsed_ms` are null without an end |
| `status` | `turn.ended`'s `status` when it is `completed` or `bounded`; `interrupted` when it is `cancelled` (a stop cut it off); else `failed`; with no `turn.ended`: `running` when a host holds the state lock and this is the newest started turn, else `interrupted` (cut off: the host died, or the next turn began) |
| `failure` | `turn.ended`'s `failure` object, whole, when it has a `class`; else null |
| `text` | the turn's reply, whole: the text of every assistant message of its `turn.log`, each trimmed, joined by a blank line; then `turn.ended`'s `final_text` (trimmed) when that is not already inside it; empty while the turn has no `turn.log` |
| `llm` | `{"calls", "prompt_tokens", "cached_tokens", "completion_tokens", "cost_usd"}`: the sums over the turn's `llm.call` lines |
| `tool_calls` | how many entries `calls` has (below) |
| `asked` | how many items the turn was asked (below) |
| `first_seq`, `last_seq` | the first and last record line numbers carrying this turn's number |

### `GET /api/i/{id}/v1/turns`

| query | meaning |
|---|---|
| `limit` | turns in the page, a whole number ≥ 1; default 50 (a default, not a bound). **No maximum** |
| `before` | the cursor: the page holds the newest turns numbered **below** this. Default: from the newest |
| `after` | the cursor: the page holds the oldest turns numbered **above** this, oldest first. Not with `before` |

Turn numbers need not be contiguous, so this list pages by cursor on the turn number, not by `offset` (the one stated exception to the paging rule).

A name not in the table, a repeated name, a value that is not a whole number,
`limit=0`, or both `before` and `after` is 400 `bad_request`.

```json
{"turns": [{"turn": 4, "kind": "free", "mode": "free", "project": null,
            "model": "m", "epoch": 1, "started_at": 1791208196000,
            "ended_at": 1791208197000, "elapsed_ms": 930, "status": "completed",
            "failure": null, "text": "turn 4 done: …",
            "llm": {"calls": 2, "prompt_tokens": 4515, "cached_tokens": 2216,
                    "completion_tokens": 80, "cost_usd": 0.0},
            "tool_calls": 1, "asked": 0, "first_seq": 33, "last_seq": 40}],
 "total": 17, "limit": 50, "order": "desc", "next": null,
 "damaged": null, "redaction": {"env_unset": []}}
```

`turns` is newest first (oldest first with `after`). `total` is the number of
turns in the intact record. `next` is the cursor that continues the page (the
number to pass as `before`, or as `after` with `after`) or null on the last
page. `order` is `desc` or `asc`. `damaged` and `redaction` are as in H3: a
damaged record is read as far as it is intact. `text` is whole in every row, so
a page is as large as its replies.

### `GET /api/i/{id}/v1/turns/{n}`

One turn: the row above with `asked` and `calls` as lists, nothing cut. `{n}` is
a whole number (400 `bad_request` otherwise); a number that is no started turn
is 404 `no_such_turn`.

```json
{"turn": 4, "…": "the row's fields", "tool_calls": 2, "asked": 1,
 "asked_items": [{"id": "q-77", "from": "owner", "role": "owner",
                  "intent": "steer", "text": "also include the appendix",
                  "sent_at": 1791208190000, "seen_at": 1791208196000,
                  "during": 3}],
 "calls": [{"name": "read", "group": "core", "ok": true, "refused": null,
            "input": {"path": "a.md"}, "result": "…whole…"}],
 "damaged": null, "redaction": {"env_unset": []}}
```

`asked_items` (one per item the turn took at its boundary, in the order the
boundary admitted them; `asked` is their count): from the turn's
`stimulus.admitted` `ids`, each joined to its `stimulus.accepted` line. `from`
is `owner` for the owner's, else the item's `kind` (or its `role`); `intent` is
`next`, `free` or `steer` as the item says, else null; `sent_at` is when the
item arrived and `seen_at` when this turn's boundary took it; `during` is the
number of the turn that was running when it arrived, else null (it arrived
between turns, by line number). `text` is the item's whole text.

`calls` (their count is `tool_calls`), in the order the turn made them:

- when the turn has a `turn.log`, one per `tool_use` block of its assistant
  messages, in message order, with `input` (the block's `input`, whole) and
  `result` (the matching `tool_result`'s content, as text; a structure is its
  JSON), `ok` (not the result's `is_error`) and `group` null; `ok` and `result`
  are null when the call has no result;
- when it has none (a turn running, or cut off): one per `tool.call` line,
  with its `name`, `group` and `ok`, and `input` and `result` null;
- when it has none, then one per `tool.refused` line (with a `turn.log` a refusal is already one of its calls): `ok` false, `refused` its
  `why`, `result` its `message`.

### Cost and size

A turn's answer is as large as the turn: its `input` and `result` texts and
its reply are whole, however long. The door reads the record, so a request
costs one read of it (a page reads it twice: the first read finds which turns
the page holds and keeps only a few numbers per turn; the second keeps the
texts of the turns the page holds, and no others). Memory is the size of the
turns asked for, never of the record. There is no index in this slice.

### Not here

Decisions (`/v1/decisions`), the context of a call (S4), spend and the pack
(named in the door table, not served): each is a door of its own with its own
fold on the host. A client that wants a decision of a turn asks that door.

### Proof

`rung-gateway/tests/turns.rs` over real HTTP, with records written by hand so
every number asked for is known (a "running" instance is one whose state lock
the test holds): a record of six turns folds to exactly the rows above (status
for completed, bounded, failed, interrupted and running; the reply from
`turn.log` and `final_text`; the llm sums; the facts and line numbers); a turn
that only a decision line mentions is not a turn; asked items joined from
`stimulus.accepted` with `during` set by line number and an id the record never
accepted skipped; calls from `turn.log` with their results, from `tool.call`
lines without one, and refusals added; a reply and a tool result of 2,000,000
characters come back whole, in the list and in the one turn; a planted secret in
a reply, a tool input and result and an asked item is in the record file and
in no byte of any answer; paging by `before` and `after` following `next` with
no turn missed or repeated, `total`, no maximum on `limit`, an empty page past
the end; the 400s and 404s; both roles reading a stopped instance, a wrong
token 401, a write 405 or 403; a damaged record read as far as it is intact.

**Owed:** the memory bound ("never of the record") is by construction (the
second read keeps only the window's turns) and is not yet shown with a
counting allocator as the record and context doors' are.
