//! The credential redactor: the one definition of what a credential looks
//! like. Every door, stream line and console line of the host passes through
//! it before it leaves (design note H4), and so does every other place that
//! shows text rung did not write (a provider's error, tool output, a memory
//! gist): [`crate::mcp::redact`] is this redactor with its own marker.
//!
//! It **replaces**, never shortens or drops: a secret value becomes the
//! marker ([`MARK`] unless [`Redactor::with_mark`] says otherwise) and
//! everything around it is byte-for-byte what it was, so no output is shorter
//! than its input except by the replaced value. Text that holds no secret
//! comes back as the same borrowed string. There is no length cap, and the
//! scan is linear in the text: every forward look is memoised, so hostile
//! input (a million secret-looking names in a row) costs no more per byte
//! than ordinary text.
//!
//! What it removes:
//!
//! - the **exact value** of every variable the caller names
//!   ([`Redactor::from_env_names`], [`Redactor::add_env`]) or hands over
//!   ([`Redactor::add_secret`]), in any shape and any context;
//! - the **known key shapes**: `sk-…`, `ghp_…`, `github_pat_…`, `glpat-…`,
//!   `xox?-…`, `AKIA…`, `AIza…`, `hf_…`, `npm_…`, `pypi-…`, `dp.st.…`
//!   and JWTs;
//! - **private key blocks** (the body; the BEGIN/END frame stays);
//! - **auth headers** (`Authorization`, `Proxy-Authorization`, `X-Api-Key`,
//!   `Api-Key`, `X-Auth-Token`, `Mcp-Session-Id`, `Cookie`, `Set-Cookie`):
//!   the name stays, the value goes (to the next `,` or `;` for the first
//!   six, to the end of the line for the two cookie headers, which carry
//!   `;`-separated pairs); and a bare `Bearer <token>`;
//! - **URL credentials**: the password of `scheme://user:pass@host`, and a
//!   token-only user (`scheme://token@host`) of any scheme but a login one
//!   (`ssh`, `sftp`, `scp`, also as the tail of `git+ssh`), where the user is a name;
//! - **assignments** whose name says it is a secret (`NAME=value`,
//!   `name: value`, `"name": "value"`, `?name=value`): the name stays, the
//!   value goes. A name says so when its last word is `secret`, `token`,
//!   `password`, `passwd`, `pwd`, `passphrase`, `credential(s)`, `apikey`, or
//!   `key` after `api`, `access`, `secret`, `private`, `auth`, `signing`,
//!   `encryption`, `ssh`, `license` or `master`. Words split on `_ - .` and
//!   camelCase, so `max_tokens` (a count) and `api_key_env` (a variable's
//!   name) are left alone.
//!
//! It is a pattern redactor: it cannot know a secret that has no shape and
//! was not named. The config names every key by variable, which is why
//! [`Redactor::from_env_names`] exists.
//!
//! The host's record on disk stays verbatim (the cached prompt prefix is
//! rebuilt from it byte for byte); the door redacts on the way out. JSON goes
//! through [`Redactor::redact_value`], which works on decoded strings, so a
//! secret holding a quote or a newline is found although the line spells it
//! escaped.

use std::borrow::Cow;

use serde_json::{Map, Value};

/// What a secret value is replaced with, unless [`Redactor::with_mark`] says
/// otherwise.
pub const MARK: &str = "[redacted]";

/// Shortest exact value that is replaced: shorter ones would shred prose.
const MIN_EXACT: usize = 6;

/// A redactor: the known shapes, plus the exact values it was told.
///
/// Cheap to clone; share one (behind an `Arc`) between the doors. It holds
/// secret values: never log it, never print it. [`Debug`] shows only how many.
#[derive(Clone)]
pub struct Redactor {
    exact: Vec<String>,
    mark: &'static str,
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactor")
            .field("exact_values", &self.exact.len())
            .field("mark", &self.mark)
            .finish()
    }
}

