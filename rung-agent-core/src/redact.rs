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
        self.add_env_with([name], |k| std::env::var(k).ok());
    }

    /// Also remove the value `lookup` gives for each of `names` (unset or
    /// short ones are skipped).
    pub fn add_env_with<'a>(
        &mut self,
        names: impl IntoIterator<Item = &'a str>,
        lookup: impl Fn(&str) -> Option<String>,
    ) {
        for n in names {
            if let Some(v) = lookup(n) {
                self.add_secret(&v);
            }
        }
    }

    /// Also remove this exact value (trimmed; ignored if shorter than six
    /// characters).
    pub fn add_secret(&mut self, value: &str) {
        self.add_secret_min(value, MIN_EXACT);
    }

    /// Also remove this exact value (trimmed), if it has at least `min` bytes.
    pub fn add_secret_min(&mut self, value: &str, min: usize) {
        let v = value.trim();
        if !v.is_empty() && v.len() >= min && !self.exact.iter().any(|e| e == v) {
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
                        if is_secret_name(k) {
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
            Value::String(s) if keep_value(s, self.mark) => Value::String(self.mark.to_string()),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.under_secret_name(x)).collect()),
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

/// The first index at or after `from` whose byte satisfies a predicate, or
/// the end. Queries from one site come in rising order, so the last answer
/// serves every query that falls before it: the total work over a text is one
/// pass, however many matches ask. (This is what keeps hostile input from
/// making every match rescan the rest of the text.)
#[derive(Default, Clone, Copy)]
struct Next {
    from: usize,
    at: usize,
    known: bool,
}

impl Next {
    fn find(&mut self, b: &[u8], from: usize, hit: impl Fn(u8) -> bool) -> usize {
        if self.known && self.from <= from && from <= self.at {
            return self.at;
        }
        let mut e = from;
        while e < b.len() && !hit(b[e]) {
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
    dq_line: Next,
    sq_line: Next,
    hdr_line: Next,
    hdr_strict: Next,
    asg_end: Next,
    space_trim: (usize, usize),
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
            dq_line: Next::default(),
            sq_line: Next::default(),
            hdr_line: Next::default(),
            hdr_strict: Next::default(),
            asg_end: Next::default(),
            space_trim: (usize::MAX, 0),
        }
    }

    fn run(&mut self) {
        self.private_keys();
        self.urls();
        let b = self.b;
        for i in 0..b.len() {
            if !self.text.is_char_boundary(i) {
                continue;
            }
            let c = b[i];
            if i == 0 || !is_word(b[i - 1]) {
                self.token_shape(i);
                if matches!(c, b'b' | b'B') {
                    self.bearer(i);
                }
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
        let run_end = self.tok_end.find(self.b, at, |c| !is_tok(c));
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
                None if matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") => {
                    (after, after + at)
                }
                None => continue,
            };
            if to > from && keep_value(&text[from..to], self.mark_text) {
                self.ranges.push((from, to));
            }
        }
    }

    // Known token shapes.
    fn token_shape(&mut self, i: usize) {
        // (prefix, minimum characters after it)
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
        let b = self.b;
        let rest = &self.text[i..];
        for &(p, min) in SHAPES {
            if rest.starts_with(p) {
                let tail_at = i + p.len();
                let end = self.tok_run(tail_at);
                let tail_len = end - tail_at;
                let ok = tail_len >= min
                    && match p {
                        "sk-" | "hf_" => {
                            tail_len >= 32
                                || self.digit.find(b, tail_at, |c| c.is_ascii_digit()) < end
                        }
                        _ => true,
                    };
                if ok {
                    self.ranges.push((i, end));
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
            self.ranges.push((i, i + 20));
            return;
        }
        // JWT: eyJ… with two dots.
        if rest.starts_with("eyJ") {
            let end = self.tok_run(i);
            if end - i >= 30 {
                let d1 = self.dot1.find(b, i, |c| c == b'.');
                if d1 < end {
                    let d2 = self.dot2.find(b, d1 + 1, |c| c == b'.');
                    if d2 < end {
                        self.ranges.push((i, end));
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
        let e = self.bearer_end.find(b, v, |c| !is_bearer_char(c));
        // Prose ("bearer authentication") has no digit or punctuation.
        let tokenish = self.tokenish.find(b, v, |c| {
            c.is_ascii_digit() || matches!(c, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
        }) < e;
        if e - v >= 8 && (tokenish || e - v >= 20) {
            self.ranges.push((v, e));
        }
    }

    /// End of a quoted value that opened at `open` (the quote is `b[open]`):
    /// the closing quote or the end of the line.
    fn quoted_end(&mut self, open: usize) -> usize {
        let b = self.b;
        let q = b[open];
        if q == b'"' {
            self.dq_line
                .find(b, open + 1, |c| matches!(c, b'"' | b'\r' | b'\n'))
        } else {
            self.sq_line
                .find(b, open + 1, |c| matches!(c, b'\'' | b'\r' | b'\n'))
        }
    }

    // `Name: value` headers.
    fn header(&mut self, i: usize) {
        // (name, value runs to the end of the line)
        const NAMES: &[(&str, bool)] = &[
            ("proxy-authorization", false),
            ("authorization", false),
            ("x-api-key", false),
            ("x-auth-token", false),
            ("api-key", false),
            ("mcp-session-id", false),
            // The two HTTP session-state headers carry `;`-separated pairs.
            ("set-cookie", true),
            ("cookie", true),
        ];
        let b = self.b;
        let rest = &b[i..];
        for &(n, to_eol) in NAMES {
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
                let (start, e) = if matches!(b.get(p), Some(b'"' | b'\'')) {
                    (p + 1, self.quoted_end(p))
                } else if to_eol {
                    (
                        p,
                        self.hdr_line
                            .find(b, p, |c| matches!(c, b'\r' | b'\n' | b'"' | b'\'' | b'\\')),
                    )
                } else {
                    (
                        p,
                        self.hdr_strict.find(b, p, |c| {
                            matches!(c, b'\r' | b'\n' | b'"' | b'\'' | b'\\' | b',' | b';')
                        }),
                    )
                };
                // Trailing spaces belong to the line, not the value.
                let e2 = self.trim_spaces(e).max(start).min(e);
                if e2 > start && keep_value(&self.text[start..e2], self.mark_text) {
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
            Some(b'"' | b'\'') => (v + 1, self.quoted_end(v)),
            Some(_) => (
                v,
                self.asg_end.find(b, v, |c| {
                    c.is_ascii_whitespace()
                        || matches!(
                            c,
                            b',' | b';' | b'&' | b'"' | b'\'' | b'\\' | b')' | b'}' | b']'
                        )
                }),
            ),
            None => return,
        };
        if end > start && keep_value(&text[start..end], self.mark_text) {
            self.ranges.push((start, end));
        }
    }
}

/// A value that is not already redacted and not a reference to a variable.
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
