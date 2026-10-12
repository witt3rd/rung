//! Spilling a huge `web_fetch` body to a workspace file, and reading part
//! of a file back (`docs/rung-host-web-fetch-spill.md`).
//!
//! A body below the threshold is the tool result, as ever. A body at or
//! above it is streamed, as it arrives, to a file under the reserved
//! [`SPILL_DIR`] of the workspace and the tool result is a short notice.
//! Nothing is cut: the file holds the body whole, the threshold decides only
//! where it goes, and a physical disk limit is reported as a write failure
//! in words (the body is then returned whole in the result).
//!
//! The file is workspace content: it holds the text exactly as the fetch
//! returned it. The record never holds it, and every outgoing view of the
//! tool results (the request text, the doors, the streams) passes through
//! the host's redactor, as for any other tool result.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::toolbox::confine;

/// The reserved workspace directory that holds spilled bodies.
pub const SPILL_DIR: &str = ".web";

/// A body of at least this many bytes is spilled (32 KiB, about 8k tokens).
pub const DEFAULT_SPILL_BYTES: usize = 32 * 1024;

/// Where a fetch's pieces go: the header line once, then the body in order.
pub trait BodySink {
    fn header(&mut self, header: &str);
    fn body(&mut self, text: &str) -> Result<(), String>;
}

