# rung-host API — the doors a UI or gateway uses

This is the written shape of what the continuous host offers to something
that is not the agent: a command, files, and (as the slices land) HTTP doors.
It is the contract between the host and the multi-instance UI
(`docs/reports/rung-ui-design-v1/`). A section is appended as its slice
merges and is kept stable after: changes inside version 1 only add; a
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
| `ID.lock` | the liveness lock. The running host holds an exclusive `flock` on it for its whole life. |

### Who registers

- `rung-host run --config FILE` always registers. The name is `name:` in the
  file; without it, the last component of `state:`.
- `rung-host sim` (the test harness) registers only when given `--name NAME`.
- Registration happens **before the record is opened**: a refused start
  touches nothing. A refusal exits with the bad-start code `2` and names the
  entry it collides with:
  - the id belongs to a **live** host (`pid N`);
  - the id is registered for **another state directory**;
  - this state directory is registered under **another id**.
  A restart (same id, same state, lock free) renews the entry. Nothing is
  ever replaced behind the operator's back.
- The id is the name as a slug: ASCII letters and digits lowercased, every
  other run of characters one `-`, none at the ends (`Deep Thought` →
  `deep-thought`). A name with no letter or digit is refused.
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

1. Open `ID.lock` and take a shared, non-blocking `flock`. If it **fails
   because the lock is held** the host lives: **`running`**. Otherwise
   release it at once. (A missing lock file means no host ever held it.)
2. The lock is free. Read the **last line** of `STATE_DIR/record/` (the
   newest segment's last complete line; a torn tail is ignored). If its
   `kind` is `halted` the host stopped itself: **`stopped`**. Anything else
   (or no line): it died: **`down`**.
3. An entry file that cannot be parsed, or a lock that cannot be probed, is
   **`unreadable`**, listed with its `problem`.

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
| `problem` | string or null | why something could not be read (an unparseable entry, an unreadable record); null otherwise |

An `unreadable` entry carries only `id` (the file's name), `word`, `last`
(null) and `problem`.

A `down` host's `pid` is the pid that **was** its process; `ls` shows it as
`-` and the JSON still carries it. Do not signal it: the number may have
been reused.
