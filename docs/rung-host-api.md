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

## H4: the credential redactor

`rung_host::redact` is a library piece, not a door. It re-exports the
workspace's one redactor, `rung_agent_core::redact` (see "One definition"
below). Every door that returns
record text, every stream line and the console sink pass what they emit
through it first. The record on disk stays verbatim (the cached prompt prefix
is rebuilt from it byte for byte); redaction happens on the way out.

### Contract

- **Replace, never shorten.** A secret value becomes the literal `[redacted]`
  (`MARK`). Everything else is byte-for-byte the input, so no output is
  shorter than its input except by the replaced value. There is no length cap;
  the scan is linear in the text (every forward look is memoised), so a very
  long line, or hostile text such as a million secret-looking names in a row,
  is redacted whole in time proportional to its length.
- **Clean in, same out.** Text with no secret comes back as the same borrowed
  `&str` (`Cow::Borrowed`); a JSON line with no secret comes back byte for
  byte.
- **Idempotent.** Redacting redacted text changes nothing.
- **Name stays, value goes** for headers and assignments:
  `RUNG_HOST_OPENROUTER_API_KEY=[redacted]`, `Authorization: [redacted]`,
  `"api_key": "[redacted]"`. Known key shapes and private key bodies are
  replaced whole (a private key keeps its `-----BEGIN/END … PRIVATE KEY-----`
  frame).

### API

```rust
pub const MARK: &str = "[redacted]";

pub struct Redactor { /* the exact values it was told; Clone, Send, Sync */ }

impl Redactor {
    pub fn new() -> Self;                                   // known shapes only
    pub fn with_mark(mark: &'static str) -> Self;           // another marker
    pub fn from_env_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self;
    pub fn add_env(&mut self, name: &str);                  // value of that env var
    pub fn add_secret(&mut self, value: &str);              // an exact value
    pub fn with_secret(self, value: &str) -> Self;

    /// Text out: secrets replaced; borrowed when there were none.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str>;
    /// A decoded JSON value: every string redacted; every string at any depth
    /// under a secret-named key (a secret-looking name or a credential header
    /// name such as `Authorization`) replaced; object keys
    /// redacted.
    pub fn redact_value(&self, v: &serde_json::Value) -> serde_json::Value;
}

/// In `rung_host::redact` (bring the trait into scope):
pub trait RedactJsonLine {
    /// One record/stream line (no newline). Canonical JSON when something was
    /// replaced, the input untouched when not; non-JSON is redacted as text.
    fn redact_json_line<'a>(&self, line: &'a str) -> Cow<'a, str>;
}

pub fn redact(text: &str) -> Cow<'_, str>;                  // known shapes only
```

Build one `Redactor` at start from every variable the config names
(`engine.api_key_env`, `desk.api_key_env`, each `--acp-token-env` variable)
and share it between the doors. It holds secret values: its `Debug` shows only
a count; never log it.

### What it removes

| kind | replaced | kept |
|---|---|---|
| exact value of a named variable or `add_secret` (6+ chars) | the value, in any context | the rest |
| key shapes: `sk-…`, `sk_live_…`, `ghp_…` (and `gho_ ghu_ ghs_ ghr_`), `github_pat_…`, `glpat-…`, `xox?-…`, `xapp-…`, `xai-…`, `gsk_…`, `tskey-…`, `AKIA…`/`ASIA…`, `AIza…`, `hf_…`, `npm_…`, `pypi-…`, `dp.st.…` (and `pt ct sa`), JWTs | the whole token | the rest |
| private key block | the body | the BEGIN/END lines |
| headers `Authorization`, `Proxy-Authorization` and the two HTTP session-state headers | the whole value (commas, semicolons and quotes inside it included): to the end of the line, or to the quote that opened the header (`-H "…"`), or to the closing quote of a quoted value | the name |
| headers `X-Api-Key`, `Api-Key`, `X-Auth-Token`, `Mcp-Session-Id` | the value, to the next `,` `;` quote or end of line | the name |
| `Bearer <token>` | the token | `Bearer` |
| webhook URLs, exactly these six prefixes: `hooks.slack.com/services/`, `discord.com/api/webhooks/`, `discordapp.com/api/webhooks/`, `hooks.zapier.com/hooks/catch/`, `outlook.office.com/webhook/`, `webhook.office.com/webhookb2/` | the path after the host (the credential) | the host and the rest of the URL |
| URL credentials | the password of `scheme://user:pass@host`; the user of a URL that has only a token, in any scheme but a login one (`ssh`, `sftp`, `scp`, also as the tail of `git+ssh`) | the rest; a plain `ssh://git@host` is left |
| assignments `NAME=value`, `name: value`, `"name": "value"`, `?name=value` whose name says it is a secret | the value | the name and the separator |

Text that is itself JSON-escaped (a recorded tool input) is read as such: an
escaped quote (`\"`) opens and closes a value, an escaped line break (`\n`)
ends one, and a value that holds an escaped backslash or an escaped quote runs
through it.