/// `1234567` as `1,234,567`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The next temp-name counter of this process.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// The temporary name, in the spill directory, of the file being written.
fn temp_name() -> String {
    format!(
        ".partial-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Remove temp files left by processes that no longer exist.
fn sweep_stale(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        let Some(rest) = name.strip_prefix(".partial-") else {
            continue;
        };
        let Some(pid) = rest.split('-').next().and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if pid != std::process::id() && !Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Set when the caller stops waiting for a fetch; the fetch then stops
/// writing and removes its own temp file.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Cancels on drop.
pub struct CancelOnDrop(pub Cancel);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

struct Open {
    file: File,
    path: PathBuf,
    /// Bytes known written.
    written: u64,
    hash: Sha256,
    lines: u64,
    last: u8,
}

/// Gathers a fetched body: in memory until it reaches the threshold, then in
/// a file. See the module docs.
pub struct Spiller {
    workspace: PathBuf,
    threshold: usize,
    /// Whether this turn may spill at all (it can read the file back).
    allowed: bool,
    header: Option<String>,
    buf: String,
    open: Option<Open>,
    /// Why spilling failed, once it has; the body is then kept in memory.
    failed: Option<String>,
    cancel: Cancel,
    #[cfg(test)]
    fault_after: Option<u64>,
}

/// What a finished fetch came to.
pub struct Spilled {
    /// The tool result text.
    pub text: String,
    /// A spill failure to record (the body is in `text` whole).
    pub failure: Option<String>,
    /// The workspace-relative path, when the body was spilled.
    pub path: Option<String>,
}

impl Spiller {
    pub fn new(workspace: PathBuf, threshold: usize, allowed: bool) -> Self {
        Self {
            workspace,
            threshold: threshold.max(1),
            allowed,
            header: None,
            buf: String::new(),
            open: None,
            failed: None,
            cancel: Cancel::default(),
            #[cfg(test)]
            fault_after: None,
        }
    }

    pub fn with_cancel(mut self, cancel: Cancel) -> Self {
        self.cancel = cancel;
        self
    }

    #[cfg(test)]
    pub fn fault_after(mut self, bytes: u64) -> Self {
        self.fault_after = Some(bytes);
        self
    }

    fn dir(&self) -> Result<PathBuf, String> {
        let probe = confine(&self.workspace, &format!("{SPILL_DIR}/x"))?;
        let dir = probe
            .parent()
            .map(Path::to_path_buf)
            .ok_or("no directory")?;
        match dir.symlink_metadata() {
            Ok(m) if m.file_type().is_dir() => Ok(dir),
            Ok(_) => Err(format!(
                "`{SPILL_DIR}` in your workspace is not a plain directory"
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&dir).map_err(|e| format!("mkdir {SPILL_DIR}: {e}"))?;
                Ok(dir)
            }
            Err(e) => Err(format!("{SPILL_DIR}: {e}")),
        }
    }

    fn start(&mut self) -> Result<(), String> {
        let dir = self.dir()?;
        sweep_stale(&dir);
        let path = dir.join(temp_name());
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("create: {e}"))?;
        let mut o = Open {
            file,
            path,
            written: 0,
            hash: Sha256::new(),
            lines: 0,
            last: b'\n',
        };
        let pending = std::mem::take(&mut self.buf);
        match self.write(&mut o, pending.as_bytes()) {
            Ok(()) => {
                self.open = Some(o);
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::remove_file(&o.path);
                self.buf = pending;
                Err(e)
            }
        }
    }

    fn write(&self, o: &mut Open, bytes: &[u8]) -> Result<(), String> {
        #[cfg(test)]
        if let Some(n) = self.fault_after
            && o.written + bytes.len() as u64 > n
        {
            return Err("write: No space left on device (os error 28)".into());
        }
        o.file.write_all(bytes).map_err(|e| format!("write: {e}"))?;
        o.written += bytes.len() as u64;
        o.hash.update(bytes);
        o.lines += bytes.iter().filter(|b| **b == b'\n').count() as u64;
        if let Some(b) = bytes.last() {
            o.last = *b;
        }
        Ok(())
    }

    /// Give up the file and keep the body in memory (read back what was
    /// written, then the chunk that failed is the caller's).
    fn fall_back(&mut self, why: String) -> Result<(), String> {
        self.failed = Some(why);
        let Some(o) = self.open.take() else {
            return Ok(());
        };
        let back = read_prefix(&o.path, o.written);
        let _ = std::fs::remove_file(&o.path);
        self.buf = back.map_err(|e| {
            format!("the page could not be saved and what was saved could not be read back: {e}")
        })?;
        Ok(())
    }

    /// The fetch ended well: the tool result.
    pub fn finish(mut self) -> Result<Spilled, String> {
        let header = self.header.take();
        let join = |header: &Option<String>, note: &str| match header {
            Some(h) => format!("{h}\n\n{note}"),
            None => note.to_string(),
        };
        let Some(mut o) = self.open.take() else {
            let note = self
                .failed
                .as_ref()
                .map(|f| {
                    format!("(the page could not be saved to a file: {f}; it follows whole)\n\n")
                })
                .unwrap_or_default();
            let text = join(&header, &format!("{note}{}", self.buf));
            return Ok(Spilled {
                text,
                failure: self.failed,
                path: None,
            });
        };
        let tidy = |o: &Open| {
            let _ = std::fs::remove_file(&o.path);
        };
        if self.cancel.is_set() {
            tidy(&o);
            return Err("the fetch was cut".into());
        }
        let placed = o
            .file
            .flush()
            .map_err(|e| format!("write: {e}"))
            .and_then(|_| self.place(&o));
        match placed {
            Ok((rel, bytes, lines)) => {
                tidy(&o);
                let note = format!(
                    "The page was large and is saved whole at {rel}\n({} bytes, {} lines). Read it with ws_read (all of it, or part with from_line/to_line or from_char/to_char).",
                    thousands(bytes),
                    thousands(lines)
                );
                Ok(Spilled {
                    text: join(&header, &note),
                    failure: None,
                    path: Some(rel),
                })
            }
            Err(why) => {
                // The whole body is in the temp file: read it back.
                let back = read_prefix(&o.path, o.written);
                tidy(&o);
                let body = back.map_err(|e| {
                    format!("the page could not be saved ({why}) and could not be read back: {e}")
                })?;
                let note =
                    format!("(the page could not be saved to a file: {why}; it follows whole)\n\n");
                o.written = 0;
                Ok(Spilled {
                    text: join(&header, &format!("{note}{body}")),
                    failure: Some(why),
                    path: None,
                })
            }
        }
    }

    /// Move the finished temp file to its counted, hashed name; never
    /// overwrite. Returns the relative path, bytes and lines.
    fn place(&self, o: &Open) -> Result<(String, u64, u64), String> {
        let dir = o.path.parent().ok_or("no directory")?.to_path_buf();
        let hash = format!("{:x}", o.hash.clone().finalize());
        let short = &hash[..6];
        let lines = o.lines + u64::from(o.last != b'\n');
        for _ in 0..1000 {
            let mut next = 1u64;
            for e in std::fs::read_dir(&dir).map_err(|e| format!("list: {e}"))? {
                let name = e.map_err(|e| format!("list: {e}"))?.file_name();
                let name = name.to_string_lossy();
                if let Some(n) = name.split('-').next().and_then(|d| d.parse::<u64>().ok()) {
                    next = next.max(n + 1);
                }
            }
            let name = format!("{next:04}-{short}.md");
            match std::fs::hard_link(&o.path, dir.join(&name)) {
                Ok(()) => return Ok((format!("{SPILL_DIR}/{name}"), o.written, lines)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("rename: {e}")),
            }
        }
        Err("no free file name".into())
    }

    /// The fetch failed: nothing is kept.
    pub fn abort(mut self) {
        if let Some(o) = self.open.take() {
            let _ = std::fs::remove_file(&o.path);
        }
    }
}

/// The first `n` bytes of a file as text.
fn read_prefix(path: &Path, n: u64) -> Result<String, String> {
    let mut v = Vec::with_capacity(n as usize);
    File::open(path)
        .and_then(|f| f.take(n).read_to_end(&mut v))
        .map_err(|e| e.to_string())?;
    String::from_utf8(v).map_err(|e| e.to_string())
}

impl BodySink for Spiller {
    fn header(&mut self, header: &str) {
        self.header = Some(header.to_string());
    }

    fn body(&mut self, text: &str) -> Result<(), String> {
        if self.cancel.is_set() {
            if let Some(o) = self.open.take() {
                let _ = std::fs::remove_file(&o.path);
            }
            return Err("the fetch was cut".into());
        }
        if let Some(mut o) = self.open.take() {
            match self.write(&mut o, text.as_bytes()) {
                Ok(()) => self.open = Some(o),
                Err(e) => {
                    self.open = Some(o);
                    self.fall_back(e)?;
                    self.buf.push_str(text);
                }
            }
            return Ok(());
        }
        self.buf.push_str(text);
        if self.allowed
            && self.failed.is_none()
            && self.buf.len() >= self.threshold
            && let Err(e) = self.start()
        {
            self.failed = Some(e);
        }
        Ok(())
    }
}

// ─── Reading part of a file ──────────────────────────────────────────────────

/// Which part of a file `ws_read` was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    Whole,
    /// 1-based, inclusive.
    Lines(u64, u64),
    /// 1-based Unicode scalar values, inclusive.
    Chars(u64, u64),
}

impl Range {
    /// The range an input asks for, or why it is not one.
    pub fn of(input: &serde_json::Value) -> Result<Range, String> {
        let get = |k: &str| -> Result<Option<u64>, String> {
            match input.get(k) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(v) => match v.as_u64() {
                    Some(n) if n >= 1 => Ok(Some(n)),
                    _ => Err(format!("`{k}` must be a whole number, 1 or more")),
                },
            }
        };
        let (fl, tl, fc, tc) = (
            get("from_line")?,
            get("to_line")?,
            get("from_char")?,
            get("to_char")?,
        );
        let lines = fl.is_some() || tl.is_some();
        let chars = fc.is_some() || tc.is_some();
        match (lines, chars) {
            (false, false) => Ok(Range::Whole),
            (true, true) => Err("ask for lines or characters, not both".into()),
            (true, false) => {
                let (a, b) = (fl.unwrap_or(1), tl.unwrap_or(u64::MAX));
                if b < a {
                    return Err("`to_line` is before `from_line`".into());
                }
                Ok(Range::Lines(a, b))
            }
            (false, true) => {
                let (a, b) = (fc.unwrap_or(1), tc.unwrap_or(u64::MAX));
                if b < a {
                    return Err("`to_char` is before `from_char`".into());
                }
                Ok(Range::Chars(a, b))
            }
        }
    }
}

