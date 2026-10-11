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
        "Fetch a http(s) URL and return text (HTML stripped to tags-as-space).. Returns the whole body."
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

/// [`fetch_url`] without the host check; exists so tests can reach a
/// loopback server. Not part of the tool surface.
#[doc(hidden)]
pub fn fetch_unguarded(url: &str) -> Result<String, String> {
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
            if c == '<' && !self.in_tag {
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
}
