# rung-host API — the credential redactor

Part of the host API (slice codes are kept; see `rung-host-doors.md` for the map).

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
| key shapes: `sk-…`, `sk_live_…`, `ghp_…` (and `gho_ ghu_ ghs_ ghr_`), `github_pat_…`, Copilot tokens (`tid=…;exp=…;…`), `glpat-…`, `xox?-…`, `xapp-…`, `xai-…`, `gsk_…`, `tskey-…`, `AKIA…`/`ASIA…`, `AIza…`, `hf_…`, `npm_…`, `pypi-…`, `dp.st.…` (and `pt ct sa`), JWTs | the whole token | the rest |
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