impl Redactor {
    /// Known shapes only, replacing with [`MARK`].
    pub fn new() -> Self {
        Self {
            exact: Vec::new(),
            mark: MARK,
        }
    }

    /// Known shapes only, replacing with `mark`.
    pub fn with_mark(mark: &'static str) -> Self {
        Self {
            exact: Vec::new(),
            mark,
        }
    }

    /// Known shapes plus the current value of each named environment
    /// variable (unset or short ones are skipped).
    pub fn from_env_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut r = Self::new();
        for n in names {
            r.add_env(n);
        }
        r
    }

    /// Also remove the current value of the environment variable `name`.
    pub fn add_env(&mut self, name: &str) {
        if let Ok(v) = std::env::var(name) {
            self.add_secret(&v);
        }
    }

    /// Also remove this exact value (trimmed; ignored if shorter than six
    /// characters).
    pub fn add_secret(&mut self, value: &str) {
        let v = value.trim();
        if v.len() >= MIN_EXACT && !self.exact.iter().any(|e| e == v) {
            self.exact.push(v.to_string());
        }
    }

    /// Builder form of [`Self::add_secret`].
    pub fn with_secret(mut self, value: &str) -> Self {
        self.add_secret(value);
        self
    }

    /// `text` with every secret replaced by the marker; the same borrowed
    /// string when it holds none.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut ranges = Vec::new();
        for s in &self.exact {
            ranges.extend(
                text.match_indices(s.as_str())
                    .map(|(i, m)| (i, i + m.len())),
            );
        }
        Scan::new(text, &mut ranges, self.mark).run();
        splice(text, ranges, self.mark)
    }

    /// `v` with every string redacted, every string under a secret-named key
    /// replaced, and object keys redacted.
    pub fn redact_value(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.redact(s).into_owned()),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.redact_value(x)).collect()),
            Value::Object(m) => {
                let mut out = Map::new();
                for (k, x) in m {
                    out.insert(
                        self.redact(k).into_owned(),
                        if is_secret_key(k) {
                            self.under_secret_name(x)
                        } else {
                            self.redact_value(x)
                        },
                    );
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    /// Everything textual under a secret-named key is the secret: strings
    /// are replaced at any depth, numbers and flags are left.
    fn under_secret_name(&self, v: &Value) -> Value {
        match v {
            Value::String(s) if keep_value(s, self.mark) => Value::String(self.mark.to_string()),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.under_secret_name(x)).collect()),
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, x)| (self.redact(k).into_owned(), self.under_secret_name(x)))
                    .collect(),
            ),
            other => self.redact_value(other),
        }
    }
}

/// [`Redactor::new`]`.redact(text)`: the known shapes only. Prefer a
/// [`Redactor`] built from the variables the config names.
pub fn redact(text: &str) -> Cow<'_, str> {
    Redactor::new().redact(text)
}

// ---- the scan ------------------------------------------------------------

type Ranges = Vec<(usize, usize)>;

