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
| `problem` | string or null | why something could not be read (an unparseable entry, an unreadable record); null otherwise |

An `unreadable` entry carries only `id` (the file's name), `word`, `last`
(null) and `problem`.

A `down` host's `pid` is the pid that **was** its process; `ls` shows it as
`-` and the JSON still carries it. Do not signal it: the number may have
been reused.

## H9 — the gateway

`rung-gateway` is one listener in front of any number of rung hosts. It
serves the UI's built app, lists the instances it fronts, and passes
`/v1` through to one instance, adding that instance's key and streaming the
answer back. The keys are held only here: no page and no response carries
one. It is a product crate, not published.

```
rung-gateway --config gateway.yaml
```

### Config

Names of variables, never secrets:

```yaml
listen: 127.0.0.1:8787            # default; tailscale serve puts it on https
app_dir: ui/dist                  # optional; relative to this file
read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]   # optional, each names a variable
instances:
  - id: alpha                     # letters, digits, '-', '_' (a path segment)
    name: Alpha                   # optional, defaults to the id
    url: http://127.0.0.1:7001    # the host's own listener, http only
    key_env: ALPHA_HOST_KEY       # the variable holding the owner key
```

Startup refuses, naming the variable and never its value, when a variable is
unset or empty, an id is repeated or not a plain name, a url is not `http://`,
a key has a byte a header cannot carry (line break, NUL, non-ASCII), or a field is unknown. The instances come from a `Registry` (a trait asked on
every request); the config file is one source, and the registry folder (H2)
plugs in as another.

### Who may do what

The tailnet is the only boundary: there is no login. A request with no token
has the owner's role. A request that presents a read-only token has the
read-only role and may only read. The token rides `Authorization: Bearer
<token>`, or the `token` query value (an event stream cannot set headers).
It is the gateway's own: it is never forwarded, and neither are the client's
`Authorization` header or its session-state header. A token that matches no configured read-only
token is refused with 401, wherever it appears.

A write by the read-only role is refused with 403 before it reaches a host:

```json
{"error": "read_only", "message": "this token can only read"}
```

### Routes

| Route | What |
|---|---|
| `GET /api/health` | `{"ok": true}` |
| `GET /api/instances` | the registry with each instance's summary (below) |
| `/api/i/{id}/v1/...` | pass-through to that instance's `/v1/...`, any method |
| anything else | the app: a file under `app_dir`, or `index.html` for a path with no extension (client-side routes); `/api/...` is never the app |

Only `/v1/` of an instance passes (its ACP and other paths do not). A path
with a segment that decodes to `.`, `..`, or contains a separator is 404, for
the passed paths and the app alike. Unknown instance: 404
`no_such_instance`. Errors are always `{"error": <code>, "message": <text>}`.

`GET /api/instances`:

```json
{"instances": [
  {"id": "alpha", "name": "Alpha", "reachable": true,
   "summary": {"...": "the instance's GET /v1/summary, as it answered"},
   "error": null},
  {"id": "beta", "name": "beta", "reachable": false,
   "summary": null, "error": "client error (Connect): ..."}
]}
```

An instance that does not answer within 5 seconds, or answers badly, is
listed with `reachable: false`, never left out.

The pass-through adds `Authorization: Bearer <that instance's key>`, keeps the
method, body, query (minus `token`) and end-to-end headers (so `Last-Event-ID`
resumes a stream), and does not buffer the answer: an event stream arrives as
the host writes it. An instance that cannot be reached answers 502
`instance_unreachable`; error text never carries a key. A registry that hands over a key a header cannot carry gets the same 502 for that instance (and `reachable: false` in the overview), never a dropped connection. A failed `accept` is logged to stderr and retried after a short pause.

### The door table

`rung_gateway::doors::DOORS` names each host door and its access. The gateway
streams every answer through, so a door's being a stream changes nothing
there; the Stream column below is for the reader of the host's API only. A request not in the table is classed by method: GET and
HEAD read, anything else writes. The read-only role may use `Read` doors only.
Later slices add rows as their doors land.

| Door | Access | Stream | Slice |
|---|---|---|---|
| `GET /v1/summary`, `/v1/status`, `/v1/report` | read | | H3 |
| `GET /v1/record`, `/v1/turns`, `/v1/turns/{n}`, `/v1/decisions`, `/v1/pack`, `/v1/spend` | read | | H3 |
| `GET /v1/events?after=N` | read | yes | H5 |
| `GET /v1/queue` | read | | H6 |
| `POST /v1/queue`, `DELETE /v1/queue/{id}`, `POST /v1/queue/{id}/move` | write | | H6 |
| `GET /v1/config` | read | | H7 |
| `POST /v1/config/validate`, `PUT /v1/config` | write | | H7 |
| `GET /v1/console` | read | | H8 |
| `GET /v1/console/stream` | read | yes | H8 |
| `POST /v1/stop`, `POST /v1/release` | write | | exist over ACP |

Validating a config change is classed as a write: it only serves the owner who
may then apply it.

Lists on the host's doors follow one paging rule: `offset`, `limit`, `total`,
`next`; `limit` has no maximum and `total` is exact. The gateway does not
interpret it.