A name says it is a secret when its last word (split on `_ - .` and camelCase)
is `secret`, `token`, `password`, `passwd`, `pwd`, `passphrase`,
`credential(s)` or `apikey`, or is `key` after `api`, `access`, `secret`,
`private`, `auth`, `signing`, `encryption`, `ssh`, `license` or `master`.
`max_tokens` (a count) and `api_key_env` (a variable's name) are therefore
left alone, and so is a value that is already `[redacted]` or a `$VAR`
reference.

### Limits

It is a pattern redactor. A secret with no recognisable shape that is neither
named by the config nor assigned under a secret-looking name cannot be found.
A name that merely looks secret (`authorization: pending`) is replaced; that
is the deliberate side of the trade.

### Proof

`rung-host/tests/redactor.rs`, with sentinel values only: each key shape, the
private key frame, headers, bearer, assignments, URL credentials, the exact
value of a named variable (including one with a quote or newline inside a
JSON line), the JSON-value walk, prose and counters left alone, a text with
no secret returned unchanged and whole, a long mixed text whose only change is
the replaced values, a multi-megabyte line redacted whole, and hostile inputs
(a run of secret names, header names, token prefixes, quotes) redacted in
linear time.

### Wiring checklist

This slice is deliberately the redactor library. Wiring and the end-to-end
canary proof belong to the slice that adds each door (the H3 read doors have
them; the H5 event stream and the H8 console sink do not exist yet). Each
of these MUST call `rung_host::redact` before emitting and MUST carry its own
canary test, written red first (a secret in the environment, a tool that
prints it, zero hits across every door, stream line and console file):

- [x] H3 read doors (record text): wired and proven by `rung-gateway/tests/native_doors.rs`
- [x] S4 request record and context door: wired and proven by `rung-host/tests/context_record.rs`, `context_door.rs` and `rung-gateway/tests/context.rs`
- [ ] H5 event stream
- [ ] H8 console sink

### One definition

There is one definition of what a credential looks like:
`rung_agent_core::redact::Redactor`. `rung_agent_core::mcp::redact` (used by
memory, the turn check, MCP errors and the gist in `rung-host/src/inbox.rs`)
is that redactor, built once, with its own marker (`[REDACTED]`), followed by
its own exact secrets (registered ones and the well-known key variables),
whose existing tests pass unchanged.
`rung_host::redact` re-exports it and adds the canonical JSON line form, which
needs the host's canonical serializer. A change to what is recognised is made
once, in `rung-agent-core/src/redact.rs`.

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
allowed_hosts: [gw.tailnet.example]   # optional: names a request may be addressed to, besides loopback
read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]   # optional, each names a variable
registry_dir: /home/me/.rung/instances   # optional: the H2 registry folder of native hosts (relative to this file if not absolute)
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
plugs in as another. An instance named here is also the one a **native**
instance of the same id is written to with (S1): `key_env` is the only key the
gateway ever sends a host to write.

### Who may do what

The tailnet is the only boundary: there is no login. A request with no token
has the owner's role. A request that presents a read-only token has the
read-only role and may only read. The token rides `Authorization: Bearer
<token>`, or the `token` query value (an event stream cannot set headers).
It is the gateway's own: it is never forwarded, and neither are the client's
`Authorization` header or its session-state header. A token that matches no configured read-only
token is refused with 401, wherever it appears.