/// Sort, merge and cut the ranges out of `text`.
fn splice<'a>(text: &'a str, mut ranges: Ranges, mark: &str) -> Cow<'a, str> {
    if ranges.is_empty() {
        return Cow::Borrowed(text);
    }
    ranges.sort_unstable();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    let mut i = 0;
    while i < ranges.len() {
        let (s, mut e) = ranges[i];
        i += 1;
        while i < ranges.len() && ranges[i].0 <= e {
            e = e.max(ranges[i].1);
            i += 1;
        }
        out.push_str(&text[at..s]);
        out.push_str(mark);
        at = e;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

/// The first index at or after `from` where a predicate holds, or the end.
/// Queries from one site come in rising order, so the last answer serves
/// every query that falls before it: the total work over a text is one pass,
/// however many matches ask. (This is what keeps hostile input from making
/// every match rescan the rest of the text.) The predicate looks at the bytes
/// around an index but not at where the search began, so an answer stays
/// valid for any start before it.
#[derive(Default, Clone, Copy)]
struct Next {
    from: usize,
    at: usize,
    known: bool,
}

impl Next {
    fn find(&mut self, b: &[u8], from: usize, hit: impl Fn(&[u8], usize) -> bool) -> usize {
        if self.known && self.from <= from && from <= self.at {
            return self.at;
        }
        let mut e = from;
        while e < b.len() && !hit(b, e) {
            e += 1;
        }
        *self = Next {
            from,
            at: e,
            known: true,
        };
        e
    }
}

/// A backslash that starts an escape: not itself the second of a pair.
fn escape_at(b: &[u8], i: usize) -> bool {
    b[i] == b'\\' && !(i > 0 && b[i - 1] == b'\\')
}

/// An escape that is a quote (`\"`, `\'`).
fn escaped_quote_at(b: &[u8], i: usize) -> bool {
    escape_at(b, i) && matches!(b.get(i + 1), Some(b'"' | b'\''))
}

/// An escape that breaks a line or a value (`\n`, `\r`).
fn escaped_break_at(b: &[u8], i: usize) -> bool {
    escape_at(b, i) && matches!(b.get(i + 1), Some(b'n' | b'r'))
}

/// A quote that closes a quoted value: not escaped by a single backslash.
fn closing_quote_at(b: &[u8], i: usize, q: u8) -> bool {
    b[i] == q && !(i > 0 && b[i - 1] == b'\\' && !(i > 1 && b[i - 2] == b'\\'))
}

fn line_end(c: u8) -> bool {
    matches!(c, b'\r' | b'\n')
}

/// What closes a quoted value that opened at the quote or escaped quote
/// found by [`opening`].
#[derive(Clone, Copy, PartialEq)]
enum Close {
    /// A plain quote: the same quote, unescaped, or the end of the line.
    Quote(u8),
    /// An escaped quote in JSON-escaped text: the next escaped quote.
    Escaped,
}

/// A quoted value opening at `p`: where the value starts and what closes it.
fn opening(b: &[u8], p: usize) -> Option<(usize, Close)> {
    match b.get(p) {
        Some(&q @ (b'"' | b'\'')) => Some((p + 1, Close::Quote(q))),
        Some(b'\\') if matches!(b.get(p + 1), Some(b'"' | b'\'')) => Some((p + 2, Close::Escaped)),
        _ => None,
    }
}

/// Where a header's value ends when no quote opened it.
#[derive(Clone, Copy)]
enum Class {
    /// To the end of the line: `Authorization` and the cookie headers carry
    /// commas, semicolons and quotes inside their values.
    Line,
    /// To the next comma, semicolon, quote or break: a plain token.
    Token,
}

/// The header names whose values are credentials.
const HEADERS: &[(&str, Class)] = &[
    ("proxy-authorization", Class::Line),
    ("authorization", Class::Line),
    ("x-api-key", Class::Token),
    ("x-auth-token", Class::Token),
    ("api-key", Class::Token),
    ("mcp-session-id", Class::Token),
    ("set-cookie", Class::Line),
    ("cookie", Class::Line),
];

struct Scan<'a, 'r> {
    text: &'a str,
    b: &'a [u8],
    ranges: &'r mut Ranges,
    mark_text: &'static str,
    // One memo per forward look.
    tok_end: Next,
    tok_trim: (usize, usize),
    digit: Next,
    dot1: Next,
    dot2: Next,
    bearer_end: Next,
    tokenish: Next,
    dq_close: Next,
    sq_close: Next,
    esc_close: Next,
    hdr_line: [Next; 4],
    hdr_token: Next,
    asg_end: Next,
    space_trim: (usize, usize),
    /// Where the last key shape ended: a key glued on right after it starts
    /// a new word.
    glue: usize,
}

impl<'a, 'r> Scan<'a, 'r> {
    fn new(text: &'a str, ranges: &'r mut Ranges, mark: &'static str) -> Self {
        Scan {
            text,
            b: text.as_bytes(),
            ranges,
            mark_text: mark,
            tok_end: Next::default(),
            tok_trim: (usize::MAX, 0),
            digit: Next::default(),
            dot1: Next::default(),
            dot2: Next::default(),
            bearer_end: Next::default(),
            tokenish: Next::default(),
            dq_close: Next::default(),
            sq_close: Next::default(),
            esc_close: Next::default(),
            hdr_line: [Next::default(); 4],
            hdr_token: Next::default(),
            asg_end: Next::default(),
            space_trim: (usize::MAX, 0),
            glue: usize::MAX,
        }
    }

    fn run(&mut self) {
        self.private_keys();
        self.urls();
        self.webhooks();
        let b = self.b;
        for i in 0..b.len() {
            if !self.text.is_char_boundary(i) {
                continue;
            }
            let c = b[i];
            if i == 0 || !b[i - 1].is_ascii_alphanumeric() || i == self.glue {
                self.token_shape(i);
            }
            if (i == 0 || !is_word(b[i - 1])) && matches!(c, b'b' | b'B') {
                self.bearer(i);
            }
            if (i == 0 || !is_header_char(b[i - 1])) && c.is_ascii_alphabetic() {
                self.header(i);
            }
            if c == b'=' || c == b':' {
                self.assignment(i);
            }
        }
    }

    /// The end of the `[A-Za-z0-9_\-.]` run from `at`, minus trailing `.`/`-`.
    fn tok_run(&mut self, at: usize) -> usize {
        let run_end = self.tok_end.find(self.b, at, |b, i| !is_tok(b[i]));
        // The last character that may end a token, before `run_end`: memoised
        // per run so a run of candidates does not each walk back over it.
        if self.tok_trim.0 != run_end {
            let mut e = run_end;
            while e > 0 && matches!(self.b[e - 1], b'.' | b'-') {
                e -= 1;
            }
            self.tok_trim = (run_end, e);
        }
        self.tok_trim.1.max(at).min(run_end)
    }

    // Private key blocks.
    fn private_keys(&mut self) {
        const BEGIN: &str = "-----BEGIN ";
        let text = self.text;
        let mut from = 0;
        while let Some(p) = text[from..].find(BEGIN) {
            let start = from + p;
            let label_at = start + BEGIN.len();
            let Some(q) = text[label_at..].find("-----") else {
                return;
            };
            let label = &text[label_at..label_at + q];
            let body_from = label_at + q + 5;
            if !label.contains("PRIVATE KEY") || label.contains('\n') {
                // Not a block start: resume right after this marker, so a real
                // block that begins inside what looked like its label is found.
                from = label_at;
                continue;
            }
            let body_to = text[body_from..]
                .find("-----END ")
                .map_or(text.len(), |e| body_from + e);
            let body = &text[body_from..body_to];
            let lead = body.len() - body.trim_start().len();
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                self.ranges
                    .push((body_from + lead, body_from + lead + trimmed.len()));
            }
            from = body_to.max(body_from);
            if from >= text.len() {
                return;
            }
        }
    }

    // URL credentials.
    fn urls(&mut self) {
        let text = self.text;
        let b = self.b;
        for (idx, _) in text.match_indices("://") {
            let mut s = idx;
            while s > 0 && b[s - 1].is_ascii_alphabetic() {
                s -= 1;
            }
            let scheme = text[s..idx].to_ascii_lowercase();
            let after = idx + 3;
            let mut end = after;
            while end < b.len()
                && !matches!(
                    b[end],
                    b'/' | b'?' | b'#' | b'"' | b'\'' | b')' | b'>' | b'<' | b']'
                )
                && !b[end].is_ascii_whitespace()
            {
                end += 1;
            }
            let authority = &text[after..end];
            let Some(at) = authority.rfind('@') else {
                continue;
            };
            let user = &authority[..at];
            let (from, to) = match user.find(':') {
                Some(c) => (after + c + 1, after + at),
                // A token-only user is a secret in any scheme but a login one,
                // where the user is a name (`ssh://git@host`).
                None if !matches!(scheme.as_str(), "ssh" | "sftp" | "scp") => (after, after + at),
                None => continue,
            };
            if to > from && keep_value(&text[from..], self.mark_text) {
                self.ranges.push((from, to));
            }
        }
    }

    // Webhook URLs: the path after the host is the credential.
    fn webhooks(&mut self) {
        const PREFIXES: &[&str] = &[
            "hooks.slack.com/services/",
            "discord.com/api/webhooks/",
            "discordapp.com/api/webhooks/",
            "hooks.zapier.com/hooks/catch/",
            "outlook.office.com/webhook/",
            "webhook.office.com/webhookb2/",
        ];
        let text = self.text;
        let b = self.b;
        for p in PREFIXES {
            for (idx, _) in text.match_indices(p) {
                let from = idx + p.len();
                let mut e = from;
                while e < b.len()
                    && (b[e].is_ascii_alphanumeric() || matches!(b[e], b'_' | b'-' | b'/'))
                {
                    e += 1;
                }
                while e > from && b[e - 1] == b'/' {
                    e -= 1;
                }
                if e - from >= 8 {
                    self.ranges.push((from, e));
                }
            }
        }
    }

    // Known token shapes.
    fn token_shape(&mut self, i: usize) {
        // (prefix, minimum characters after it, may follow `_`)
        const SHAPES: &[(&str, usize, bool)] = &[
            ("sk-", 16, false),
            ("sk_live_", 16, true),
            ("sk_test_", 16, true),
            ("rk_live_", 16, true),
            ("ghp_", 20, true),
            ("gho_", 20, true),
            ("ghu_", 20, true),
            ("ghs_", 20, true),
            ("ghr_", 20, true),
            ("github_pat_", 20, true),
            ("glpat-", 16, true),
            ("xoxb-", 10, true),
            ("xoxp-", 10, true),
            ("xoxa-", 10, true),
            ("xoxr-", 10, true),
            ("xoxs-", 10, true),
            ("xapp-", 20, true),
            ("xai-", 20, true),
            ("gsk_", 20, true),
            ("tskey-", 16, true),
            ("AIza", 30, true),
            ("hf_", 30, false),
            ("npm_", 30, false),
            ("pypi-", 30, false),
            ("dp.st.", 20, true),
            ("dp.pt.", 20, true),
            ("dp.ct.", 20, true),
            ("dp.sa.", 20, true),
        ];
        let b = self.b;
        let rest = &self.text[i..];
        let after_word = i > 0 && i != self.glue && is_word(b[i - 1]);
        for &(p, min, after_underscore) in SHAPES {
            if rest.starts_with(p) {
                // A generic prefix needs a word boundary (`task-…` is not
                // `sk-…`); a distinctive one may follow an underscore.
                if after_word && !(after_underscore && b[i - 1] == b'_') {
                    return;
                }
                let tail_at = i + p.len();
                let end = self.tok_run(tail_at);
                let tail_len = end - tail_at;
                let ok = tail_len >= min
                    && match p {
                        "sk-" | "hf_" => {
                            tail_len >= 32
                                || self.digit.find(b, tail_at, |b, j| b[j].is_ascii_digit()) < end
                        }
                        _ => true,
                    };
                if ok {
                    self.ranges.push((i, end));
                    self.glue = end;
                }
                return;
            }
        }
        if after_word {
            return;
        }
        // AWS access key ids: AKIA / ASIA + 16 upper-case letters or digits.
        if (rest.starts_with("AKIA") || rest.starts_with("ASIA"))
            && rest.len() >= 20
            && rest.as_bytes()[4..20]
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            // What follows (another key glued on, a longer id) is not part of
            // the id; keep scanning there.
            self.ranges.push((i, i + 20));
            self.glue = i + 20;
            return;
        }
        // JWT: eyJ… with two dots.
        if rest.starts_with("eyJ") {
            let end = self.tok_run(i);
            if end - i >= 30 {
                let d1 = self.dot1.find(b, i, |b, j| b[j] == b'.');
                if d1 < end {
                    let d2 = self.dot2.find(b, d1 + 1, |b, j| b[j] == b'.');
                    if d2 < end {
                        self.ranges.push((i, end));
                        self.glue = end;
                    }
                }
            }
        }
    }

    // A bare `Bearer <token>`.
    fn bearer(&mut self, i: usize) {
        let b = self.b;
        if !self.text[i..]
            .get(..6)
            .is_some_and(|s| s.eq_ignore_ascii_case("bearer"))
        {
            return;
        }
        let mut v = i + 6;
        if !matches!(b.get(v), Some(b' ' | b'\t')) {
            return;
        }
        while matches!(b.get(v), Some(b' ' | b'\t')) {
            v += 1;
        }
        let e = self.bearer_end.find(b, v, |b, j| !is_bearer_char(b[j]));
        // Prose ("bearer authentication") has no digit or punctuation.
        let tokenish = self.tokenish.find(b, v, |b, j| {
            b[j].is_ascii_digit() || matches!(b[j], b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
        }) < e;
        if e - v >= 8 && (tokenish || e - v >= 20) {
            self.ranges.push((v, e));
        }
    }

    /// End of a quoted value that starts at `start` and closes as `close`
    /// says.
    fn quoted_end(&mut self, start: usize, close: Close) -> usize {
        let b = self.b;
        match close {
            Close::Quote(b'"') => self.dq_close.find(b, start, |b, i| {
                line_end(b[i]) || closing_quote_at(b, i, b'"')
            }),
            Close::Quote(q) => self
                .sq_close
                .find(b, start, |b, i| line_end(b[i]) || closing_quote_at(b, i, q)),
            Close::Escaped => self
                .esc_close
                .find(b, start, |b, i| line_end(b[i]) || escaped_quote_at(b, i)),
        }
    }

    // `Name: value` headers.
    fn header(&mut self, i: usize) {
        let b = self.b;
        let rest = &b[i..];
        for &(n, class) in HEADERS {
            if rest.len() > n.len() && rest[..n.len()].eq_ignore_ascii_case(n.as_bytes()) {
                let mut p = i + n.len();
                // JSON: `"authorization": "…"`, or escaped `\"authorization\": …`.
                if let Some((after, _)) = opening(b, p) {
                    p = after;
                }
                while matches!(b.get(p), Some(b' ' | b'\t')) {
                    p += 1;
                }
                if b.get(p) != Some(&b':') {
                    return;
                }
                p += 1;
                while matches!(b.get(p), Some(b' ' | b'\t')) {
                    p += 1;
                }
                let (start, e) = if let Some((start, close)) = opening(b, p) {
                    (start, self.quoted_end(start, close))
                } else {
                    // The quote that opened the header itself (a `-H "…"`
                    // argument), if any, closes its value.
                    let opener = if i > 0 && matches!(b[i - 1], b'"' | b'\'') {
                        if i > 1 && b[i - 2] == b'\\' {
                            Some(Close::Escaped)
                        } else {
                            Some(Close::Quote(b[i - 1]))
                        }
                    } else {
                        None
                    };
                    let end = match class {
                        Class::Line => {
                            let slot = match opener {
                                None => 0,
                                Some(Close::Quote(b'"')) => 1,
                                Some(Close::Quote(_)) => 2,
                                Some(Close::Escaped) => 3,
                            };
                            self.hdr_line[slot].find(b, p, |b, j| {
                                line_end(b[j])
                                    || escaped_break_at(b, j)
                                    || match opener {
                                        None => false,
                                        Some(Close::Quote(q)) => closing_quote_at(b, j, q),
                                        Some(Close::Escaped) => escaped_quote_at(b, j),
                                    }
                            })
                        }
                        Class::Token => self.hdr_token.find(b, p, |b, j| {
                            line_end(b[j])
                                || matches!(b[j], b'"' | b'\'' | b',' | b';')
                                || escaped_quote_at(b, j)
                                || escaped_break_at(b, j)
                        }),
                    };
                    (p, end)
                };
                // Trailing spaces belong to the line, not the value.
                let e2 = self.trim_spaces(e).max(start).min(e);
                if e2 > start && keep_value(&self.text[start..], self.mark_text) {
                    self.ranges.push((start, e2));
                }
                return;
            }
        }
    }

    /// `e` moved back over spaces and tabs (memoised per `e`).
    fn trim_spaces(&mut self, e: usize) -> usize {
        if self.space_trim.0 != e {
            let mut t = e;
            while t > 0 && matches!(self.b[t - 1], b' ' | b'\t') {
                t -= 1;
            }
            self.space_trim = (e, t);
        }
        self.space_trim.1
    }

    // `name=value` / `name: value` at the separator at `sep`.
    fn assignment(&mut self, sep: usize) {
        let b = self.b;
        let text = self.text;
        // The name: optional closing quote (escaped, in JSON-escaped text) and
        // spaces back from the separator.
        let mut n_end = sep;
        while n_end > 0 && matches!(b[n_end - 1], b' ' | b'\t') {
            n_end -= 1;
        }
        if n_end > 0 && matches!(b[n_end - 1], b'"' | b'\'') {
            n_end -= 1;
            if n_end > 0 && b[n_end - 1] == b'\\' {
                n_end -= 1;
            }
        }
        let mut n_start = n_end;
        while n_start > 0 && is_name_char(b[n_start - 1]) {
            n_start -= 1;
        }
        if n_start == n_end || !is_secret_name(&text[n_start..n_end]) {
            return;
        }
        // The value.
        let mut v = sep + 1;
        while matches!(b.get(v), Some(b' ' | b'\t')) {
            v += 1;
        }
        let (start, end) = if let Some((start, close)) = opening(b, v) {
            (start, self.quoted_end(start, close))
        } else if v < b.len() {
            (
                v,
                self.asg_end.find(b, v, |b, j| {
                    b[j].is_ascii_whitespace()
                        || matches!(b[j], b',' | b';' | b'&' | b'"' | b'\'' | b')' | b'}' | b']')
                        || escaped_quote_at(b, j)
                        || escaped_break_at(b, j)
                        || (escape_at(b, j) && b.get(j + 1) == Some(&b't'))
                }),
            )
        } else {
            return;
        };
        if end > start && keep_value(&text[start..], self.mark_text) {
            self.ranges.push((start, end));
        }
    }
}

