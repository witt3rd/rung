//! `webfetch` — HTTP GET for the agent. Size-capped, http(s) only.

use super::Tool;
use super::fsutil;
use serde_json::Value;

const MAX_BYTES: usize = 512 * 1024;
const TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct WebFetch;

impl Tool for WebFetch {
    fn name(&self) -> &'static str {
        "webfetch"
    }
    fn description(&self) -> &'static str {
        "Fetch a http(s) URL and return text (HTML stripped to tags-as-space). Cap 512KiB."
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
    let bytes = resp.bytes().map_err(|e| format!("webfetch: {e}"))?;
    let slice = if bytes.len() > MAX_BYTES {
        &bytes[..MAX_BYTES]
    } else {
        &bytes
    };
    let text = String::from_utf8_lossy(slice);
    let body = if ctype.contains("html") {
        strip_html(&text)
    } else {
        text.into_owned()
    };
    let mut out = format!("url: {url}\nstatus: {status}\n\n{body}");
    if bytes.len() > MAX_BYTES {
        out.push_str("\n\n[truncated]");
    }
    Ok(out)
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

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut skip = false;
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
        let t = strip_html("<html><script>x()</script><p>Hi <b>there</b></p>");
        assert_eq!(t, "Hi there");
    }
}