**An `Authorization` header that is there but is not a bearer token the
gateway can read is 401, never "no token".** Only a request with *no*
`Authorization` header (and no `token` value) is the owner's. These are all 401
`unauthorized`, on every door, reads and writes alike: a bare `Bearer`, an empty
`Bearer `, another scheme (`Basic ...`), text that is not UTF-8, more than one
`Authorization` header, a bearer that is not a configured read-only token (the
hosts' own keys included), and an empty `?token=`. The scheme name is read
without regard to case.

A write by the read-only role is refused with 403 before it reaches a host:

```json
{"error": "read_only", "message": "this token can only read"}
```

### Host and Origin

The owner role is whoever reaches the gateway, so every owner-role request
under `/api` (reads included) passes a Host check first, and every write (any
method other than GET or HEAD, whatever door it is for, now or added by a
later slice) also passes an Origin check. A request with a read-only token
skips both: the read-only role keeps its own 403 on a write.

- The `Host` must be `localhost`, `127.0.0.1`, `[::1]`, a name or bare IP in
  `allowed_hosts` (compared without case or port), or the IP the gateway is
  bound to when that is a specific address that is neither loopback nor
  unspecified (so a tailnet-bound listener is not locked out). Anything else,
  or no `Host`, is refused: this is what stops a rebound DNS name from
  reading or driving an instance. Behind the tailnet's serve command, list
  the name it gives the gateway.
- On a write only, an `Origin`, if the client sends one, must be the same
  origin as the request: the same host name as the `Host` and the same port (a
  scheme's default port counts as no port). `null`, another host, or another
  port on the same host is refused. A browser sends `Origin` on every write; a
  script usually sends none and passes.

Refusals are 403 with `{"error": "bad_host", ...}` or `{"error": "bad_origin",
...}`, before anything reaches an instance.

### Routes

| Route | What |
|---|---|
| `GET /api/health` | `{"ok": true}` |
| `GET /api/instances` | the configured instances with each one's summary, and the native instances of `registry_dir` read from their state directories (below, and H3) |
| `/api/i/{id}/v1/...` | pass-through to that instance's `/v1/...`, any method; for a native instance of `registry_dir`, `summary` and `record` are answered from its state directory (H3) |
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
| `GET /v1/turns/{n}/context?call=k` | read | yes | S4 |
| `GET /v1/queue` | read | | S1 |
| `POST /v1/queue` (with `intent`) | write | | S1, S2 |
| `DELETE /v1/queue/{id}`, `POST /v1/queue/{id}/move` | write | | H6 |
| `GET /v1/config` | read | | H7 |
| `POST /v1/config/validate`, `PUT /v1/config` | write | | H7 |
| `GET /v1/console` | read | | H8 |
| `GET /v1/console/stream` | read | yes | H8 |
| `POST /v1/stop`, `POST /v1/release` | write | | S7 |

Of the H3 rows, the first slice serves `summary` and `record` for native
instances (the H3 section below); `status`, `report`, `turns`, `decisions`,
`pack` and `spend` are named here and not served yet. `events` (H5) is served
for native instances from their state directories, and by the host's own
listener (the H5 section).

Validating a config change is classed as a write: it only serves the owner who
may then apply it.

Lists on the host's doors follow one paging rule: `offset`, `limit`, `total`,
`next`; `limit` has no maximum and `total` is exact. The gateway does not
interpret it.

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
entry's fields, `word`, `last`, `problem`) plus:

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
| `state` | `Working`, `Answering`, `Free time`, `Waiting`, `Stuck` (the lock is held), or `Down`, `Stopped`, `Unreadable` (from `word`). Order of the held ones: **Stuck** when the governor is blocked (a refused key) or the copy guard has seen `COPY_LOOP_TURNS` copying turns in a row; **Waiting** when a degraded wait is open (`paced`, `quota`, `backoff`); **Answering** when an item is in a turn or the turn running is `responding`; **Working** when committed to a project; else **Free time**. Thresholds that need a clock (a turn past its bound, a missed watchdog, no progress for a set number of turns) are not derived here |
| `doing` | one line: the commitment, `Free time`, or why it is not running |
| `needs_you` | true for `Stuck` and `Down` (what the record can say; an unread message to the owner is not recorded) |
| `turn` | the last turn started, or null |
| `last_seq`, `last_at` | the last intact line's number and time (ms); `0` and null for an empty record |
| `requests`, `spend_usd_day` | the llm calls and their paid cost for the UTC `day` of the last line (counters are those of that day, not of the day the page is read); `quota` is the config's requests-a-day or null |
| `waiting_for` | what it waits for: `degraded` (`class`, `until`, `why`), `turn` (running now), `boundary` (items waiting for the next one), `calendar` (the next entry due), or null |
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

`rung-gateway/tests/native_doors.rs` (22 tests) and
`rung-gateway/tests/native_doors_memory.rs` (1), written first against the
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

`rung-gateway/tests/native_doors_stall.rs` (3 tests, H3b), written first and
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

`rung-host/tests/events_stream.rs` (37 tests), `rung-host/tests/events_adapter.rs`
(3) and `rung-host/tests/events_memory.rs` (1), then `rung-host/tests/events_door.rs`
(6) and `rung-gateway/tests/events.rs` (11), written first against the code as
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
| `turn.log` | `recall_text` (the recall block, whole, **once**; null when the turn had none), `recall_hash` (the hash of the text as stored; null with it) | when the turn ends, as `turn.log` always was |
| `host.start` | `config` now passes the redactor too (a secret in the identity or the pinned lines is also in the system text) | at every start |

`s_hash` and `l_hash` are the hashes the pack already computes of the
**unredacted** layers (the ones each `llm.call` carries in `prefix`), so a layer
is joined to its calls whether or not something in it was replaced.
`turn.log.header` (the header without recall; redacted at write) and `messages` (verbatim) keep
their shape. The request bytes sent to the provider, the cache breakpoints and
the pack are unchanged: only what is written changes.

**The recall is written once.** `turn.log.header` (the header without recall,
as it always was) and `recall_text` are the whole of what the model was shown at
the turn's start: the header as sent is `header`, a line break and `recall_text`
(the pack builds it the same way), so the door rebuilds **both** views, the
header as sent and the recall text, from the one copy of each, byte for byte.
The redaction is decided once, on the single copy of the recall: a secret inside
it reads `[redacted]` in both views by construction. `recall_hash` is the hash
of the text as stored; the door checks it and says in words if the text was
damaged (below). A turn recorded before S4 has no `recall_text` key at all.

**What it costs.** Measured on a seeded busy run of the mock engine with the
baseline memory (`rung-host/tests/context_record.rs`,
`the_record_grows_by_layers_not_by_calls`: 200 turns, about 500 model calls,
16 epochs, 20 recalls; the run varies a little from one execution to the next):
the record was 1.19 MB without the new fields and 1.60 MB with them (+34%).
`pack.layers` was 8 KB for the run; the 16 epoch layers 87 KB (5.4 KB each);
`recall_text` 316 KB (20 recalls of about 16 KB). Keeping the recall twice (once in a whole copy of the header as sent, once
alone) would have added about 880 KB (+74%) on this run: the single copy more
than halves the growth. The fixture's turns are short, so the
ratio is not a live run's; what carries over is the shape: the cost is per
epoch and per recalled turn, not per call. A live run that recalls large
blocks grows by about the size of its recalls, once.

**What stays repeated.** The header (`turn.log.header`, 156 KB over these 200
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
| 409 | `turn_not_logged` | the turn started and has not ended: its messages are written when it does (the live view is a later slice) |
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

`rung-host/tests/context_record.rs` (12 tests), `context_door.rs` (2),
`rung-gateway/tests/context.rs` (3) and `context_memory.rs` (1), written first
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

## S1 — the queue: list and add, and the gateway's write path

A message to the agent is a **stimulus**. The queue door lists the stimuli that
wait and the one being answered; the same door adds one. Code: `rung_host::queue`
(the fold, no HTTP), `rung_host::acp::QueueDoor` (the host's door),
`rung_gateway` (the write path to a native instance). The slices after this one
add `intent` `steer` and `free` (S2, below) and the stop and release doors; move and
cancel are S3, below. This section is S1 as it was, with S2's changes marked.

### The item

Every list answer, and the `item` of an add, uses one shape, computed from the
record's `stimulus.accepted`, `admitted`, `requeued` and `disposed` lines and the
desk's latest `decision.admit`:

```json
{"id": "q-77", "text": "also include the appendix",
 "role": "owner", "name": null, "channel": "owner", "kind": "peer",
 "intent": "next", "order": 1569, "position": 1,
 "accepted_at": 1791749955653, "state": "waiting",
 "turn": null, "disposition": null, "held": null}
```

| field | meaning |
|---|---|
| `id` | the item's id (an idempotency key, when the sender gave one) |
| `text` | the message, **whole**, through the shared redactor (the record keeps the original) |
| `role`, `name`, `channel` | who sent it. `role` is `owner`, `peer` or `host` (calendar, expectations and other host sources); `name` is the peer's name, the host source's name, or null for the owner; `channel` is the channel as recorded |
| `kind` | the item kind as recorded (`peer` for a message from a channel) |
| `intent` | `next`, `free` or `steer` (S2). The item's own `intent`; a record without one reads `next` |
| `order` | the number of the item's `stimulus.accepted` line: its place by acceptance. Stable |
| `position` | the 1-based place among the items that wait, in list order (owner items first); null for any other state |
| `accepted_at` | when the host accepted it, ms since the epoch (the time sent) |
| `state` | `waiting` (accepted, no boundary has read it), `answering` (in the turn that is answering it), `answered` (disposed by a turn, whatever its `disposition`: `answered`, `digested`, …), `withdrawn` (withdrawn before any turn read it) |
| `turn` | the turn that is answering it, or that disposed it; else null |
| `disposition` | the recorded disposition once over, else null |
| `held` | words saying why the host holds a waiting item (the decision desk holds it until the agent's next break, or as one line in the next turn header; or the model is unavailable and the owner was told), else null |

### `GET /v1/queue`

Computed from the record, so a stopped instance answers the same as a running
one (the gateway reads the state directory; the host's own door reads its own).
The list is ordered for a page: the item being answered; then what waits, **the
owner's items first, then every other sender's, each group in the order the
items were accepted, as S3's moves have changed it** (a peer's item is never
listed above an owner's); then what
is over, the most recently settled first.

Query (all optional; a name it does not know, a repeated name, or a value that is
not as below is 400 `bad_request`):

| parameter | meaning |
|---|---|
| `state` | a comma list of `waiting`, `answering`, `answered`, `withdrawn`, `open` (waiting and answering: **the default**) or `all` |
| `offset`, `limit` | the page, by the paging rule above: `limit` has no maximum (default 500) |

```json
{"items": ["...the items above..."], "total": 3, "offset": 0, "limit": 500,
 "next": null,
 "counts": {"waiting": 2, "answering": 1, "answered": 9, "withdrawn": 0},
 "record_lines": 412, "damaged": null, "redaction": {"env_unset": []}}
```

`total` is the number of items in the asked states, exact; `next` is the next
offset or null on the last page; `counts` is over all four states whatever was
asked; `record_lines` is the last record line read; `damaged` is as in H3 (the
record is read as far as it is intact). `redaction.env_unset` is as for the
other gateway answers (H3). Who may read: the owner role, and the read-only
role (through the gateway); on the host's own door, an owner token only.

### `POST /v1/queue`

```json
{"text": "also include the appendix", "key": "q-77", "intent": "next"}
```

`Content-Type: application/json` (415 otherwise). `text` is required, a string,
and not blank; it is stored whole, **with no limit on its size**. A request is
refused on its headers alone, **before any of its body is read** (see "The
host's own door"). `key` is optional: when
given it **becomes the item id**, so a repeat adds once. A key is 1 to 200 bytes
of letters, digits, `.`, `_`, `:` and `-`, starting with a letter or digit (it is
an id in a path and a record, not content). `intent` is optional: `next` (the default), `free` or `steer` (S2: below); any other value
is refused in words (400), with nothing written. Any other field is refused by name.

The door answers **only after the item is written to the record and fsynced**
(the same acceptance as the ACP `_rung/stimulus` from an owner channel: one code
path), so a message the page shows survives a crash. Answers:

| status | `error` | when |
|---|---|---|
| 201 | | added: `{"id", "repeat": false, "item"}` |
| 200 | | the key was already accepted with this same text: nothing written, `{"id", "repeat": true, "item"}` (the item as it stands now) |
| 400 | `bad_request` | the body, a field or the key is not as above (the message says which) |
| 401 | `unauthorized` | no token, or one that is not the host's |
| 403 | `forbidden` | a token of the host that is not the owner's (host door) |
| 405 | `method_not_allowed` | not GET, HEAD or POST |
| 409 | `key_reused` | the key already names a **different** message; nothing written |
| 409 | `not_running` | the host is stopping, or (gateway) no host is running: **"start it first"**; nothing written. The gateway has no start door |
| 415 | `unsupported_media_type` | not JSON |

A message is never added twice by a retry: send it again with the same key and
the answer is the first one's item with `repeat: true`. Any client that may retry
(the page does) sends a key: the gateway mints none, and a keyless message gets
a new id each time. Item ids are one namespace: the host's own items use the
prefixes `cal-`, `exp-` and `acp-`, and a peer's file names are theirs, so a key
that begins with one of those can collide; `kind` is `peer` for any message from
a channel, so a page marks the owner by `role`.

### The gateway's write path to a native instance

`GET /api/i/{id}/v1/queue` is answered from the instance's state directory (a
registry instance; the same redactor and `env_unset` as `summary`), running or
not. `POST /api/i/{id}/v1/queue` is **forwarded to the running host's own
listener** and its answer is returned (status and body), after the gateway's own
checks, in this order:

1. the gateway's roles and checks, first: an unknown token is 401; the
   read-only token is 403 `read_only` (it may `GET` only); an owner-role request
   passes the Host check and, for a write, the Origin check (H9, 403
   `bad_host` / `bad_origin`). Nothing reaches a host before them.
2. the state word: a registry instance whose state lock is not held (`stopped`,
   `down`) is **409 `not_running`**, with the words "start it first", and nothing
   is sent.
3. where and with what key. **The key is the gateway's own**: the one its own
   config names for this id (`instances[].key_env`, read from this process's
   environment). A variable name an instance supplies (its config file's
   `acp.tokens`, its registry entry) is **never read**, so nothing an instance
   says can make the gateway send it an environment variable. An id the gateway
   config does not name has no write key: 503 `no_owner_key` ("name it in the
   gateway's own config (instances[].key_env)"), nothing sent. Where it goes: the
   running host's registered `address` (it may have asked for a port of its own
   choosing) if that is a **private** address (loopback, a private range, link-local,
   the tailnet's 100.64.0.0/10, or an IPv6 unique-local address; a name is not one
   and is not resolved), else 503 `no_address` and nothing is sent; with no
   registered address, the config's `url`.
   **This is the prototype's trust boundary, stated plainly: the gateway sends
   its write key to any private registry address.** An instance's registry entry
   decides where the key for that id goes, so anyone who can edit the registry
   folder can point the key at any private address (another process on the same
   machine, another box on the tailnet). The registry is the operator's own
   files; this is accepted, and the checks above only keep the key from public
   addresses and from variables an instance names.
4. the forward: the body is **streamed** to the host as it arrives (the gateway
   holds none of it, whatever its size; a refusal above is answered without
   reading it), and the body, `Content-Type` and query go on; the client's
   `Authorization`, its session-state header and `Host` do not, and the host's key is added as the
   bearer. The key value is held in memory only: it is in no response, log, error,
   record or file the gateway writes, and the host's answer, even one that echoes
   its bearer, is passed through the redactor (the key, every config-named value
   and every known key shape) before it is returned.

Failures are plain and never a hang (one async task per request; no thread is
held while the host is slow): a host that cannot be connected to is **502
`instance_unreachable`** ("... could not be reached ...: nothing was queued"); a
connection that broke after the message was sent is 502 saying the message may be
queued; a host that does not answer within the gateway's wait (30 seconds, a
bound on a wait and not on any content: it counts from the last byte moved in
either direction, so a client still sending a long message is not cut off) is **503 `instance_not_answering`**
("the message may or may not be queued; look at the queue, or send it again with
the same key (a repeat adds once)"). Other methods on `/v1/queue` are 405 for a
native instance.

### The host's own door

`GET` and `POST /v1/queue` are on the host's HTTP listener beside `/acp` and
`/v1/events`, owner token only (the host's `acp.tokens`: an unknown token is 401,
another role's token is 403). The list reads the host's own record; every text
and every error is passed through the host's redactor.

**Refused before the body is read.** The listener asks the door, from the request's
headers alone, whether to take a request at all (`RouteGate`): no token or an
unknown one is 401, another role's token 403, a method the door does not have 405,
a body that is not `application/json` 415, a host that is stopping 409
`not_running`. Each is answered at once with `Connection: close` and the body is
never read, so a client sending a body of any size to a request that is refused
costs the host nothing (a 1 GiB generated body to each refusal left the host's
peak memory where it was). Only a request the gate *admits* has its body read, as it
arrives and with no limit, and passed whole to the route (`RouteRequest.body`); a
request to any path the gate does not own (an unserved path, or a door with no
body to take) has none read, and the connection is closed after its answer. An item an
owner sends is stored whole; storing it takes a bounded number of copies of it
(the body, the parsed item, the inbox's, the line written, the answer: about nine
measured), because an item is one record line that every reader reads whole and
the inbox holds it until a turn reads it. Making that flat needs the item's text
kept outside its record line, a change to the record's format, and is not part of
this slice.

Each list and each add reads the record from its start (a fold; linear in the
record's size, about a tenth of a second at 24 MB). That is fine now and is a
cost to watch as records grow; nothing else is read or kept.

### Not here

`stop` and `release` doors, and the page. (Moving and cancelling an item is S3; `intent`
is S2.) Peers' items are listed and ordered here but only the owner adds through this
door.

### Proof

`rung-host/tests/queue_door.rs` (11), `rung-host/tests/queue_door_memory.rs` (3), `rung-gateway/tests/queue.rs` (20) and `rung-gateway/tests/queue_memory.rs` (1), and
the fold's own tests in `rung-host/src/queue.rs`, written first against code with
neither door and kept red until it made them green: an item in the record before
the answer; one key twice is one item, a different message under the key is
refused; a stopped or down instance 409 "start it first" with the record byte for
byte unchanged and no request sent upstream; the read-only token cannot write
and can read; a planted owner key (and a shaped secret, and a host that echoes
its bearer) in no gateway answer; an unreachable host 502 and a silent one 503
while the gateway keeps answering; no key held 503; a bad Host or Origin refused
before any host; a stopped instance's list from its record; a 200,000-character
task queued and listed whole, through the gateway and on the host; peer items
never above owner items; the desk's reason in words; states from the record's
lines; paging with an exact total.

The harm review of the first version added, each written first and each killed
by a mutation of the code it guards: refusals answered before the body is read
and the host's memory unchanged by a 1 GiB generated body (the gate removed:
red); an item past 16 MiB stored whole; 20 concurrent posts with one key make one
201, nineteen 200s and one record line (the atomic check-and-write replaced by
check, wait, write: red); a retry after the host is killed and restarted is one item; a stopping host refuses 409 (the check removed: red);
every form of `Authorization` on the list and the add (a bare, empty or foreign
header read as no token: red); a variable an instance's config names never read
or sent (the old read: red); a registered public address never sent a key; a
256 MiB body streamed through the gateway with its peak memory unchanged, and
refused ones not read (the gateway collecting the body: red); an upload slower
than the wait not cut.

## S3 — the queue: reorder and cancel

The owner can change what waits: put an item in front of another, or take it
back. Two doors on the host's listener beside S1's, and one new record line.
Code: `rung_host::inbox` (`Order`, the fold; `InboxState::plan_move`, the
judgement), `rung_host::presence::Host` (`move_item`, `cancel`),
`rung_host::acp::QueueDoor` (the doors), `rung_gateway` (the write path).

### The record line `queue.moved`

```json
{"seq": 1571, "kind": "queue.moved", "id": "q-80", "before": "q-77", "key": "mv-1"}
```

`id` is the item moved; `before` is the id it was put in front of, or **null for
the end of its group**; `key` is present when the sender gave one. One line is
written for a move that changes the order, durable (fsynced) before the door
answers. Nothing else about an item is changed by it.

**The waiting order is a fold of acceptance and these lines**, the same fold in
the host (what a turn reads) and in every reader of the record (the list):

1. An item takes the end of the order when it is accepted. An item being
   answered keeps its place, so one requeued after a failed or interrupted turn
   waits where it did; an item that is disposed has no place.
2. A `queue.moved` line takes `id` out and puts it in front of `before`
   (after the last waiting item of its group, when `before` is null).
3. A line is applied only when `id` waits, `before` (if any) waits, they are not
   the same item, and both are in **one group**: the owner's `next` and `steer`
   items, the owner's `free` items (S2), a peer's, or the host's own (calendar,
   expectations). The group is `inbox::read_rank(role, intent)`, the same key the
   turn reads by, so an item is never moved to where it would not be read. A line
   that does not meet that is read as no move. (The doors never write one; this keeps a record that has one
   deterministic.)
4. **The later line wins**: two moves are two lines in the order the host took
   its lock, and the fold applies them in that order.
5. The list and the order a turn reads are the groups in the order owner `next`
   and `steer`, owner `free`, peers, host, and inside a group that order. So an
   **owner item stays above a peer's** (a peer's item can never be put above one,
   nor an owner's put below a peer's), and a `free` item stays below the owner's
   `next` and `steer` items and above a peer's.

**A move changes order only.** When an item is shown is still the desk's: its
interruption limits, a firm item's deadline, "at the agent's next break", a
digest line. A moved item that the desk holds is still held, with the same reason
in words in the list (`held`). What a move does change for real is the order a
turn reads the items it admits, and the order the desk's decider is asked about
the non-owner ones: both follow the folded order. **One order, one path:** the
`stimulus.admitted` ids and the rows of the turn header are in the order the list
shows (the fold above, grouped by `read_rank`), so what the owner moved is what the
agent reads (the header was once sorted by acceptance, which undid a move). The record keeps each
item's own `order` (its acceptance) unchanged in the list.

### `POST /v1/queue/{id}/move`

```json
{"before": "q-77", "key": "mv-1"}
```

`before` is **required**: the id of the item to put this one in front of, or
`null` for the end of its group (a page's "down" on the last-but-one row, or
"move to the bottom"). It is an id, not a position: a position goes stale as the
list changes under it, an id names what the owner was looking at. `key` is
optional (the S1 key rule: 1 to 200 bytes of letters, digits, `.`, `_`, `:`, `-`,
starting with a letter or digit). Any other field is refused by name.
`Content-Type: application/json` (415 otherwise). `{id}` is percent-decoded (an
id from a peer's file name may hold a space). Refused on the headers alone, before
any body is read, as S1's.

The door answers **only after the line is in the record and fsynced**. Answers
(`item` is the item in S1's shape, after the move):

| status | `error` | when |
|---|---|---|
| 200 | | moved: `{"id", "moved": true, "repeat": false, "item"}`; the item's `position` is where it is now |
| 200 | | **already in that place** (it is already in front of `before`, or already last): `moved: false`, a `message` saying so in words, **nothing written**; never `moved: true` |
| 200 | | the key already named this same move: `{"moved": false, "repeat": true, "item"}`, nothing written |
| 400 | `bad_request` | the body or a field is not as above (the message says which) |
| 401, 403 | `unauthorized`, `forbidden` | no token or an unknown one; a token of the host that is not the owner's |
| 404 | `not_found` | `id` or `before` names no item (or an owner control: stop and release are no items) |
| 405 | `method_not_allowed` | not POST on this path |
| 409 | `not_waiting` | `id` or `before` is **being answered right now** (it says the turn), or **was answered or withdrawn**: said in words |
| 409 | `group` | `id` and `before` are in different groups (the owner's `next` and `steer`, the owner's `free`, a peer's, the host's own): "a peer's item cannot be put above the owner's items: they stay on top", "the owner's free item cannot be put above a next item" (and the like). A move across them used to be impossible to tell from a move; it is refused, nothing written |
| 409 | `boundary` | the move would jump over an item that waits for a different boundary (below) |
| 409 | `key_reused` | the key already names a **different** move |
| 409 | `not_running` | the host is stopping, or (gateway) no host is running: **"start it first"** |
| 415 | `unsupported_media_type` | not JSON |

Every refusal writes **no line**; the message says "nothing was written".

**The same boundary.** Each waiting item waits for one boundary: the owner's
items are always shown at the next boundary; any other item waits for the one the
desk's latest `decision.admit` gave it (the next boundary, the agent's next break,
or a digest line). A move puts an item in front of another, so it jumps over
every waiting item of its group between where it is and where it goes; each of
those must wait for the same boundary as the item moved, else 409 `boundary`
naming both. (The page's up and down buttons move one place, so a row beside one
with another boundary just has no button.) An owner's `free` item is a group of its
own (S2), so it is never jumped over by, nor jumps, a `next` item.

**Idempotent.** A key names a line: the same key with the same move again is
`repeat: true` and one line; the key is in the line, so it is remembered across a
restart. A move that changes nothing writes nothing and so remembers no key: a
retry of it later moves the item then, if it is still where the retry puts it
the answer is `moved: false`. A client that may retry sends a key.

**Concurrent moves.** The check and the write are one step under the host's lock.
Twenty moves of the same pair at once are one line and nineteen 200s with
`moved: false`; twenty opposing moves are as many lines as changed the order, and
the later line wins.

### `DELETE /v1/queue/{id}`

Cancel: **withdraw a waiting item**. It is the path ACP's `session/cancel` of a
prompt uses (`Host::withdraw`, one `stimulus.disposed` line, disposition
`withdrawn`), so an item's end is the same line however it was cancelled, the item
never reaches the agent, and a client holding it open over ACP is answered
`cancelled`. No body; no key (it is idempotent by itself). Any item the list shows
as waiting may be cancelled, whoever sent it.

| status | `error` | when |
|---|---|---|
| 200 | | withdrawn: `{"id", "withdrawn": true, "repeat": false, "item"}` (state `withdrawn`) |
| 200 | | **already withdrawn**: `repeat: true`, nothing written (a cancel is a no-op the second time, not a refusal: a retry is safe) |
| 401, 403, 405, 409 `not_running` | | as the move's |
| 404 | `not_found` | no such item |
| 409 | `not_waiting` | the item is **being answered right now** (it would cut a running turn; the message says the turn) or was **answered** |

An item a turn took between the look and the withdrawal is the same 409: the
withdrawal itself is checked and written under the lock.

### Roles, a stopped instance, the gateway

Owner token only on the host's door (another role's token 403, none or unknown
401). A stopping host refuses 409 `not_running` ("start it first") from the
moment the stop is raised.

Through the gateway: `POST /api/i/{id}/v1/queue/{item}/move` and
`DELETE /api/i/{id}/v1/queue/{item}` are forwarded to the running host's own
listener by S1's write path and under S1's rules, unchanged: the gateway's roles
and Host/Origin checks first (the read-only token is 403 `read_only`; it never
writes); a stopped instance 409 `not_running` "start it first" with nothing sent;
the gateway's own key, by name, to a private address; the body streamed; the
host's answer redacted again. The item id is sent as the client wrote it
(percent-encoded). A failure says whether anything may have changed: an
unreachable host, "nothing was changed"; a connection that broke or a host that
did not answer, "the change may have been made; look at the queue, then ask again"
(a cancel of an item already withdrawn, and a move to the place the item is in,
change nothing). `GET` of these paths and other methods are 405; a path with more
segments 404. `GET /api/i/{id}/v1/queue` shows the folded order for a stopped
instance too, from its record.

### The order survives a restart

The order is the fold of the record, so a host that is killed and comes back,
and a stopped instance's list, show the order the moves left. A move is on disk
before it is answered.

### Redaction and size

Every text in an answer, and every error (which may name the id it was given),
goes through the redactor. An item is stored and listed whole whatever its size:
a move or a cancel writes a small line that names the item by id and does not
copy or touch its text (a 200,000-character item is the same before and after).

### Not here

`intent` (S2); stop and release are S7. A move does not override the desk,
and a cancel does not cut a running turn (that is what the owner's cancel over
ACP, or a steer, does).

### Proof

`rung-host/tests/queue_move.rs` (17) over real HTTP against the `rung-host`
binary, and `rung-gateway/tests/queue.rs` (the S3 block, 9), and the fold's own
tests in `rung-host/src/inbox.rs` and `queue.rs`, each written first and shown
red against code with no door: an owner item moved up, down and to the end with
the line whole in the record before the answer; a move to its own place writes
nothing; a peer item never passes an owner item and an owner item is never put
below a peer (both ways, all refused in words with no line); an item being
answered, answered, withdrawn or unknown is refused (409 / 404, no line); a
cancelled item is withdrawn once, a repeat is a no-op, it never reaches the
agent after the host comes back with a model; the item being answered cannot be
cancelled; the order after a kill and a restart, and the order a turn reads;
20 equal moves at once make one line, 20 opposing moves leave every line whole
and the order the later line's; a key twice is one line and a reused key is
refused; the read-only and peer tokens never write; a stopping host and a
stopped instance say "start it first"; a 200,000-character item is the same
after a move and a cancel; an error that names the id it was given is redacted.

## S2 — intent on the queue item: `next`, `steer`, `free`

An owner's message says what it is for. `POST /v1/queue` takes `intent`; the item
carries it; the host's desk reads it. Code: `rung_host::inbox::Intent` (the type and
the one question "is this read at the next boundary": `Item::pushes`,
`InboxState::owner_pushes`), `rung_host::desk::admit` (the guard), `rung_host::queue`
(the list), `rung_host::acp::QueueDoor` (the door). The rules the desk keeps are in
`docs/rung-host.md` ("Owner intent").

| `intent` | meaning | cuts a long tool call |
|---|---|---|
| `next` | a task. Read at the next turn boundary, ahead of anything the owner has not yet sent. **The default** | yes |
| `steer` | a message about what the agent is doing now. Read at the next boundary like `next`, and shown as a steer, not a task | yes: the host's own `OwnerWatch`, as for any owner item |
| `free` | wait until the agent is in free time or at a natural break; **never interrupts committed work** | no |

### The door

```json
{"text": "use the other table", "key": "q-78", "intent": "steer"}
```

`intent` is a string, one of the three words, any other value (a different word,
another case, an empty string, a number, a list) is **400 `bad_request`** in words that
name the three, with 0 lines written. A missing or null `intent` is `next`. Everything
else is S1's: the answer comes only after the item is on disk, a key is the item id,
and the answers are S1's table. **A key names one message, and the intent is part of
it:** the same key with the same text and the same intent is a repeat (200, nothing
written); the same key and text under another intent (an omitted one is `next`) is 409
`key_reused`, nothing written, since the page would otherwise show the first intent
for a message the owner meant otherwise. The gateway adds no rule of its own: it
forwards the body as it came, so the intents go through its write path as through the
host's own door, and a host's refusal is passed back with its status.

### The record

`stimulus.accepted`'s `item` carries `"intent": "free"` or `"intent": "steer"`. A
`next` is not written (absent is `next`): every record from before S2 reads as `next`
and its bytes are unchanged. The list and the answer of an add show the intent as one
of the three words; an item whose recorded intent is not one of them reads `next`, and
only an owner's item is ever anything else. The inbox is a fold of the record, so the
intent survives a restart.

### What the host does with it

- **`next`**, as before: `now` at the next boundary, always.
- **`steer`**: read at the next boundary, always, like `next`. The turn header marks
  its row `[owner/id @time · steer]`. It cuts a long tool call exactly as an owner item
  does (a call past one second is cut as soon as it waits; the call's result says the
  owner is waiting): the same `OwnerWatch`, with no second mechanism, and the steer is
  then read at the very next boundary.
- **`free`** (plainly: it waits for free time, but a limit wins over the owner's flag, so it
  is read after 30 minutes at the longest, committed or not): the desk's rule for it is
  `at_break`, not `now`. The guard reads it when
  the agent is not committed (free time, mid-session or at a break) and holds it while the
  agent is committed. It never cuts a long tool call and never wakes a wait the owner
  may cut (a paced wait), so it costs a committed agent nothing. **The desk's limits
  and firm deadlines still win**: a firm item that is due is read (an owner item can be
  firm), and so is any item that has waited its kind's longest (30 minutes for a message
  from a channel), which means a `free` item sent to an agent that stays committed for
  longer than that **is read after 30 minutes**; when it is read for that reason it
  does not lift the hourly limit on interrupting a commitment for any other item. The
  gates G-b and G-m do not count a `free` item as an owner item left past a boundary.

### Order

The owner's items are above every other sender's, always (a peer's item is never above
an owner's, and a `free` owner item is above a peer's). Within the owner's: `next` and
`steer` together, in the order sent (they are read at the same boundary: that boundary
reads them together, in that order); then `free`, in the order sent (it is read later).
The queue list's `position` and order follow it: owner `next`/`steer`, owner `free`,
peers, then the host's own items, each in the order they wait in: acceptance, as the
owner's moves (S3) have changed it. **A move stays inside one of those groups** (the
group is the read rank, so a `free` item is never moved among the `next` ones, which
it would not be read among: that move is refused in words, nothing written), and the
turn header's admitted rows, the `stimulus.admitted` ids and the list are in that one
order. A message's text in the header, from any sender, is whole (white space runs become one
space; nothing is cut or ended with an ellipsis); the shared redactor still runs on it.

### Held

A `free` item the desk's latest decision holds (`at_break`) says so in `held`: "you
asked it to wait: it is read when the agent is in free time or at a natural break, not
while the agent is committed to a project". It is not said before a boundary has looked at
it, and not once it is read.

### The turn view

The record carries what the page's turn view needs: the item's `intent` and `at` (the
time it was sent) on `stimulus.accepted`, and the turn that read it (`stimulus.admitted`,
and `turn` on the item in the list). A steer that cut a long call shows the moment the
agent saw it in the turn's `tool.refused` line (`why: owner_waiting`). The page's own
fold of these is the page's (`ui/src/record/folds.ts`).

### Not here

Moving and cancelling an item (S3), stop and release (S7), a `intent` on ACP's
`_rung/stimulus` and on a directory source's files (an owner's `*.msg` is `next`).

