//! The record: one append-only NDJSON stream, the host's single truth.
//!
//! - Each line is one canonical JSON object `{seq, at, kind, ...body}`.
//!   `seq` counts from 1 with no gap; `at` is the host clock in ms.
//! - Lines go to segment files `seg-NNNNNNNN.ndjson` under the record
//!   directory. A segment past its size limit is closed and the next one
//!   opened (rotation). The host never deletes or rewrites a segment.
//! - Every line is written with one `write` call as it happens, so a
//!   `kill -9` loses nothing already written. [`Record::sync`] fsyncs; the
//!   host calls it when a stimulus is accepted and when a turn ends (log
//!   before forget), and on halt.
//! - Opening a record after a crash cuts a torn last line (a partial write)
//!   and reports how many bytes it cut. Nothing else is ever cut.
//!
//! Appending is crate-private: nothing outside the host writes its record.
//! The kinds that carry the agent's own authority (`kernel.commit`,
//! `kernel.release`) and the host's settlement of an expectation
//! (`expectation.settled`) are refused by the generic append and go through
//! sealed entries built only in their own modules (gates G-c and G-e).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Map, Value};

use crate::canon;
use crate::clock::Millis;

/// Kinds only a sealed entry may write.
pub const SEALED_KINDS: &[&str] = &["kernel.commit", "kernel.release", "expectation.settled"];

/// Default segment size before rotation.
pub const SEGMENT_BYTES: u64 = 64 * 1024 * 1024;

/// One record line.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub seq: u64,
    pub at: Millis,
    pub kind: String,
    /// Every other field of the line.
    pub body: Map<String, Value>,
}

impl Line {
    pub fn get(&self, key: &str) -> &Value {
        self.body.get(key).unwrap_or(&Value::Null)
    }

    pub fn str(&self, key: &str) -> &str {
        self.get(key).as_str().unwrap_or("")
    }

    pub fn u64(&self, key: &str) -> u64 {
        self.get(key).as_u64().unwrap_or(0)
    }

    pub fn i64(&self, key: &str) -> i64 {
        self.get(key).as_i64().unwrap_or(0)
    }

    pub fn f64(&self, key: &str) -> f64 {
        self.get(key).as_f64().unwrap_or(0.0)
    }

    /// The line as one JSON object.
    pub fn to_value(&self) -> Value {
        let mut m = self.body.clone();
        m.insert("seq".into(), self.seq.into());
        m.insert("at".into(), self.at.into());
        m.insert("kind".into(), self.kind.clone().into());
        Value::Object(m)
    }

    /// The canonical text of the line, without its newline.
    pub fn text(&self) -> String {
        canon::string(&self.to_value())
    }

    pub fn parse(s: &str) -> Option<Line> {
        let Value::Object(mut m) = serde_json::from_str::<Value>(s).ok()? else {
            return None;
        };
        let seq = m.remove("seq")?.as_u64()?;
        let at = m.remove("at")?.as_i64()?;
        let kind = m.remove("kind")?.as_str()?.to_string();
        Some(Line {
            seq,
            at,
            kind,
            body: m,
        })
    }

    /// The line with every `wall_*` field removed at any depth: what a
    /// determinism check compares (wall-clock measurements differ by run).
    pub fn without_wall(&self) -> Value {
        strip_wall(&self.to_value())
    }
}

/// Sort every object's keys, at every depth.
pub fn sort_keys(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.sort_keys();
            for x in m.values_mut() {
                sort_keys(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(sort_keys),
        _ => {}
    }
}

fn strip_wall(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| !k.starts_with("wall_"))
                .map(|(k, v)| (k.clone(), strip_wall(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(strip_wall).collect()),
        other => other.clone(),
    }
}

struct Inner {
    file: File,
    segment: u32,
    size: u64,
    next_seq: u64,
}

/// An open record. See the module docs.
pub struct Record {
    dir: PathBuf,
    max_segment: u64,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Record").field("dir", &self.dir).finish()
    }
}

/// What opening found.
#[derive(Debug)]
pub struct Opened {
    pub record: Record,
    /// Every line already in the record, in order.
    pub lines: Vec<Line>,
    /// Bytes cut from a torn last line.
    pub torn_bytes: u64,
}

fn segment_path(dir: &Path, n: u32) -> PathBuf {
    dir.join(format!("seg-{n:08}.ndjson"))
}

