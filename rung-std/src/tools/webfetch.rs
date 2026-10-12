//! `webfetch` — HTTP GET for the agent. http(s) only; whole body, streamed; HTML becomes Markdown.
//!
//! It fetches any address it is given (loopback, private networks and
//! redirect targets included). It is not a security boundary.

use super::Tool;
use super::fsutil;
use serde_json::Value;
use std::io::Read;

const TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct WebFetch;

impl Tool for WebFetch {
    fn name(&self) -> &'static str {
        "webfetch"
    }
    fn description(&self) -> &'static str {
        "Fetch a http(s) URL (any address it is given; loopback and private networks are not refused, and this is not a security boundary). Returns a header line (final URL after redirects, HTTP status), a blank line, then the whole body: HTML pages are converted to Markdown (headings, [links](url), lists, code, tables as ` | ` rows; style, script, noscript and template text removed); other content types are returned as is."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "http or https URL"}
            },
            "required": ["url"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let url = fsutil::req_str(input, "url")?;
        fetch_url(url)
    }
}

pub fn fetch_url(url: &str) -> Result<String, String> {
    check_url(url)?;
    fetch_with(url, std::time::Duration::from_secs(TIMEOUT_SECS))
}

/// One piece of a streamed fetch, in order: the header line once (final URL
/// and status, without its trailing blank line), then the body in pieces.
#[derive(Debug, Clone, Copy)]
pub enum Piece<'a> {
    Header(&'a str),
    Body(&'a str),
}

/// The whole-page fetch (`webfetch`), but the page is handed to `sink` as it arrives instead of
/// being gathered: memory stays the size of one piece, whatever the page.
/// Joined (`header`, a blank line, the body pieces) the pieces are exactly
/// what the whole-page fetch returns. A sink error stops the fetch and is returned.
pub fn fetch_url_streaming(
    url: &str,
    sink: &mut dyn FnMut(Piece<'_>) -> Result<(), String>,
) -> Result<(), String> {
    check_url(url)?;
    fetch_stream(url, std::time::Duration::from_secs(TIMEOUT_SECS), sink)
}

/// Fetch with a wall-clock bound `limit` on the request and on reading
/// the whole body. A time bound, not a content cap: the body, however
/// large, is returned whole if it arrives in time.
fn fetch_with(url: &str, limit: std::time::Duration) -> Result<String, String> {
    let mut out = String::new();
    fetch_stream(url, limit, &mut |p| {
        match p {
            Piece::Header(h) => {
                out.push_str(h);
                out.push_str("\n\n");
            }
            Piece::Body(t) => out.push_str(t),
        }
        Ok(())
    })?;
    Ok(out)
}

fn fetch_stream(
    url: &str,
    limit: std::time::Duration,
    emit: &mut dyn FnMut(Piece<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + limit;
    let client = reqwest::blocking::Client::builder()
        .timeout(limit)
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("rung-std-webfetch/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("webfetch: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("webfetch: HTTP {status}"));
    }
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let html = ctype.contains("html");
    // Header: the URL actually reached (after redirects) and the status.
    emit(Piece::Header(&format!(
        "url: {} | status: {status}",
        resp.url()
    )))?;
    // `out` is the working buffer of the sink: it is handed on and emptied
    // after every chunk, except its last character (the markdown writer
    // looks back one character), so memory is a chunk, never the page.
    let mut out = String::new();
    // Stream: each chunk is decoded and appended to `out`; the body is
    // never held a second time. A reader thread feeds the chunks
    // so the whole-body deadline is enforced exactly, even while a single
    // read is blocked (a bare read can wait out its own timeout).
    let mut sink = Sink::new(html);
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, String>>(4);
    std::thread::spawn(move || {
        let mut resp = resp;
        loop {
            let mut buf = vec![0u8; 64 * 1024];
            match resp.read(&mut buf) {
                Ok(0) => return, // dropping `tx` signals end of body
                Ok(n) => {
                    buf.truncate(n);
                    if tx.send(Ok(buf)).is_err() {
                        return; // deadline passed; the fetch is over
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("webfetch: {e}")));
                    return;
                }
            }
        }
    });
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let chunk = match rx.recv_timeout(left) {
            Ok(r) => r?,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return Err(format!(
                    "webfetch: body not complete within {}s",
                    limit.as_secs_f64().ceil() as u64
                ));
            }
        };
        carry.extend_from_slice(&chunk);
        let used = decode_lossy(&carry, false, |t| sink.feed(t, &mut out));
        carry.drain(..used);
        hand_on(&mut out, emit, false)?;
    }
    decode_lossy(&carry, true, |t| sink.feed(t, &mut out));
    sink.finish(&mut out);
    hand_on(&mut out, emit, true)
}

