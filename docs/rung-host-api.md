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
| webhook URLs: Slack `hooks.slack.com/services/…`, Discord `…/api/webhooks/…`, Zapier `hooks.zapier.com/hooks/catch/…`, Teams `…/webhook/…` | the path after the host (the credential) | the host and the rest of the URL |
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

This slice is deliberately the redactor library. The H3 read doors, the H5
event stream and the H8 console sink do not exist in this tree yet, so wiring
and the end-to-end canary proof belong to the slice that adds each door. Each
of these MUST call `rung_host::redact` before emitting and MUST carry its own
canary test, written red first (a secret in the environment, a tool that
prints it, zero hits across every door, stream line and console file):

- [ ] H3 read doors (record text)
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
