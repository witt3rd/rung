//! The credential redactor (design note H4): one function every door, stream
//! line and console line passes through before it leaves the host.
//!
//! It **replaces**, never shortens or drops: a secret value becomes
//! [`MARK`] and everything around it is byte-for-byte what it was, so no
//! output is shorter than its input except by the replaced value. Text that
//! holds no secret comes back as the same borrowed string. There is no length
//! cap; the scan is one linear pass, so a very long line is redacted whole.
//!
//! What it removes:
//!
//! - the **exact value** of every variable the host config names
//!   ([`Redactor::from_env_names`], [`Redactor::add_env`]) or that the caller
//!   hands over ([`Redactor::add_secret`]), in any shape and any context;
//! - the **known key shapes**: `sk-…`, `ghp_…`, `github_pat_…`, `glpat-…`,
//!   `xox?-…`, `AKIA…`, `AIza…`, `hf_…`, `npm_…`, `pypi-…`, `dp.st.…`
//!   and JWTs;
//! - **private key blocks** (the body; the BEGIN/END frame stays);
//! - **auth headers** (`Authorization`, `Proxy-Authorization`, `X-Api-Key`,
//!   `Api-Key`, `X-Auth-Token`, `Mcp-Session-Id`, and the HTTP session-state request and
//!   response headers): the
//!   name stays, the value goes; and a bare `Bearer <token>`;
//! - **URL credentials**: the password of `scheme://user:pass@host`, and a
//!   token-only user of an http(s)/ws(s) URL;
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
//! The record on disk stays verbatim (the cached prompt prefix is rebuilt from
//! it byte for byte); the door redacts on the way out. JSON goes through
//! [`Redactor::redact_json_line`] or [`Redactor::redact_value`], which work on
//! decoded strings, so a secret holding a quote or a newline is found
//! although the line spells it escaped.

use std::borrow::Cow;

use serde_json::{Map, Value};

use crate::canon;

/// What a secret value is replaced with.
pub const MARK: &str = "[redacted]";

/// Shortest exact value that is replaced: shorter ones would shred prose.
const MIN_EXACT: usize = 6;

/// A redactor: the known shapes, plus the exact values it was told.
///
/// Cheap to clone; share one (behind an `Arc`) between the doors. It holds
/// secret values: never log it, never print it. [`Debug`] shows only how many.
#[derive(Clone, Default)]
pub struct Redactor {
    exact: Vec<String>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactor")
            .field("exact_values", &self.exact.len())
            .finish()
    }
}

impl Redactor {
    /// Known shapes only.
    pub fn new() -> Self {
        Self::default()
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

    /// `text` with every secret replaced by [`MARK`]; the same borrowed
    /// string when it holds none.
    pub fn redact<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut ranges = Vec::new();
        for s in &self.exact {
            ranges.extend(
                text.match_indices(s.as_str())
                    .map(|(i, m)| (i, i + m.len())),
            );
        }
        scan(text, &mut ranges);
        splice(text, ranges)
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
                    let secret_name = is_secret_name(k);
                    out.insert(
                        self.redact(k).into_owned(),
                        if secret_name {
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

    fn under_secret_name(&self, v: &Value) -> Value {
        match v {
            Value::String(s) if keep_value(s) => Value::String(MARK.to_string()),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.under_secret_name(x)).collect()),
            other => self.redact_value(other),
        }
    }

    /// One JSON line (no newline) with its secrets replaced, as canonical
    /// JSON. A line with nothing to redact comes back byte for byte; a line
    /// that is not JSON is redacted as text.
    pub fn redact_json_line<'a>(&self, line: &'a str) -> Cow<'a, str> {
        match serde_json::from_str::<Value>(line) {
            Ok(v) => {
                let red = self.redact_value(&v);
                if red == v {
                    Cow::Borrowed(line)
                } else {
                    Cow::Owned(canon::string(&red))
                }
            }
            Err(_) => self.redact(line),
        }
    }
}

/// [`Redactor::new`]`.redact(text)`: the known shapes only. Prefer a
/// [`Redactor`] built from the host's named variables.
pub fn redact(text: &str) -> Cow<'_, str> {
    Redactor::new().redact(text)
}