/// Pass `out` to `emit`, keeping its last character unless `all`.
fn hand_on(
    out: &mut String,
    emit: &mut dyn FnMut(Piece<'_>) -> Result<(), String>,
    all: bool,
) -> Result<(), String> {
    let keep = if all {
        0
    } else {
        out.chars().next_back().map_or(0, char::len_utf8)
    };
    let n = out.len() - keep;
    if n > 0 {
        emit(Piece::Body(&out[..n]))?;
        out.drain(..n);
    }
    Ok(())
}

/// Decode `bytes` as lossy UTF-8, calling `f` per piece. Returns bytes
/// consumed; an incomplete trailing sequence is left unconsumed unless
/// `last`.
fn decode_lossy(bytes: &[u8], last: bool, mut f: impl FnMut(&str)) -> usize {
    let mut i = 0;
    while i < bytes.len() {
        match std::str::from_utf8(&bytes[i..]) {
            Ok(t) => {
                f(t);
                return bytes.len();
            }
            Err(e) => {
                let v = e.valid_up_to();
                f(std::str::from_utf8(&bytes[i..i + v]).unwrap_or(""));
                i += v;
                match e.error_len() {
                    Some(l) => {
                        f("\u{FFFD}");
                        i += l;
                    }
                    None => {
                        if last {
                            f("\u{FFFD}");
                            return bytes.len();
                        }
                        return i;
                    }
                }
            }
        }
    }
    i
}

/// Incremental output sink: plain passthrough, or HTML -> Markdown.
struct Sink {
    md: Option<Md>,
}

impl Sink {
    fn new(html: bool) -> Self {
        Sink {
            md: html.then(Md::default),
        }
    }
    fn feed(&mut self, s: &str, out: &mut String) {
        match &mut self.md {
            Some(m) => m.feed(s, out),
            None => out.push_str(s),
        }
    }
    fn finish(&mut self, out: &mut String) {
        if let Some(m) = &mut self.md {
            m.finish(out);
        }
    }
}

/// List nesting levels that indent further. Rule: item `d` (0-based depth)
/// is padded by `2 * min(d, INDENT_STOPS_AT)` spaces, so output stays
/// linear in the input for any nesting; from level `INDENT_STOPS_AT + 1`
/// on, the item line also carries its true depth as `[depth N] ` (N is
/// 1-based) so no structure information is lost. Only indentation
/// whitespace is clamped; every item's text is kept whole.
const INDENT_STOPS_AT: usize = 8;

/// Elements whose text is never shown (CSS/JS/template noise).
const SKIPPED: [&str; 4] = ["style", "script", "noscript", "template"];

/// Streaming HTML -> Markdown. All state survives chunk edges, so memory
/// is the tag being parsed, never the page. Breaks and spaces are lazy:
/// they are written only when more content follows.
#[derive(Default)]
struct Md {
    lt: bool,
    in_tag: bool,
    in_comment: bool,
    comment_tail: [char; 2],
    tag: String,
    skip: Option<&'static str>,
    ent: Option<String>,
    pre: bool,
    links: Vec<Option<String>>,
    lists: Vec<Option<u32>>,
    cell: u32,
    pending_break: u8,
    pending_space: bool,
    wrote: bool,
}

impl Md {
    fn feed(&mut self, s: &str, out: &mut String) {
        for c in s.chars() {
            self.ch(c, out);
        }
    }

    fn finish(&mut self, out: &mut String) {
        if self.lt && self.skip.is_none() {
            self.lt = false;
            self.text('<', out);
        }
        if let Some(e) = self.ent.take() {
            self.text('&', out);
            for c in e.chars() {
                self.text(c, out);
            }
        }
    }

    fn ch(&mut self, c: char, out: &mut String) {
        if self.in_comment {
            if c == '>' && self.comment_tail == ['-', '-'] {
                self.in_comment = false;
                self.in_tag = false;
            }
            self.comment_tail = [self.comment_tail[1], c];
            return;
        }
        if self.in_tag {
            if c == '<' {
                self.in_tag = false;
                self.lt = true;
            } else if c == '>' {
                self.in_tag = false;
                let t = std::mem::take(&mut self.tag);
                self.dispatch(&t, out);
            } else {
                self.tag.push(c);
                if self.tag == "!--" {
                    self.in_comment = true;
                    self.comment_tail = [' ', ' '];
                    self.tag.clear();
                }
            }
            return;
        }
        if self.lt {
            self.lt = false;
            if c.is_ascii_alphabetic() || c == '/' || c == '!' || c == '?' {
                self.in_tag = true;
                self.tag.clear();
                self.ch(c, out);
                return;
            }
            if self.skip.is_none() {
                self.text('<', out);
            }
        }
        if c == '<' {
            if let Some(b) = self.ent.take() {
                self.text('&', out);
                for x in b.chars() {
                    self.text(x, out);
                }
            }
            self.lt = true;
            return;
        }
        if self.skip.is_some() {
            return;
        }
        if let Some(buf) = &mut self.ent {
            if c == ';' {
                let b = self.ent.take().unwrap_or_default();
                match entity(&b) {
                    Some(d) => self.text(d, out),
                    None => {
                        self.text('&', out);
                        for x in b.chars() {
                            self.text(x, out);
                        }
                        self.text(';', out);
                    }
                }
                return;
            }
            if (c.is_ascii_alphanumeric() || c == '#') && buf.len() < 10 {
                buf.push(c);
                return;
            }
            let b = self.ent.take().unwrap_or_default();
            self.text('&', out);
            for x in b.chars() {
                self.text(x, out);
            }
        }
        if c == '&' {
            self.ent = Some(String::new());
            return;
        }
        self.text(c, out);
    }

    /// One decoded character of page text.
    fn text(&mut self, c: char, out: &mut String) {
        if self.pre {
            self.flush_break(out);
            out.push(c);
            self.wrote = true;
            return;
        }
        if c.is_whitespace() {
            self.pending_space = true;
            return;
        }
        self.lead(out);
        out.push(c);
    }

    fn flush_break(&mut self, out: &mut String) {
        if self.wrote {
            for _ in 0..self.pending_break {
                out.push('\n');
            }
        }
        self.pending_break = 0;
    }

    /// Write whatever must precede new inline content: a pending
    /// line/paragraph break, else a pending single space.
    fn lead(&mut self, out: &mut String) {
        if self.pending_break > 0 {
            self.flush_break(out);
        } else if self.pending_space && self.wrote {
            out.push(' ');
        }
        self.pending_space = false;
        self.wrote = true;
    }

    fn put(&mut self, s: &str, out: &mut String) {
        self.lead(out);
        out.push_str(s);
    }

    /// Closing marker: glued to the text before it; pending spacing and
    /// breaks still follow it.
    fn put_close(&mut self, s: &str, out: &mut String) {
        out.push_str(s);
        self.wrote = true;
    }

    fn brk(&mut self, n: u8) {
        self.pending_break = self.pending_break.max(n);
    }

    fn dispatch(&mut self, tag: &str, out: &mut String) {
        let closing = tag.starts_with('/');
        let body = tag.trim_start_matches('/');
        let name: String = body
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if let Some(sk) = self.skip {
            if closing && name == sk {
                self.skip = None;
            }
            return;
        }
        let self_closing = tag.ends_with('/');
        if !closing
            && !self_closing
            && let Some(sk) = SKIPPED.iter().find(|n| **n == name)
        {
            self.skip = Some(sk);
            return;
        }
        if closing {
            self.close(&name, out);
        } else if self_closing && name == "pre" {
            // an empty preformatted block: nothing to fence, and no
            // closing tag will come to leave pre mode
        } else {
            self.open(&name, tag, out);
            if self_closing {
                // `<x/>` opens and closes at once, so no state (link,
                // list, emphasis) outlives the tag
                self.close(&name, out);
            }
        }
    }

    fn open(&mut self, name: &str, tag: &str, out: &mut String) {
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.brk(2);
                let n = (name.as_bytes()[1] - b'0') as usize;
                self.put(&format!("{} ", "#".repeat(n)), out);
            }
            "title" => {
                self.brk(2);
                self.put("# ", out);
            }
            "p" | "blockquote" | "table" | "figure" | "details" | "dl" => {
                self.brk(2);
                if name == "blockquote" {
                    self.put("> ", out);
                }
            }
            "div" | "section" | "article" | "main" | "header" | "footer" | "nav" | "aside"
            | "form" | "fieldset" | "address" | "summary" | "figcaption" | "dt" | "dd" => {
                self.brk(1)
            }
            "br" => self.brk(1),
            "hr" => {
                self.brk(2);
                self.put("---", out);
                self.brk(2);
            }
            "ul" | "ol" => {
                self.brk(if self.lists.is_empty() { 2 } else { 1 });
                self.lists.push((name == "ol").then_some(0));
            }
            "li" => {
                self.brk(1);
                // Indent clamp and depth number: see INDENT_STOPS_AT.
                let d = self.lists.len().saturating_sub(1);
                let pad = "  ".repeat(d.min(INDENT_STOPS_AT));
                let mark = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{n}. ")
                    }
                    _ => "- ".to_string(),
                };
                let depth = if d > INDENT_STOPS_AT {
                    format!("[depth {}] ", d + 1)
                } else {
                    String::new()
                };
                self.put(&format!("{pad}{mark}{depth}"), out);
            }
            "pre" => {
                self.brk(2);
                self.put("```\n", out);
                self.pre = true;
            }
            "code" if !self.pre => self.put("`", out),
            "b" | "strong" if !self.pre => self.put("**", out),
            "i" | "em" if !self.pre => self.put("*", out),
            "a" => {
                let href = attr(tag, "href");
                if href.is_some() {
                    self.put("[", out);
                }
                self.links.push(href);
            }
            "img" => {
                if let Some(src) = attr(tag, "src") {
                    let alt = attr(tag, "alt").unwrap_or_default();
                    self.put(&format!("![{alt}]({src})"), out);
                }
            }
            "tr" => {
                self.brk(1);
                self.cell = 0;
            }
            "td" | "th" => {
                if self.cell > 0 {
                    self.pending_space = false;
                    self.put_close(" | ", out);
                }
                self.cell += 1;
            }
            _ => {}
        }
    }

    fn close(&mut self, name: &str, out: &mut String) {
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "title" | "p" | "blockquote" | "table"
            | "figure" | "details" | "dl" => self.brk(2),
            "div" | "section" | "article" | "main" | "header" | "footer" | "nav" | "aside"
            | "form" | "fieldset" | "address" | "summary" | "figcaption" | "dt" | "dd" | "li"
            | "tr" => self.brk(1),
            "ul" | "ol" => {
                self.lists.pop();
                self.brk(if self.lists.is_empty() { 2 } else { 1 });
            }
            "pre" => {
                if self.pre {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```");
                    self.pre = false;
                    self.brk(2);
                }
            }
            "code" if !self.pre => self.put_close("`", out),
            "b" | "strong" if !self.pre => self.put_close("**", out),
            "i" | "em" if !self.pre => self.put_close("*", out),
            "a" => {
                if let Some(Some(h)) = self.links.pop() {
                    self.put_close(&format!("]({h})"), out);
                }
            }
            _ => {}
        }
    }
}

