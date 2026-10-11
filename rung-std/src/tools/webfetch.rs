//! `webfetch` — HTTP GET for the agent. http(s) only; whole body, streamed.

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
        "Fetch a http(s) URL and return text (HTML stripped to tags-as-space). Returns the whole body."
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
    fetch_unguarded(url)
}

fn fetch_unguarded(url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
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
    let mut out = format!("url: {url}\nstatus: {status}\n\n");
    // Stream: each chunk is decoded and appended straight to `out`; the
    // body is never held a second time.
    let mut sink = Sink::new(html);
    let mut resp = resp;
    let mut buf = vec![0u8; 64 * 1024];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let n = resp.read(&mut buf).map_err(|e| format!("webfetch: {e}"))?;
        if n == 0 {
            break;
        }
        carry.extend_from_slice(&buf[..n]);
        let used = decode_lossy(&carry, false, |t| sink.feed(t, &mut out));
        carry.drain(..used);
    }
    decode_lossy(&carry, true, |t| sink.feed(t, &mut out));
    Ok(out)
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

/// Incremental text sink: plain passthrough, or HTML tag stripping with
/// whitespace collapsed to single spaces (state survives chunk edges).
struct Sink {
    html: bool,
    in_tag: bool,
    skip: bool,
    tag: String,
    pending_space: bool,
    wrote: bool,
}

impl Sink {
    fn new(html: bool) -> Self {
        Sink {
            html,
            in_tag: false,
            skip: false,
            tag: String::new(),
            pending_space: false,
            wrote: false,
        }
    }

    fn feed(&mut self, s: &str, out: &mut String) {
        if !self.html {
            out.push_str(s);
            return;
        }
        for c in s.chars() {
            if c == '<' {
                self.in_tag = true;
                self.tag.clear();
                continue;
            }
            if self.in_tag {
                if c == '>' {
                    let t = self.tag.to_ascii_lowercase();
                    if t.starts_with("script") {
                        self.skip = true;
                    }
                    if t.starts_with("/script") {
                        self.skip = false;
                    }
                    self.in_tag = false;
                } else {
                    self.tag.push(c);
                }
                continue;
            }
            if self.skip {
                continue;
            }
            if c.is_whitespace() {
                self.pending_space = self.wrote;
            } else {
                if self.pending_space {
                    out.push(' ');
                    self.pending_space = false;
                }
                out.push(c);
                self.wrote = true;
            }
        }
    }
}

pub fn check_url(url: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("webfetch: only http and https")?;
    let hostport = rest.split('/').next().unwrap_or("");
    let host = hostport
        .rsplit('@')
        .next()
        .unwrap_or(hostport)
        .split('%')
        .next()
        .unwrap_or(hostport);
    let h = if host.starts_with('[') {
        host.trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    } else {
        host.split(':').next().unwrap_or(host).to_ascii_lowercase()
    };
    if h == "localhost"
        || h == "127.0.0.1"
        || h == "::1"
        || h == "0.0.0.0"
        || h.ends_with(".localhost")
    {
        return Err("webfetch: refused loopback host".into());
    }
    if h.starts_with("10.") || h.starts_with("192.168.") || h.starts_with("169.254.") {
        return Err("webfetch: refused private host".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_loopback_and_non_http() {
        assert!(check_url("ftp://x").is_err());
        assert!(check_url("http://localhost/x").is_err());
        assert!(check_url("http://127.0.0.1/").is_err());
        assert!(check_url("https://example.com/a").is_ok());
    }

    #[test]
    fn html_strip_drops_tags() {
        let mut out = String::new();
        let mut s = Sink::new(true);
        // split mid-tag to exercise chunk-edge state
        s.feed("<html><scr", &mut out);
        s.feed("ipt>x()</script><p>Hi <b>the", &mut out);
        s.feed("re</b></p>", &mut out);
        assert_eq!(out, "Hi there");
    }

    use std::hash::{DefaultHasher, Hasher};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Reference: the pre-streaming implementation.
    fn old_strip_html(s: &str) -> String {
        let mut out = String::new();
        let (mut in_tag, mut skip) = (false, false);
        let mut tag = String::new();
        for c in s.chars() {
            if c == '<' {
                in_tag = true;
                tag.clear();
                continue;
            }
            if in_tag {
                if c == '>' {
                    let t = tag.to_ascii_lowercase();
                    if t.starts_with("script") {
                        skip = true;
                    }
                    if t.starts_with("/script") {
                        skip = false;
                    }
                    in_tag = false;
                } else {
                    tag.push(c);
                }
                continue;
            }
            if !skip {
                out.push(c);
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn html_sink_matches_old_strip_html() {
        let cases = [
            "<html><script>x()</script><p>Hi <b>there</b></p>",
            "  \n\t<p> a  b\n\nc </p>  ",
            "a<b>c</b> d <i> e </i>f",
            "x < y <b>z</b>",
            "<<b>q</b>>r",
            "<SCRIPT type=x>no</ScRiPt>yes",
            "<p>caf\u{e9}  \u{3000} \u{4e16}\u{754c}</p>",
            "",
            "   ",
        ];
        for c in cases {
            let mut out = String::new();
            let mut s = Sink::new(true);
            s.feed(c, &mut out);
            assert_eq!(out, old_strip_html(c), "{c:?}");
            let mut out2 = String::new();
            let mut s2 = Sink::new(true);
            for ch in c.chars() {
                s2.feed(&ch.to_string(), &mut out2);
            }
            assert_eq!(out2, out, "per-char {c:?}");
        }
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
        // deterministic, non-repeating-ish ascii, lines so whitespace is kept
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
        let out = fetch_unguarded(&url).unwrap();
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

    #[cfg(target_os = "linux")]
    #[test]
    fn whole_body_flat_memory() {
        let n = 50 * MIB;
        let base = hwm_kib();
        check(n);
        let grown = hwm_kib().saturating_sub(base) * 1024;
        assert!(
            grown < 3 * n,
            "peak RSS grew {} MiB for a {} MiB body",
            grown / MIB,
            n / MIB
        );
    }
}