/// Whether the value that starts `v` (the text from its first character on,
/// not only the value, since a marker holds a `]` that ends an unquoted value)
/// is to be replaced: not already redacted, not a reference to a variable.
fn keep_value(v: &str, mark: &str) -> bool {
    !v.is_empty()
        && !v.starts_with(mark)
        && !v.starts_with(MARK)
        && !v.starts_with('$')
        && !v.starts_with("${")
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn is_header_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-'
}

fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')
}

fn is_tok(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')
}

fn is_bearer_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
}

// ---- secret names ----------------------------------------------------------

/// Words of a name, split on `_ - .` and camelCase, lower-cased.
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if matches!(ch, '_' | '-' | '.') || !ch.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
        cur.extend(ch.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Whether a map key says its value is a secret: a secret-looking name, or
/// the name of a header that carries a credential.
fn is_secret_key(key: &str) -> bool {
    is_secret_name(key) || HEADERS.iter().any(|(n, _)| key.eq_ignore_ascii_case(n))
}

/// Whether `name` says its value is a secret.
fn is_secret_name(name: &str) -> bool {
    let w = words(name);
    let Some(last) = w.last() else {
        return false;
    };
    match last.as_str() {
        "secret" | "token" | "password" | "passwd" | "pwd" | "passphrase" | "credential"
        | "credentials" | "apikey" => true,
        "key" => {
            w.len() >= 2
                && matches!(
                    w[w.len() - 2].as_str(),
                    "api"
                        | "access"
                        | "secret"
                        | "private"
                        | "auth"
                        | "signing"
                        | "encryption"
                        | "ssh"
                        | "license"
                        | "master"
                )
        }
        _ => false,
    }
}