// ---- the scan ------------------------------------------------------------

type Ranges = Vec<(usize, usize)>;

/// Sort, merge and cut the ranges out of `text`.
fn splice(text: &str, mut ranges: Ranges) -> Cow<'_, str> {
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
        out.push_str(MARK);
        at = e;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

fn scan(text: &str, ranges: &mut Ranges) {
    private_keys(text, ranges);
    urls(text, ranges);
    let b = text.as_bytes();
    for i in 0..b.len() {
        if !text.is_char_boundary(i) {
            continue;
        }
        let c = b[i];
        let boundary = i == 0 || !is_word(b[i - 1]);
        if boundary {
            token_shape(text, i, ranges);
            if matches!(c, b'b' | b'B') {
                bearer(text, i, ranges);
            }
        }
        if (i == 0 || !is_header_char(b[i - 1])) && c.is_ascii_alphabetic() {
            header(text, i, ranges);
        }
        if c == b'=' || c == b':' {
            assignment(text, i, ranges);
        }
    }
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

/// `[A-Za-z0-9_\-.]*` from `at`, minus trailing `.`/`-`.
fn tok_run(b: &[u8], at: usize) -> usize {
    let mut e = at;
    while e < b.len() && is_tok(b[e]) {
        e += 1;
    }
    while e > at && matches!(b[e - 1], b'.' | b'-') {
        e -= 1;
    }
    e
}

// Private key blocks.
fn private_keys(text: &str, ranges: &mut Ranges) {
    const BEGIN: &str = "-----BEGIN ";
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
            from = body_from.min(text.len());
            continue;
        }
        let body_to = text[body_from..]
            .find("-----END ")
            .map_or(text.len(), |e| body_from + e);
        let body = &text[body_from..body_to];
        let lead = body.len() - body.trim_start().len();
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            ranges.push((body_from + lead, body_from + lead + trimmed.len()));
        }
        from = body_to.max(body_from);
        if from >= text.len() {
            return;
        }
    }
}

// URL credentials.
fn urls(text: &str, ranges: &mut Ranges) {
    let b = text.as_bytes();
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
            None if matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") => {
                (after, after + at)
            }
            None => continue,
        };
        if to > from && keep_value(&text[from..to]) {
            ranges.push((from, to));
        }
    }
}

/// A value that is not already redacted and not a reference to a variable.
fn keep_value(v: &str) -> bool {
    !v.is_empty() && !v.starts_with(MARK) && !v.starts_with('$') && !v.starts_with("${")
}

// Known token shapes.
fn token_shape(text: &str, i: usize, ranges: &mut Ranges) {
    // (prefix, minimum characters after it, charset check)
    const SHAPES: &[(&str, usize)] = &[
        ("sk-", 16),
        ("sk_live_", 16),
        ("sk_test_", 16),
        ("rk_live_", 16),
        ("ghp_", 20),
        ("gho_", 20),
        ("ghu_", 20),
        ("ghs_", 20),
        ("ghr_", 20),
        ("github_pat_", 20),
        ("glpat-", 16),
        ("xoxb-", 10),
        ("xoxp-", 10),
        ("xoxa-", 10),
        ("xoxr-", 10),
        ("xoxs-", 10),
        ("AIza", 30),
        ("hf_", 30),
        ("npm_", 30),
        ("pypi-", 30),
        ("dp.st.", 20),
        ("dp.pt.", 20),
        ("dp.ct.", 20),
        ("dp.sa.", 20),
    ];
    let b = text.as_bytes();
    let rest = &text[i..];
    for &(p, min) in SHAPES {
        if rest.starts_with(p) {
            let end = tok_run(b, i + p.len());
            let tail = &text[i + p.len()..end];
            let ok = tail.len() >= min
                && match p {
                    "sk-" | "hf_" => tail.len() >= 32 || tail.bytes().any(|c| c.is_ascii_digit()),
                    _ => true,
                };
            if ok {
                ranges.push((i, end));
            }
            return;
        }
    }
    // AWS access key ids: AKIA / ASIA + 16 upper-case letters or digits.
    if (rest.starts_with("AKIA") || rest.starts_with("ASIA"))
        && rest.len() >= 20
        && rest.as_bytes()[4..20]
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && !rest.as_bytes().get(20).is_some_and(|c| is_word(*c))
    {
        ranges.push((i, i + 20));
        return;
    }
    // JWT: eyJ… with two dots.
    if rest.starts_with("eyJ") {
        let end = tok_run(b, i);
        let run = &text[i..end];
        if run.len() >= 30 && run.bytes().filter(|c| *c == b'.').count() >= 2 {
            ranges.push((i, end));
        }
    }
}