/// Value of attribute `name` in a raw tag body (quoted or bare), entities
/// decoded.
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at > 0 && lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let v = rest[1..].trim_start();
        let raw = match v.chars().next()? {
            q @ ('"' | '\'') => v[1..].split(q).next().unwrap_or(""),
            _ => v
                .split(|c: char| c.is_whitespace() || c == '/' && v.ends_with('/'))
                .next()
                .unwrap_or(""),
        };
        return Some(decode_entities(raw));
    }
    None
}

fn decode_entities(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        match rest
            .find(';')
            .and_then(|j| entity(&rest[1..j]).map(|c| (j, c)))
        {
            Some((j, c)) => {
                out.push(c);
                rest = &rest[j + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decode the inside of `&...;`.
fn entity(b: &str) -> Option<char> {
    match b {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => {
            let n = b.strip_prefix('#')?;
            let v = match n.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(v)
        }
    }
}

/// Only the scheme is checked: http and https are the protocols this tool
/// speaks. Hosts are not filtered; this is not a security boundary.
fn check_url(url: &str) -> Result<(), String> {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .map(|_| ())
        .ok_or_else(|| "webfetch: only http and https".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{DefaultHasher, Hasher};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn refuses_only_non_http_schemes() {
        assert!(check_url("ftp://x").is_err());
        assert!(check_url("file:///etc/passwd").is_err());
        assert!(check_url("https://example.com/a").is_ok());
        // Not a boundary: any host is accepted, loopback and private included.
        assert!(check_url("http://localhost/x").is_ok());
        assert!(check_url("http://127.0.0.1/").is_ok());
        assert!(check_url("http://10.0.0.1/").is_ok());
    }

    fn md(html: &str) -> String {
        let mut out = String::new();
        let mut s = Sink::new(true);
        s.feed(html, &mut out);
        s.finish(&mut out);
        // chunk-edge invariance: one char at a time gives the same text
        let mut out2 = String::new();
        let mut s2 = Sink::new(true);
        for ch in html.chars() {
            s2.feed(&ch.to_string(), &mut out2);
        }
        s2.finish(&mut out2);
        assert_eq!(out, out2, "per-char feed differs for {html:?}");
        out
    }

    #[test]
    fn style_script_noscript_template_text_removed() {
        let h = "<html><head><style>p{color:red}</style><script>if(a<b){x()}</script>\
                 </head><body><noscript>enable js</noscript><template><b>t</b></template>\
                 <p>Hi <b>there</b></p><script>a<b; y</script>after";
        let t = md(h);
        assert_eq!(t, "Hi **there**\n\nafter");
        for bad in ["color", "x()", "enable", "<b>", "y"] {
            assert!(!t.contains(bad), "{bad} leaked: {t:?}");
        }
    }

    #[test]
    fn markdown_shape_of_a_small_page() {
        let h = r#"<!doctype html><html><head><title>My &amp; Page</title></head><body>
<!-- note -> hidden -->
<h1>Top</h1>
<p>See <a href="https://e.com/a?x=1&amp;y=2">the <em>docs</em></a> and <code>f()</code>.
Next   line &lt;ok&gt; &#169; x < y</p>
<ul><li>one</li><li>two<ol><li>sub</li></ol></li></ul>
<h2>Sec</h2><pre>fn a() {
    1
}</pre>
<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>
<img src="/i.png" alt="pic"><hr>
</body></html>"#;
        let want = "# My & Page\n\n# Top\n\n\
See [the *docs*](https://e.com/a?x=1&y=2) and `f()`. Next line <ok> \u{a9} x < y\n\n\
- one\n- two\n  1. sub\n\n\
## Sec\n\n```\nfn a() {\n    1\n}\n```\n\n\
A | B\n1 | 2\n\n\
![pic](/i.png)\n\n---";
        assert_eq!(md(h), want);
    }

    #[test]
    fn plain_text_passes_through_untouched() {
        let mut out = String::new();
        let mut s = Sink::new(false);
        s.feed("a <b>  c\n", &mut out);
        s.finish(&mut out);
        assert_eq!(out, "a <b>  c\n");
    }

    #[test]
    fn decode_lossy_across_chunk_edges() {
        let src = "a\u{e9}\u{4e16}\u{1f600}z".as_bytes();
        let mut out = String::new();
        let mut carry: Vec<u8> = Vec::new();
        for b in src {
            carry.push(*b);
            let used = decode_lossy(&carry, false, |t| out.push_str(t));
            carry.drain(..used);
        }
        decode_lossy(&carry, true, |t| out.push_str(t));
        assert_eq!(out, "a\u{e9}\u{4e16}\u{1f600}z");
    }

    const MIB: usize = 1 << 20;

    fn body(n: usize) -> Vec<u8> {
        // deterministic, pseudo-random ascii, lines so whitespace is kept
        let mut v = Vec::with_capacity(n);
        let mut x: u64 = 0x9E3779B97F4A7C15;
        while v.len() < n {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            v.extend_from_slice(format!("{x:016x}\n").as_bytes());
        }
        v.truncate(n);
        v
    }

    fn hash(b: &[u8]) -> u64 {
        let mut h = DefaultHasher::new();
        h.write(b);
        h.finish()
    }

    fn serve(n: usize) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut req = [0u8; 4096];
            let _ = s.read(&mut req);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n"
            );
            s.write_all(head.as_bytes()).unwrap();
            let chunk = body(1 << 20);
            let mut left = n;
            // stream from a small reused buffer so the server adds no RSS growth
            while left > 0 {
                let k = left.min(chunk.len());
                s.write_all(&chunk[..k]).unwrap();
                left -= k;
            }
        });
        format!("http://127.0.0.1:{port}/")
    }

    /// Serve each connection with `route(path) -> raw HTTP response`.
    fn serve_routes(route: fn(&str) -> String) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for conn in l.incoming() {
                let mut s = conn.unwrap();
                let mut req = [0u8; 4096];
                let n = s.read(&mut req).unwrap_or(0);
                let line = String::from_utf8_lossy(&req[..n]).to_string();
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                let _ = s.write_all(route(&path).as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn header_states_final_url_and_status() {
        fn route(p: &str) -> String {
            match p {
                "/start" => "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
                "/final" => {
                    let b = "<h1>Hi</h1><style>x{}</style>";
                    format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
                }
                _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            }
        }
        let base = serve_routes(route);
        let out = fetch_url(&format!("{base}/start")).unwrap();
        assert_eq!(out, format!("url: {base}/final | status: 200 OK\n\n# Hi"));
        let err = fetch_url(&format!("{base}/nope")).unwrap_err();
        assert!(err.contains("404"), "{err}");
    }

    #[test]
    fn the_tool_fetches_a_loopback_server_and_refuses_other_schemes() {
        fn route(_: &str) -> String {
            let b = "<h1>Local</h1>";
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}",
                b.len()
            )
        }
        let base = serve_routes(route);
        let out = WebFetch
            .execute(&serde_json::json!({"url": format!("{base}/x")}))
            .unwrap();
        assert_eq!(out, format!("url: {base}/x | status: 200 OK\n\n# Local"));
        assert!(fetch_url("file:///etc/passwd").is_err());
    }

    /// An HTML page of `reps` identical paragraphs: whole, in order, hashed.
    fn check_html(reps: usize) {
        const UNIT: &str = "<p>Hello <b>world</b> &amp; <a href=\"/x\">y</a></p>\n";
        let n = UNIT.len() * reps;
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut req = [0u8; 4096];
            let _ = s.read(&mut req);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n"
            );
            s.write_all(head.as_bytes()).unwrap();
            let chunk = UNIT.repeat(10_000);
            let mut left = reps;
            while left > 0 {
                let k = left.min(10_000);
                s.write_all(&chunk.as_bytes()[..k * UNIT.len()]).unwrap();
                left -= k;
            }
        });
        let url = format!("http://127.0.0.1:{port}/");
        let out = fetch_url(&url).unwrap();
        let text = out.split_once("\n\n").unwrap().1;
        let one = "Hello **world** & [y](/x)";
        let want_len = one.len() * reps + 2 * (reps - 1);
        assert_eq!(text.len(), want_len, "whole html page, no cut");
        let unit = format!("{one}\n\n");
        let mut h = DefaultHasher::new();
        for _ in 0..reps - 1 {
            h.write(unit.as_bytes());
        }
        h.write(one.as_bytes());
        assert_eq!(hash(text.as_bytes()), h.finish());
    }

    #[test]
    fn whole_html_page_5mb() {
        check_html(5 * MIB / 50);
    }

    fn expected(n: usize) -> Vec<u8> {
        let chunk = body(1 << 20);
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            let k = (n - v.len()).min(chunk.len());
            v.extend_from_slice(&chunk[..k]);
        }
        v
    }

    #[cfg(target_os = "linux")]
    fn hwm_kib() -> usize {
        let s = std::fs::read_to_string("/proc/self/status").unwrap();
        let l = s.lines().find(|l| l.starts_with("VmHWM:")).unwrap();
        l.split_whitespace().nth(1).unwrap().parse().unwrap()
    }

    fn check(n: usize) -> usize {
        let want = expected(n);
        let (want_len, want_hash) = (want.len(), hash(&want));
        drop(want);
        let url = serve(n);
        let out = fetch_url(&url).unwrap();
        let text = out.split_once("\n\n").unwrap().1;
        assert_eq!(text.len(), want_len, "whole body, no cut");
        assert_eq!(hash(text.as_bytes()), want_hash);
        assert!(!text.contains("[truncated]"));
        n
    }

    #[test]
    fn whole_body_not_truncated() {
        check(5 * MIB);
    }

    /// The returned String is the one copy of the body (a whole result
    /// cannot be smaller); nothing else holds it. Old buffering held ~3x.
    /// One test so parallel 50 MB tests cannot pollute the process peak.
    #[cfg(target_os = "linux")]
    #[test]
    fn whole_50mb_pages_one_copy_in_memory() {
        let n = 50 * MIB;
        let base = hwm_kib();
        check(n);
        check_html(n / 50);
        let grown = hwm_kib().saturating_sub(base) * 1024;
        assert!(
            grown < n + n / 2 + 16 * MIB,
            "peak RSS grew {} MiB for a {} MiB body",
            grown / MIB,
            n / MIB
        );
    }

    /// Serve `200 text/plain` as an endless body (`gap` between writes of
    /// `chunk` bytes); stops by itself after `give_up` so a missing
    /// deadline fails the test instead of hanging it.
    fn serve_endless(
        chunk: usize,
        gap: std::time::Duration,
        give_up: std::time::Duration,
    ) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut req = [0u8; 4096];
            let _ = s.read(&mut req);
            let head = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n";
            if s.write_all(head.as_bytes()).is_err() {
                return;
            }
            let data = vec![b'x'; chunk];
            let t0 = std::time::Instant::now();
            while t0.elapsed() < give_up {
                if s.write_all(&data).is_err() {
                    return;
                }
                std::thread::sleep(gap);
            }
        });
        format!("http://127.0.0.1:{port}/")
    }

    #[test]
    fn trickle_body_hits_the_whole_body_deadline() {
        use std::time::Duration;
        // one byte per 50 ms: every read is quick, only the total is long
        let url = serve_endless(1, Duration::from_millis(50), Duration::from_secs(5));
        let t0 = std::time::Instant::now();
        let err = fetch_with(&url, Duration::from_millis(600)).unwrap_err();
        assert!(err.contains("body not complete within"), "{err}");
        assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    }

    #[test]
    fn never_ending_body_hits_the_whole_body_deadline() {
        use std::time::Duration;
        // 16 KiB every 2 ms (~8 MB/s): fast, never ends
        let url = serve_endless(16 * 1024, Duration::from_millis(2), Duration::from_secs(5));
        let t0 = std::time::Instant::now();
        let err = fetch_with(&url, Duration::from_millis(600)).unwrap_err();
        assert!(err.contains("body not complete within"), "{err}");
        assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    }

    #[test]
    fn self_closed_pre_does_not_swallow_the_page() {
        assert_eq!(md("<pre/><p>a  b</p><p>c</p>"), "a b\n\nc");
        assert_eq!(md("<pre />x   y"), "x y");
        assert_eq!(md("<a href=\"/u\"/>t"), "[](/u)t"); // closed at once: no link state leaks
    }

    #[test]
    fn one_slow_read_cannot_overrun_the_deadline() {
        use std::time::Duration;
        // One byte every 1.5 s, each read well inside the 2 s per-read
        // timeout; a check only between reads would end at ~3 s.
        let url = serve_endless(1, Duration::from_millis(1500), Duration::from_secs(8));
        let t0 = std::time::Instant::now();
        let err = fetch_with(&url, Duration::from_secs(2)).unwrap_err();
        assert!(err.contains("body not complete within"), "{err}");
        let el = t0.elapsed();
        assert!(el < Duration::from_millis(2500), "overran: {el:?}");
    }

    #[test]
    fn deeply_nested_lists_stay_linear_and_keep_every_item() {
        let n = 5_000;
        let html: String = (0..n)
            .map(|i| format!("<ul><li>item-number-{i:05}-padding-text"))
            .collect();
        assert!(html.len() > 100_000);
        let out = md(&html);
        assert!(
            out.len() < 2 * html.len(),
            "output {} for input {}",
            out.len(),
            html.len()
        );
        for i in 0..n {
            assert_eq!(
                out.matches(&format!("item-number-{i:05}-padding-text"))
                    .count(),
                1
            );
        }
        // the true depth survives the indent clamp, as a number
        assert!(
            out.contains("- [depth 5000] item-number-04999"),
            "{}",
            &out[out.len() - 200..]
        );
        assert!(out.starts_with("- item-number-00000"));
        assert!(out.contains("\n  - item-number-00001"));
        // time is linear: 4x the input takes about 4x, not 16x
        let t = |k: usize| {
            let h: String = (0..k).map(|i| format!("<ul><li>i{i}")).collect();
            let t0 = std::time::Instant::now();
            let _ = md(&h);
            t0.elapsed().as_secs_f64()
        };
        let (a, b) = (t(2_500).max(1e-4), t(10_000));
        assert!(b / a < 10.0, "x4 input took x{:.1}", b / a);
    }
}