### Proof

`rung-host/tests/intent_desk.rs` (8, the desk's guard: each intent, a firm deadline,
the longest wait, the interrupt limit), `rung-host/tests/queue_move.rs` (the S2 and S3 block, 3: the header in the order the owner moved items to, a move across `free` and `next` refused with no line, `steer`, `next` and `free` after moves and a restart), `rung-host/tests/intent_run.rs` (9, whole
simulated runs on a simulated clock: a steer read at the first boundary after it arrives
and marked, a long steer whole, a free item held through committed turns and read once
the agent is free, the header order, a paced wait a free item does not end, a steer that
cuts a long call and is read next boundary while a free one cuts nothing, the intent
across a restart), `toolbox::tests` (the real `OwnerWatch` on a simulated clock: a
steer cuts a call at the moment it arrives, a free item does not), `queue::tests` (an old
record reads `next`, the order, the words for a held free item), `inbox::tests`,
`gates::tests`, `rung-host/tests/queue_door.rs` (each intent accepted, recorded and
listed; an unknown one refused in words with 0 lines; idempotency with an intent; a
restart), `rung-gateway/tests/queue.rs` (the intents through the gateway's write path)
and the page's contract test of the door (`ui/test/contract-queue.test.ts`, an intent the host does not know). Written first: the desk tests were red against a desk that
forced every owner item to `now`, the run tests against a host with no intent, and
every safeguard was mutated (the exception in the guard, the long-call watch, the
wake, the gates, the ordering, the key's intent, the header mark, the header order)
and killed by a test.

## S7 — stop and release

Two controls of the owner, on the host's listener beside the queue's. Code:
`rung_host::acp::QueueDoor` (the doors, `ControlTarget`), `rung_gateway` (the
write path). **There is no start door**: the unit manager starts a host; a
stopped instance is answered in words.

Both are `POST`, owner token only (no token or an unknown one 401, another role's
token 403, another method 405), refused on the headers alone. The gateway's
read-only token is 403 `read_only` and nothing is sent. Each is the same
acceptance as ACP's `_rung/stop` and `_rung/release` from an owner channel: an
owner control is a `stimulus.accepted` line (with `control`) and a
`stimulus.disposed` line (`control`), fsynced before the door answers; it is
never listed in the queue. A stop is then raised the way the stop file raises
one, and a release goes through the kernel's own owner release.

The check ("is the host already stopping", "is anything held") and the write are
**one step** under one lock, `Host::owner_stop` and `Host::owner_release_control`,
and ACP's `_rung/stop` and `_rung/release` go through the same two methods, so the
two roads record and answer alike: any number of concurrent stops write exactly
one stop line (the rest answer `repeat: true`), and a release writes its lines
only when it releases a commitment (ACP answers `{"released", "repeat"}` too).

### `POST /v1/stop`

No body at all: a declared body (a `Content-Length` other than 0, or a
chunked one) is refused 400 `bad_request` from the headers, before any of it is
read. (Stop takes no content, so this bounds nothing the owner writes.)

| status | when |
|---|---|
| 200 | `{"stopping": true, "repeat": false}`: the stop is in the record and raised; the host halts at its next check and exits |
| 200 | `{"stopping": true, "repeat": true}`: the host is already stopping; **nothing is written** |

### `POST /v1/release`

Body optional: `{"reason": "..."}` (a string; any other field is 400). The
reason is recorded whole, as written (no bound on its length). **A release is
not named to a commitment: it lets go of whatever is held at the moment it
arrives.** If an answer is lost and the agent has since committed to something
else, asking again releases that. Look at the instance before asking again.

| status | when |
|---|---|
| 200 | `{"released": true, "repeat": false}`: the commitment is released (the kernel's `kernel.release` line, `released_by` `owner`) |
| 200 | `{"released": false, "repeat": true}`: nothing is held; **nothing is written** |
| 409 | `not_running`: the host is stopping: "start it first" |

### Through the gateway

`POST /api/i/{id}/v1/stop` and `/v1/release` are forwarded to the running host
by the queue's write path under its rules (the gateway's roles and Host/Origin
checks first, its own key by name, a private address, the answer redacted). A
stopped instance answers 409 `not_running` in words, with nothing sent: for a
stop, that it is already stopped; for a release, "start it first". A failure
says whether anything may have changed (unreachable: nothing; a broken
connection or no answer: look at the instance, then ask again; a stop asked
again changes nothing more, but a release asked again releases whatever is held
then). Other methods are 405.