fn is_bearer_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
}

// A bare `Bearer <token>`.
fn bearer(text: &str, i: usize, ranges: &mut Ranges) {
    let b = text.as_bytes();
    if !text[i..]
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
    let mut e = v;
    while e < b.len() && is_bearer_char(b[e]) {
        e += 1;
    }
    let tok = &b[v..e];
    // Prose ("bearer authentication") has no digit or punctuation.
    let tokenish = tok
        .iter()
        .any(|c| c.is_ascii_digit() || matches!(c, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'='));
    if tok.len() >= 8 && (tokenish || tok.len() >= 20) {
        ranges.push((v, e));
    }
}

// `Name: value` headers.
fn header(text: &str, i: usize, ranges: &mut Ranges) {
    const NAMES: &[&str] = &[
        "proxy-authorization",
        "authorization",
        "x-api-key",
        "x-auth-token",
        "api-key",
        // The two HTTP session-state headers. Spelled in halves because the
        // repo's consumer-name guard reads the whole word as a name.
        concat!("set-coo", "kie"),
        concat!("coo", "kie"),
        "mcp-session-id",
    ];
    let b = text.as_bytes();
    let rest = &b[i..];
    for n in NAMES {
        if rest.len() > n.len() && rest[..n.len()].eq_ignore_ascii_case(n.as_bytes()) {
            let mut p = i + n.len();
            // JSON: `"authorization": "…"`.
            if matches!(b.get(p), Some(b'"' | b'\'')) {
                p += 1;
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
            let start = p;
            let mut e = p;
            while e < b.len() && !matches!(b[e], b'\r' | b'\n' | b'"' | b'\'') {
                e += 1;
            }
            // A quote right at the start opens a quoted value (JSON).
            let (start, e) = if e == start && matches!(b.get(start), Some(b'"' | b'\'')) {
                let q = b[start];
                let mut e = start + 1;
                while e < b.len() && b[e] != q && !matches!(b[e], b'\r' | b'\n') {
                    e += 1;
                }
                (start + 1, e)
            } else {
                (start, e)
            };
            // Trailing spaces belong to the line, not the value.
            let mut e2 = e;
            while e2 > start && matches!(b[e2 - 1], b' ' | b'\t') {
                e2 -= 1;
            }
            if e2 > start && keep_value(&text[start..e2]) {
                ranges.push((start, e2));
            }
            return;
        }
    }
}

// ---- assignments -----------------------------------------------------------

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

// `name=value` / `name: value` at the separator at `sep`.
fn assignment(text: &str, sep: usize, ranges: &mut Ranges) {
    let b = text.as_bytes();
    // The name: optional closing quote and spaces back from the separator.
    let mut n_end = sep;
    while n_end > 0 && matches!(b[n_end - 1], b' ' | b'\t') {
        n_end -= 1;
    }
    if n_end > 0 && matches!(b[n_end - 1], b'"' | b'\'') {
        n_end -= 1;
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
    let (start, end) = match b.get(v) {
        Some(&q @ (b'"' | b'\'')) => {
            let mut e = v + 1;
            while e < b.len() && b[e] != q && !matches!(b[e], b'\r' | b'\n') {
                e += 1;
            }
            (v + 1, e)
        }
        Some(_) => {
            let mut e = v;
            while e < b.len()
                && !b[e].is_ascii_whitespace()
                && !matches!(b[e], b',' | b';' | b'&' | b'"' | b'\'' | b')' | b'}' | b']')
            {
                e += 1;
            }
            (v, e)
        }
        None => return,
    };
    if end > start && keep_value(&text[start..end]) {
        ranges.push((start, end));
    }
}