fn segments(dir: &Path) -> io::Result<Vec<(u32, PathBuf)>> {
    let mut out = Vec::new();
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    for e in rd {
        let e = e?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if let Some(n) = name
            .strip_prefix("seg-")
            .and_then(|s| s.strip_suffix(".ndjson"))
            .and_then(|s| s.parse::<u32>().ok())
        {
            out.push((n, e.path()));
        }
    }
    out.sort();
    Ok(out)
}

fn parse_all(text: &str, into: &mut Vec<Line>) -> io::Result<()> {
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let line = Line::parse(l).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: {l:.80}", i + 1),
            )
        })?;
        into.push(line);
    }
    Ok(())
}

impl Record {
    /// Open (or create) the record in `dir`, cutting a torn last line.
    pub fn open(dir: impl AsRef<Path>) -> io::Result<Opened> {
        Self::open_with(dir, SEGMENT_BYTES)
    }

    pub fn open_with(dir: impl AsRef<Path>, max_segment: u64) -> io::Result<Opened> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        let segs = segments(&dir)?;
        let mut lines = Vec::new();
        let mut torn_bytes = 0;
        for (i, (_, path)) in segs.iter().enumerate() {
            let mut bytes = Vec::new();
            File::open(path)?.read_to_end(&mut bytes)?;
            if i + 1 == segs.len() {
                let keep = bytes
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                if keep < bytes.len() {
                    torn_bytes = (bytes.len() - keep) as u64;
                    bytes.truncate(keep);
                    let f = OpenOptions::new().write(true).open(path)?;
                    f.set_len(keep as u64)?;
                    f.sync_all()?;
                }
            }
            parse_all(&String::from_utf8_lossy(&bytes), &mut lines)?;
        }
        let segment = segs.last().map(|(n, _)| *n).unwrap_or(1);
        let path = segment_path(&dir, segment);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();
        let next_seq = lines.last().map(|l| l.seq + 1).unwrap_or(1);
        Ok(Opened {
            record: Record {
                dir,
                max_segment,
                inner: Mutex::new(Inner {
                    file,
                    segment,
                    size,
                    next_seq,
                }),
            },
            lines,
            torn_bytes,
        })
    }

    /// Every line in `dir`, tolerating a torn last line (not cut: read only).
    pub fn read_dir(dir: impl AsRef<Path>) -> io::Result<Vec<Line>> {
        let segs = segments(dir.as_ref())?;
        let mut lines = Vec::new();
        for (i, (_, path)) in segs.iter().enumerate() {
            let mut text = fs::read_to_string(path)?;
            if i + 1 == segs.len() && !text.ends_with('\n') {
                let keep = text.rfind('\n').map(|p| p + 1).unwrap_or(0);
                text.truncate(keep);
            }
            parse_all(&text, &mut lines)?;
        }
        Ok(lines)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The seq the next line will get.
    pub fn next_seq(&self) -> u64 {
        self.inner.lock().expect("record").next_seq
    }

    /// Append one line. Refuses the sealed kinds.
    pub(crate) fn append(&self, at: Millis, kind: &str, body: Value) -> Line {
        assert!(
            !SEALED_KINDS.contains(&kind),
            "`{kind}` is written only through its sealed entry"
        );
        self.write(at, kind, body)
    }

    /// Append several lines with one write syscall (consecutive seqs).
    pub(crate) fn append_many(&self, at: Millis, items: Vec<(&str, Value)>) -> Vec<Line> {
        for (k, _) in &items {
            assert!(
                !SEALED_KINDS.contains(k),
                "`{k}` is written only through its sealed entry"
            );
        }
        self.write_many(at, items)
    }

    /// Append a sealed kind. Only the sealed entry types call this.
    pub(crate) fn append_sealed(&self, at: Millis, kind: &'static str, body: Value) -> Line {
        debug_assert!(SEALED_KINDS.contains(&kind));
        self.write(at, kind, body)
    }

    fn make_line(seq: u64, at: Millis, kind: &str, body: Value) -> Line {
        let body = match body {
            Value::Object(m) => m,
            Value::Null => Map::new(),
            other => {
                let mut m = Map::new();
                m.insert("value".into(), other);
                m
            }
        };
        for k in ["seq", "at", "kind"] {
            assert!(!body.contains_key(k), "`{k}` is reserved in a record line");
        }
        // Sorted, as a replay parses it: a live line and its replay are
        // the same value, key order included.
        let mut body = Value::Object(body);
        sort_keys(&mut body);
        let Value::Object(body) = body else {
            unreachable!()
        };
        Line {
            seq,
            at,
            kind: kind.to_string(),
            body,
        }
    }

    fn write(&self, at: Millis, kind: &str, body: Value) -> Line {
        self.write_many(at, vec![(kind, body)]).remove(0)
    }

    fn write_many(&self, at: Millis, items: Vec<(&str, Value)>) -> Vec<Line> {
        let mut inner = self.inner.lock().expect("record");
        let mut lines = Vec::new();
        let mut text = Vec::new();
        for (i, (kind, body)) in items.into_iter().enumerate() {
            let line = Self::make_line(inner.next_seq + i as u64, at, kind, body);
            text.extend_from_slice(line.text().as_bytes());
            text.push(b'\n');
            lines.push(line);
        }
        if inner.size > 0 && inner.size + text.len() as u64 > self.max_segment {
            let _ = inner.file.sync_all();
            let next = inner.segment + 1;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(segment_path(&self.dir, next))
                .unwrap_or_else(|e| panic!("record: open segment {next}: {e}"));
            inner.file = file;
            inner.segment = next;
            inner.size = 0;
        }
        inner
            .file
            .write_all(&text)
            .unwrap_or_else(|e| panic!("record: write: {e}"));
        inner.size += text.len() as u64;
        inner.next_seq += lines.len() as u64;
        lines
    }

    /// fsync the open segment.
    pub fn sync(&self) {
        let inner = self.inner.lock().expect("record");
        inner
            .file
            .sync_data()
            .unwrap_or_else(|e| panic!("record: fsync: {e}"));
    }

    /// Bytes in every segment.
    pub fn bytes(&self) -> u64 {
        segments(&self.dir)
            .unwrap_or_default()
            .iter()
            .filter_map(|(_, p)| fs::metadata(p).ok())
            .map(|m| m.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("rung-host-record-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn lines_round_trip_and_seq_continues() {
        let d = dir("rt");
        let o = Record::open(&d).unwrap();
        o.record.append(5, "a", json!({"x": 1}));
        o.record.append(6, "b", json!({"y": [1, 2]}));
        drop(o);
        let o = Record::open(&d).unwrap();
        assert_eq!(o.lines.len(), 2);
        assert_eq!(o.lines[1].seq, 2);
        assert_eq!(o.lines[1].get("y"), &json!([1, 2]));
        assert_eq!(o.record.append(7, "c", json!({})).seq, 3);
        assert_eq!(o.lines[0].text(), r#"{"at":5,"kind":"a","seq":1,"x":1}"#);
    }

    #[test]
    fn a_torn_tail_is_cut_and_reported() {
        let d = dir("torn");
        let o = Record::open(&d).unwrap();
        o.record.append(1, "a", json!({}));
        drop(o);
        let mut f = OpenOptions::new()
            .append(true)
            .open(segment_path(&d, 1))
            .unwrap();
        f.write_all(br#"{"at":2,"kin"#).unwrap();
        assert_eq!(Record::read_dir(&d).unwrap().len(), 1);
        let o = Record::open(&d).unwrap();
        assert_eq!(o.torn_bytes, 12);
        assert_eq!(o.lines.len(), 1);
        assert_eq!(o.record.append(3, "b", json!({})).seq, 2);
        assert_eq!(Record::read_dir(&d).unwrap().len(), 2);
    }

    #[test]
    fn segments_rotate_and_read_in_order() {
        let d = dir("rot");
        let o = Record::open_with(&d, 100).unwrap();
        for i in 0..20 {
            o.record.append(i, "k", json!({"i": i}));
        }
        assert!(segments(&d).unwrap().len() > 3);
        let lines = Record::read_dir(&d).unwrap();
        assert_eq!(lines.len(), 20);
        assert!(lines.windows(2).all(|w| w[1].seq == w[0].seq + 1));
    }

    #[test]
    #[should_panic(expected = "sealed entry")]
    fn the_generic_append_refuses_sealed_kinds() {
        let d = dir("sealed");
        let o = Record::open(&d).unwrap();
        o.record.append(1, "kernel.commit", json!({}));
    }

    #[test]
    fn wall_fields_are_stripped_for_comparison() {
        let l = Line::parse(r#"{"seq":1,"at":2,"kind":"k","wall_us":9,"x":{"wall_ms":3,"y":1}}"#)
            .unwrap();
        assert_eq!(
            l.without_wall(),
            json!({"seq":1,"at":2,"kind":"k","x":{"y":1}})
        );
    }
}