/// `path` read as text, and the part `range` asks for with the file's totals.
/// Streamed in chunks; a range that reaches past the end gives what is there.
pub fn read_range(path: &Path, range: Range) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| format!("read: {e}"))?;
    let mut out = String::new();
    let (mut line, mut chars, mut last) = (1u64, 0u64, '\n');
    let mut carry: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let bad = || "read: stream did not contain valid UTF-8".to_string();
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        carry.extend_from_slice(&buf[..n]);
        let end = match std::str::from_utf8(&carry) {
            Ok(_) => carry.len(),
            Err(e) if e.error_len().is_none() && n > 0 => e.valid_up_to(),
            Err(_) => return Err(bad()),
        };
        let text = std::str::from_utf8(&carry[..end]).map_err(|_| bad())?;
        for c in text.chars() {
            chars += 1;
            let keep = match range {
                Range::Whole => true,
                Range::Lines(a, b) => line >= a && line <= b,
                Range::Chars(a, b) => chars >= a && chars <= b,
            };
            if keep {
                out.push(c);
            }
            if c == '\n' {
                line += 1;
            }
            last = c;
        }
        carry.drain(..end);
        if n == 0 {
            break;
        }
    }
    if range == Range::Whole {
        return Ok(out);
    }
    let lines = if chars == 0 {
        0
    } else {
        line - u64::from(last == '\n')
    };
    let head = match range {
        Range::Lines(a, b) => format!(
            "[lines {a}-{} of {}; {} characters in the file]",
            b.min(lines.max(a)),
            thousands(lines),
            thousands(chars)
        ),
        Range::Chars(a, b) => format!(
            "[characters {a}-{} of {}; {} lines in the file]",
            b.min(chars.max(a)),
            thousands(chars),
            thousands(lines)
        ),
        Range::Whole => unreachable!(),
    };
    Ok(format!("{head}\n{out}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbox::{HostTools, LiveWeb, NoWeb, READ, WEB_READ, WORKSPACE_WRITE, WebReader};
    use rung_std::tools::Toolset;
    use serde_json::{Value, json};
    use std::sync::Arc;

    /// Line `i` of a generated page: about 60 bytes, plain text.
    fn line(i: u64) -> String {
        format!("line {i:09} the quick brown fox jumps over the lazy dog\n")
    }

    /// The generated page of `bytes` or more (whole lines) and its digest.
    fn page_size(bytes: u64) -> (u64, u64, String) {
        let (mut n, mut size, mut h) = (0u64, 0u64, Sha256::new());
        while size < bytes {
            let l = line(n);
            size += l.len() as u64;
            h.update(l.as_bytes());
            n += 1;
        }
        (n, size, format!("{:x}", h.finalize()))
    }

    /// A local server that answers every GET with `lines` generated lines
    /// as text/plain, without ever holding the page.
    fn serve_lines(lines: u64) -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let total: u64 = (0..lines.min(1)).map(|i| line(i).len() as u64).sum::<u64>() * lines;
        std::thread::spawn(move || {
            for c in l.incoming() {
                let Ok(mut c) = c else { return };
                let mut b = [0u8; 2048];
                let _ = c.read(&mut b);
                let _ = write!(
                    c,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
                );
                let mut chunk = String::new();
                for i in 0..lines {
                    chunk.push_str(&line(i));
                    if chunk.len() > 60_000 {
                        if c.write_all(chunk.as_bytes()).is_err() {
                            break;
                        }
                        chunk.clear();
                    }
                }
                let _ = c.write_all(chunk.as_bytes());
            }
        });
        format!("http://{addr}/page")
    }

    fn tools(
        name: &str,
        enabled: &[&str],
        web: Arc<dyn WebReader>,
    ) -> (HostTools, crate::sim::TempDir) {
        let guard = crate::sim::temp_dir_guard(name);
        let sc = crate::sim::Scenario::new(guard.path(), 1);
        let (host, _, _) = crate::sim::build(sc);
        let t = HostTools::new(
            host.core.clone(),
            1,
            enabled.iter().map(|s| s.to_string()).collect(),
            0,
            None,
            web,
            guard.path().join("ws"),
        );
        (t, guard)
    }

    fn spilled_path(out: &str) -> String {
        let at = out.find(".web/").expect("a .web path in the notice");
        out[at..]
            .split(|c: char| c.is_whitespace())
            .next()
            .unwrap()
            .to_string()
    }

    fn sha_file(p: &Path) -> String {
        let mut h = Sha256::new();
        let mut f = File::open(p).unwrap();
        let mut b = vec![0u8; 1 << 16];
        loop {
            let n = f.read(&mut b).unwrap();
            if n == 0 {
                break;
            }
            h.update(&b[..n]);
        }
        format!("{:x}", h.finalize())
    }

    fn spill_whole(bytes: u64) {
        let (n, size, digest) = page_size(bytes);
        let url = serve_lines(n);
        let (t, g) = tools(
            &format!("spill-{bytes}"),
            &[READ, WEB_READ],
            Arc::new(LiveWeb::default()),
        );
        let out = t
            .execute("web_fetch", &json!({"url": url}))
            .expect("fetched");
        assert!(out.starts_with("url: http://127.0.0.1:"), "header: {out}");
        assert!(out.contains("status: 200 OK"));
        assert!(out.len() < 600, "a notice, not the page");
        assert!(out.contains(&format!(
            "({} bytes, {} lines)",
            thousands(size),
            thousands(n)
        )));
        let rel = spilled_path(&out);
        let file = g.path().join("ws").join(&rel);
        assert_eq!(std::fs::metadata(&file).unwrap().len(), size);
        assert_eq!(sha_file(&file), digest, "the file is the body, whole");
        let name = rel.strip_prefix(".web/").unwrap();
        assert!(name.starts_with("0001-") && name.ends_with(".md"), "{name}");
        assert!(name[5..11] == digest[..6], "hash in the name");
        // nothing else is left in the directory
        let left: Vec<_> = std::fs::read_dir(g.path().join("ws/.web"))
            .unwrap()
            .collect();
        assert_eq!(left.len(), 1);
        // a second fetch takes the next counter and overwrites nothing
        let again = t.execute("web_fetch", &json!({"url": url})).unwrap();
        assert!(spilled_path(&again).starts_with(".web/0002-"));
        assert!(file.exists());
    }

    #[test]
    fn a_5_mb_page_is_spilled_whole() {
        spill_whole(5_000_000);
    }

    #[test]
    fn a_50_mb_page_is_spilled_whole() {
        spill_whole(50_000_000);
    }

    #[test]
    fn a_page_below_the_threshold_stays_inline() {
        let (n, size, _) = page_size(30_000);
        assert!(size < DEFAULT_SPILL_BYTES as u64);
        let url = serve_lines(n);
        let (t, g) = tools(
            "spill-inline",
            &[READ, WEB_READ],
            Arc::new(LiveWeb::default()),
        );
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        let (head, body) = out.split_once("\n\n").unwrap();
        assert!(head.starts_with("url: "));
        assert_eq!(body.len() as u64, size);
        assert!(body.starts_with(&line(0)));
        assert!(!g.path().join("ws/.web").exists());
    }

    #[test]
    fn the_threshold_is_the_setting_and_counts_bytes() {
        let (n, size, _) = page_size(30_000);
        let url = serve_lines(n);
        let lw = |b| Arc::new(LiveWeb { spill_bytes: b });
        let (t, _g) = tools("spill-set-a", &[READ, WEB_READ], lw(size as usize));
        assert!(
            t.execute("web_fetch", &json!({"url": url}))
                .unwrap()
                .contains(".web/")
        );
        let (t, _g) = tools("spill-set-b", &[READ, WEB_READ], lw(size as usize + 1));
        assert!(
            !t.execute("web_fetch", &json!({"url": url}))
                .unwrap()
                .contains(".web/")
        );
    }

    #[test]
    fn without_the_read_group_the_whole_body_comes_back() {
        let (n, size, _) = page_size(100_000);
        let url = serve_lines(n);
        let (t, g) = tools("spill-noread", &[WEB_READ], Arc::new(LiveWeb::default()));
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        assert_eq!(out.split_once("\n\n").unwrap().1.len() as u64, size);
        assert!(!g.path().join("ws/.web").exists());
    }

    #[test]
    fn a_range_of_lines_and_of_characters_is_exactly_that_view() {
        let (n, size, _) = page_size(100_000);
        let url = serve_lines(n);
        let (t, _g) = tools(
            "spill-range",
            &[READ, WEB_READ],
            Arc::new(LiveWeb::default()),
        );
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        let rel = spilled_path(&out);
        let got = t
            .execute(
                "ws_read",
                &json!({"path": rel, "from_line": 3, "to_line": 4}),
            )
            .unwrap();
        assert_eq!(
            got,
            format!(
                "[lines 3-4 of {}; {} characters in the file]\n{}{}",
                thousands(n),
                thousands(size),
                line(2),
                line(3)
            )
        );
        let got = t
            .execute(
                "ws_read",
                &json!({"path": rel, "from_char": 6, "to_char": 14}),
            )
            .unwrap();
        assert!(got.ends_with("]\n000000000"), "{got}");
        let whole = t.execute("ws_read", &json!({"path": rel})).unwrap();
        assert_eq!(whole.len() as u64, size);
        assert!(
            t.execute(
                "ws_read",
                &json!({"path": rel, "from_line": 1, "from_char": 1})
            )
            .is_err()
        );
    }

    #[test]
    fn a_char_range_counts_characters_and_a_one_line_file_is_readable() {
        let (t, g) = tools("spill-chars", &[READ, WORKSPACE_WRITE], Arc::new(NoWeb));
        let text = "é世界x".repeat(30_000);
        std::fs::create_dir_all(g.path().join("ws")).unwrap();
        std::fs::write(g.path().join("ws/one.txt"), &text).unwrap();
        let got = t
            .execute(
                "ws_read",
                &json!({"path": "one.txt", "from_char": 2, "to_char": 6}),
            )
            .unwrap();
        assert_eq!(
            got,
            format!(
                "[characters 2-6 of {}; 1 lines in the file]\n世界xé世",
                thousands(120_000)
            )
        );
        let got = t
            .execute(
                "ws_read",
                &json!({"path": "one.txt", "from_line": 1, "to_line": 1}),
            )
            .unwrap();
        assert!(got.ends_with(&text));
    }

    #[test]
    fn a_failed_spill_returns_the_whole_body_in_words_with_an_event() {
        let (n, size, digest) = page_size(100_000);
        let url = serve_lines(n);
        let (t, g) = tools(
            "spill-fail",
            &[READ, WEB_READ],
            Arc::new(LiveWeb::default()),
        );
        std::fs::create_dir_all(g.path().join("ws")).unwrap();
        std::fs::write(g.path().join("ws/.web"), "a file").unwrap();
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        assert!(out.contains("could not be saved to a file"), "{out}");
        assert!(out.contains("not a plain directory"));
        let body = out.split_once("follows whole)\n\n").unwrap().1;
        assert_eq!(body.len() as u64, size);
        let mut h = Sha256::new();
        h.update(body.as_bytes());
        assert_eq!(format!("{:x}", h.finalize()), digest);
        let rec = std::fs::read_to_string(g.path().join("record.jsonl"))
            .or_else(|_| std::fs::read_to_string(g.path().join("state/record.jsonl")));
        if let Ok(rec) = rec {
            assert!(rec.contains("web.spill_failed"));
        }
        // a symlink as .web is refused too
        std::fs::remove_file(g.path().join("ws/.web")).unwrap();
        std::os::unix::fs::symlink("/tmp", g.path().join("ws/.web")).unwrap();
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        assert!(out.contains("leaves your workspace"), "{out}");
        assert!(out.contains("could not be saved to a file"));
    }

    #[test]
    fn an_unwritable_directory_is_answered_in_words() {
        use std::os::unix::fs::PermissionsExt;
        let (n, size, _) = page_size(100_000);
        let url = serve_lines(n);
        let (t, g) = tools(
            "spill-perm",
            &[READ, WEB_READ],
            Arc::new(LiveWeb::default()),
        );
        let d = g.path().join("ws/.web");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o555)).unwrap();
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        if out.contains(".web/0") {
            return; // running as a user the mode does not bind
        }
        assert!(out.contains("could not be saved to a file"), "{out}");
        assert_eq!(
            out.split_once("follows whole)\n\n").unwrap().1.len() as u64,
            size
        );
    }

    #[test]
    fn a_disk_full_mid_stream_returns_the_whole_body() {
        let (_, _, _) = page_size(1);
        let g = crate::sim::temp_dir_guard("spill-full");
        let ws = g.path().join("ws");
        let mut sp = Spiller::new(ws.clone(), 1000, true).fault_after(5000);
        sp.header("url: x | status: 200");
        let mut all = String::new();
        for i in 0..400 {
            let l = line(i);
            all.push_str(&l);
            sp.body(&l).unwrap();
        }
        let done = sp.finish().unwrap();
        assert!(done.failure.as_deref().unwrap().contains("No space left"));
        assert!(done.text.ends_with(&all), "the body is whole");
        assert!(done.text.contains("could not be saved to a file"));
        let left: Vec<_> = std::fs::read_dir(ws.join(".web")).unwrap().collect();
        assert!(left.is_empty(), "no partial file is left");
    }

    #[test]
    fn a_cut_fetch_stops_and_leaves_no_temp_and_a_second_is_whole() {
        let g = crate::sim::temp_dir_guard("spill-cut");
        let ws = g.path().join("ws");
        let cancel = Cancel::default();
        let mut a = Spiller::new(ws.clone(), 1000, true).with_cancel(cancel.clone());
        let mut b = Spiller::new(ws.clone(), 1000, true);
        let mut all = String::new();
        for i in 0..100 {
            let l = line(i);
            all.push_str(&l);
            a.body(&l).unwrap();
            b.body(&l).unwrap();
        }
        cancel.cancel();
        assert!(a.body(&line(100)).is_err());
        for i in 100..200 {
            let l = line(i);
            all.push_str(&l);
            b.body(&l).unwrap();
        }
        let done = b.finish().unwrap();
        let rel = spilled_path(&done.text);
        assert_eq!(std::fs::read_to_string(ws.join(rel)).unwrap(), all);
        let names: Vec<_> = std::fs::read_dir(ws.join(".web"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(!names[0].starts_with(".partial"));
    }

    #[test]
    fn a_dead_process_temp_is_swept_and_a_live_one_kept() {
        let g = crate::sim::temp_dir_guard("spill-stale");
        let ws = g.path().join("ws");
        let d = ws.join(".web");
        std::fs::create_dir_all(&d).unwrap();
        let dead = d.join(".partial-4294967294-0");
        let live = d.join(format!(".partial-{}-999999", std::process::id()));
        std::fs::write(&dead, "x").unwrap();
        std::fs::write(&live, "y").unwrap();
        let mut sp = Spiller::new(ws, 10, true);
        sp.body(&"z".repeat(20)).unwrap();
        assert!(!dead.exists());
        assert!(live.exists());
        sp.abort();
    }

    #[test]
    fn a_later_turn_without_read_is_told_where_the_page_is() {
        let (t, _g) = tools("spill-gate", &[WORKSPACE_WRITE], Arc::new(NoWeb));
        for tool in ["ws_read", "ws_list"] {
            let e = t
                .execute(tool, &json!({"path": ".web/0007-3fa91c.md"}))
                .unwrap_err();
            let v: Value = serde_json::from_str(&e).unwrap();
            assert_eq!(v["error"], "not enabled this turn");
            assert_eq!(v["path"], ".web/0007-3fa91c.md");
            assert!(
                v["note"]
                    .as_str()
                    .unwrap()
                    .contains("`read` group is needed")
            );
        }
        // other paths are refused generically; writes are unaffected
        let e = t.execute("ws_read", &json!({"path": "a.txt"})).unwrap_err();
        assert!(!e.contains("note"));
        assert!(
            t.execute("ws_write", &json!({"path": ".web/x.md", "text": "t"}))
                .is_ok()
        );
    }

    #[test]
    fn spilled_files_are_listed_and_nothing_removes_them() {
        let (n, _, _) = page_size(100_000);
        let url = serve_lines(n);
        let (t, _g) = tools(
            "spill-list",
            &[READ, WEB_READ],
            Arc::new(LiveWeb::default()),
        );
        t.execute("web_fetch", &json!({"url": url})).unwrap();
        assert!(t.execute("ws_list", &json!({})).unwrap().contains(".web"));
        assert_eq!(
            t.execute("ws_list", &json!({"path": ".web"}))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }

    /// Streaming keeps memory flat: the child process fetches a 100 MB page
    /// and its peak resident set grows by far less than the page.
    #[test]
    fn memory_stays_flat_while_spilling() {
        if std::env::var("RUNG_SPILL_RSS_CHILD").is_ok() {
            return;
        }
        let me = std::env::current_exe().unwrap();
        let out = std::process::Command::new(me)
            .args(["--exact", "spill::tests::rss_child", "--nocapture"])
            .env("RUNG_SPILL_RSS_CHILD", "1")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(
            out.status.success(),
            "{text}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(text.contains("rss-ok"), "{text}");
    }

    fn hwm_kb() -> u64 {
        let s = std::fs::read_to_string("/proc/self/status").unwrap();
        s.lines()
            .find_map(|l| l.strip_prefix("VmHWM:"))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap()
    }

    #[test]
    fn rss_child() {
        if std::env::var("RUNG_SPILL_RSS_CHILD").is_err() {
            return;
        }
        let (n, size, _) = page_size(100_000_000);
        let url = serve_lines(n);
        let (t, g) = tools("spill-rss", &[READ, WEB_READ], Arc::new(LiveWeb::default()));
        let before = hwm_kb();
        let out = t.execute("web_fetch", &json!({"url": url})).unwrap();
        let grew = hwm_kb().saturating_sub(before);
        let rel = spilled_path(&out);
        assert_eq!(
            std::fs::metadata(g.path().join("ws").join(rel))
                .unwrap()
                .len(),
            size
        );
        assert!(grew < 30_000, "peak grew {grew} kB for a 100 MB page");
        println!("rss-ok grew {grew} kB");
    }

    /// Twenty overlapping spills into one workspace, some cut part way:
    /// none overwrites another, every placed file is whole, no temp is left.
    #[test]
    fn overlapping_and_cut_spills_never_overwrite_each_other() {
        let g = crate::sim::temp_dir_guard("spill-overlap");
        let ws = g.path().join("ws");
        let hs: Vec<_> = (0..20u64)
            .map(|k| {
                let ws = ws.clone();
                std::thread::spawn(move || {
                    let cancel = Cancel::default();
                    let mut sp = Spiller::new(ws, 1000, true).with_cancel(cancel.clone());
                    let mut all = String::new();
                    for i in 0..3000u64 {
                        if k % 3 == 0 && i == 1500 {
                            cancel.cancel();
                        }
                        let l = format!("{k}:{}", line(i));
                        all.push_str(&l);
                        if sp.body(&l).is_err() {
                            sp.abort();
                            return None;
                        }
                    }
                    let done = sp.finish().unwrap();
                    Some((spilled_path(&done.text), all))
                })
            })
            .collect();
        let done: Vec<_> = hs.into_iter().filter_map(|h| h.join().unwrap()).collect();
        assert_eq!(done.len(), 13, "the 7 cut ones stop");
        let paths: std::collections::BTreeSet<_> = done.iter().map(|d| d.0.clone()).collect();
        assert_eq!(paths.len(), done.len(), "no two spills share a file");
        for (rel, all) in &done {
            assert_eq!(&std::fs::read_to_string(ws.join(rel)).unwrap(), all);
        }
        let n = std::fs::read_dir(ws.join(".web")).unwrap().count();
        assert_eq!(n, done.len(), "no temp and no stray file is left");
    }

    /// A temp of a live process that is not this one is never swept.
    #[test]
    fn a_live_foreign_process_temp_is_never_swept() {
        let g = crate::sim::temp_dir_guard("spill-foreign");
        let ws = g.path().join("ws");
        let d = ws.join(".web");
        std::fs::create_dir_all(&d).unwrap();
        let mut other = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let theirs = d.join(format!(".partial-{}-0", other.id()));
        std::fs::write(&theirs, "theirs").unwrap();
        let mut sp = Spiller::new(ws, 10, true);
        sp.body(&"z".repeat(20)).unwrap();
        sp.abort();
        let kept = theirs.exists();
        let _ = other.kill();
        let _ = other.wait();
        assert!(kept, "a live process's temp was swept");
    }

    /// The temp is created exclusively: a planted file or symlink at a temp
    /// name is never written through, and a failed start leaves no temp.
    #[test]
    fn the_temp_is_created_exclusively_and_a_failure_leaves_none() {
        let g = crate::sim::temp_dir_guard("spill-excl");
        let ws = g.path().join("ws");
        let d = ws.join(".web");
        std::fs::create_dir_all(&d).unwrap();
        let victim = g.path().join("victim");
        std::fs::write(&victim, "keep").unwrap();
        let from = NEXT.load(Ordering::Relaxed);
        for n in from..from + 500 {
            let l = d.join(format!(".partial-{}-{n}", std::process::id()));
            std::os::unix::fs::symlink(&victim, l).unwrap();
        }
        let mut sp = Spiller::new(ws.clone(), 10, true);
        let body = "y".repeat(50);
        sp.body(&body).unwrap();
        let done = sp.finish().unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
        assert!(done.text.ends_with(&body), "the body came back whole");
        assert!(done.failure.is_some_and(|f| f.contains("create")));
        // a failed write leaves no temp either
        let mut sp = Spiller::new(ws, 10, true).fault_after(5);
        sp.body(&body).unwrap();
        sp.finish().unwrap();
        let temps = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .filter(|e| !e.path().is_symlink())
            .count();
        assert_eq!(temps, 0);
    }
}
