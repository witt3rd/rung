//! Persist role+text lines. An assistant line also keeps the turn's full
//! message sequence (tool-use, tool-result, final text) so the next turn
//! replays what the model actually did, not only what it said. A turn that
//! stopped without an answer keeps the steps that ran and records why it
//! stopped in `failure`, which is never replayed to the model.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use rung_std::llm::ChatMessage;

use crate::catalog::Kind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub role: String,
    pub text: String,
    /// The turn's messages after the user line, ending with the final
    /// assistant text. Absent on user lines and on older sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<ChatMessage>>,
    /// Why the turn stopped without an answer (error, refusal, interrupt).
    /// It is not something the model said; `text` is empty and `messages`
    /// holds only the steps that ran. Absent on answered turns and on older
    /// sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    /// The recalled-memory block shown after this user line's text on the
    /// turn that asked it (`memory::shown`), kept so every later turn
    /// replays that message byte for byte. It is quoted data, never the
    /// user's words: `text` stays what the user wrote. Absent on other lines,
    /// on turns that recalled nothing, and on older sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recalled: Option<String>,
}

impl Line {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            text: text.into(),
            messages: None,
            failure: None,
            recalled: None,
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            text: text.into(),
            messages: None,
            failure: None,
            recalled: None,
        }
    }

    /// A turn that stopped for `why` after the steps in `messages` ran.
    pub fn failed(why: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            role: "assistant".into(),
            text: String::new(),
            messages: Some(messages),
            failure: Some(why.into()),
            recalled: None,
        }
    }
}

impl PartialEq for Line {
    fn eq(&self, other: &Self) -> bool {
        self.role == other.role
            && self.text == other.text
            && self.failure == other.failure
            && self.recalled == other.recalled
            && serde_json::to_value(&self.messages).ok()
                == serde_json::to_value(&other.messages).ok()
    }
}

impl Eq for Line {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The session's own system text (ACP `session/new`
    /// `_meta.systemPrompt`). Kept here so a load, resume or fork in a new
    /// process sends the same system text, and the cached prefix holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default)]
    pub lines: Vec<Line>,
}

impl Session {
    pub fn new(id: impl Into<String>, kind: Kind, cwd: &Path) -> Self {
        Self {
            id: id.into(),
            kind: kind.as_str().into(),
            status: "new".into(),
            cwd: cwd.to_string_lossy().into_owned(),
            isolation_path: None,
            pid: Some(std::process::id()),
            system: None,
            lines: Vec::new(),
        }
    }

    pub fn kind(&self) -> Result<Kind, String> {
        Kind::parse(&self.kind)
    }
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    pub dir: PathBuf,
}

impl SessionStore {
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn in_cwd(cwd: &Path) -> Self {
        Self::at(cwd.join(".rung").join("sessions"))
    }

    /// The append-only log: one JSON record per line, fsynced.
    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        check_id(id)?;
        Ok(self.dir.join(format!("{id}.log")))
    }

    /// The whole-file form older versions wrote. Still read; never written.
    pub fn legacy_path(&self, id: &str) -> Result<PathBuf, String> {
        check_id(id)?;
        Ok(self.dir.join(format!("{id}.json")))
    }

    pub fn load(&self, id: &str) -> Result<Session, String> {
        match self.try_load(id)? {
            Some(s) => Ok(s),
            None => Err(format!("session {id}: not found")),
        }
    }

    pub fn try_load(&self, id: &str) -> Result<Option<Session>, String> {
        let p = self.path(id)?;
        if p.is_file() {
            let body = fs::read(&p).map_err(|e| format!("session {id}: {e}"))?;
            return replay(&body).map(|(s, _)| s).map(Some).map_err(|e| format!("session {id}: {e}"));
        }
        let lp = self.legacy_path(id)?;
        if lp.is_file() {
            let body = fs::read_to_string(&lp).map_err(|e| format!("session {id}: {e}"))?;
            return serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| format!("session {id}: {e}"));
        }
        Ok(None)
    }

    pub fn list(&self) -> Result<Vec<Session>, String> {
        let mut out = Vec::new();
        if !self.dir.is_dir() {
            return Ok(out);
        }
        let mut ids = std::collections::BTreeSet::new();
        let ents = fs::read_dir(&self.dir).map_err(|e| format!("sessions dir: {e}"))?;
        for ent in ents {
            let p = ent.map_err(|e| format!("sessions dir: {e}"))?.path();
            if !matches!(p.extension().and_then(|e| e.to_str()), Some("json" | "log")) {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                ids.insert(stem.to_string());
            }
        }
        for id in ids {
            if let Ok(Some(s)) = self.try_load(&id) {
                out.push(s);
            }
        }
        Ok(out)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        for p in [self.path(id)?, self.legacy_path(id)?] {
            if p.is_file() {
                fs::remove_file(&p).map_err(|e| format!("session {id}: {e}"))?;
            }
        }
        Ok(())
    }

    /// Append what changed since the log's last state, under an exclusive
    /// lock, and fsync. The log is never rewritten. A session whose lines
    /// no longer extend the log's (another writer appended meanwhile) is
    /// refused rather than overwriting that writer's update; only the last
    /// line may be rewritten or dropped, as a turn does.
    pub fn save(&self, session: &Session) -> Result<(), String> {
        use std::io::{Seek, SeekFrom, Write};
        check_id(&session.id)?;
        let id = &session.id;
        let err = |e: String| format!("session {id}: {e}");
        fs::create_dir_all(&self.dir).map_err(|e| format!("sessions dir: {e}"))?;
        let p = self.path(id)?;
        let mut f = fs::OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&p)
            .map_err(|e| err(e.to_string()))?;
        f.lock().map_err(|e| err(e.to_string()))?;
        let mut body = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut body).map_err(|e| err(e.to_string()))?;
        let (mut base, valid) = if body.is_empty() {
            (None, 0)
        } else {
            match replay(&body) {
                Ok((s, v)) => (Some(s), v),
                // Only a torn first record: nothing valid yet.
                Err(e) if e == "empty log" => (None, 0),
                Err(e) => return Err(err(e)),
            }
        };
        let mut from_legacy = false;
        if base.is_none() {
            // First write of an old session: continue from its .json.
            let lp = self.legacy_path(id)?;
            if lp.is_file() {
                let old = fs::read_to_string(&lp).map_err(|e| err(e.to_string()))?;
                base = Some(serde_json::from_str::<Session>(&old).map_err(|e| err(e.to_string()))?);
                from_legacy = true;
            }
        }
        // A torn final record (crash mid-append) is cut before appending.
        if (valid as u64) < body.len() as u64 {
            f.set_len(valid as u64).map_err(|e| err(e.to_string()))?;
            f.seek(SeekFrom::End(0)).map_err(|e| err(e.to_string()))?;
        }
        let mut recs: Vec<Rec> = Vec::new();
        let mut keep = 0usize;
        match &base {
            None => recs.push(Rec::Head(head_of(session))),
            Some(b) => {
                if head_of(b) != head_of(session) {
                    recs.push(Rec::Head(head_of(session)));
                }
                let common = b
                    .lines
                    .iter()
                    .zip(&session.lines)
                    .take_while(|(x, y)| x == y)
                    .count();
                if common + 1 < b.lines.len() {
                    return Err(err(
                        "the log holds lines this writer has not seen (another writer); reload first"
                            .into(),
                    ));
                }
                if from_legacy {
                    // The log starts here: the head and every line.
                    recs.clear();
                    recs.push(Rec::Head(head_of(session)));
                } else if common < b.lines.len() {
                    recs.push(Rec::Truncate { len: common });
                    keep = common;
                } else {
                    keep = common;
                }
            }
        }
        for l in &session.lines[keep.min(session.lines.len())..] {
            recs.push(Rec::Push(l.clone()));
        }
        if recs.is_empty() {
            return Ok(());
        }
        let mut out = Vec::new();
        for r in &recs {
            serde_json::to_writer(&mut out, r).map_err(|e| err(e.to_string()))?;
            out.push(b'\n');
        }
        f.write_all(&out).map_err(|e| err(e.to_string()))?;
        f.sync_all().map_err(|e| err(e.to_string()))?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Rec {
    /// The session's fields, with no lines; the last one wins.
    Head(Session),
    Push(Line),
    Truncate { len: usize },
}

fn head_of(s: &Session) -> Session {
    Session { lines: Vec::new(), ..s.clone() }
}

/// Replay a log: the session and the byte length of its valid records. A
/// final line with no newline or that does not parse is a torn write and
/// is ignored; a bad record anywhere else is an error.
fn replay(body: &[u8]) -> Result<(Session, usize), String> {
    let mut sess: Option<Session> = None;
    let mut valid = 0usize;
    let mut pos = 0usize;
    while pos < body.len() {
        let Some(nl) = body[pos..].iter().position(|&b| b == b'\n') else {
            break;
        };
        let end = pos + nl + 1;
        match serde_json::from_slice::<Rec>(&body[pos..pos + nl]) {
            Ok(rec) => match (rec, sess.as_mut()) {
                (Rec::Head(h), Some(s)) => {
                    let lines = std::mem::take(&mut s.lines);
                    *s = Session { lines, ..h };
                }
                (Rec::Head(h), None) => sess = Some(h),
                (Rec::Push(l), Some(s)) => s.lines.push(l),
                (Rec::Truncate { len }, Some(s)) => s.lines.truncate(len),
                _ => return Err("record before the session head".into()),
            },
            Err(e) => {
                if end >= body.len() {
                    break;
                }
                return Err(format!("bad record: {e}"));
            }
        }
        valid = end;
        pos = end;
    }
    sess.map(|s| (s, valid)).ok_or_else(|| "empty log".into())
}

pub fn check_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 80 {
        return Err("task id must be 1..=80 chars".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("task id must be [A-Za-z0-9._-]".into());
    }
    Ok(())
}

pub fn new_id() -> String {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{ns:x}-{:x}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> rung_testkit::TempDir {
        rung_testkit::TempDir::new("sess")
    }

    #[test]
    fn round_trip_role_text() {
        let dir = tmp();
        let store = SessionStore::at(&dir);
        let mut s = Session::new("abc-1", Kind::Explore, Path::new("/work"));
        s.lines.push(Line::user("look around"));
        s.lines.push(Line::assistant("found Cargo.toml"));
        s.status = "completed".into();
        store.save(&s).unwrap();
        let got = store.load("abc-1").unwrap();
        assert_eq!(got, s);
        assert_eq!(got.kind().unwrap(), Kind::Explore);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_written_before_failure_loads() {
        let old = r#"{"id":"old-1","kind":"implement","status":"error","cwd":"/w",
            "lines":[{"role":"user","text":"do it"},{"role":"assistant","text":"boom"}]}"#;
        let s: Session = serde_json::from_str(old).unwrap();
        assert_eq!(s.lines[1], Line::assistant("boom"));
        assert_eq!(s.lines[1].failure, None);
    }

    #[test]
    fn a_failed_turn_round_trips() {
        let dir = tmp();
        let store = SessionStore::at(&dir);
        let mut s = Session::new("abc-2", Kind::Implement, Path::new("/work"));
        s.lines.push(Line::user("do it"));
        s.lines.push(Line::failed("auth: bad key", Vec::new()));
        store.save(&s).unwrap();
        let got = store.load("abc-2").unwrap();
        assert_eq!(got, s);
        assert_eq!(got.lines[1].failure.as_deref(), Some("auth: bad key"));
        assert_eq!(got.lines[1].text, "");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unsafe_id() {
        assert!(check_id("../etc").is_err());
        assert!(check_id("a/b").is_err());
        assert!(check_id("ok_id-1").is_ok());
    }
}

#[cfg(test)]
mod log_tests {
    use super::*;

    fn sess(id: &str) -> Session {
        Session::new(id, Kind::Explore, Path::new("/w"))
    }

    #[test]
    fn appends_and_replays_without_rewriting() {
        let dir = rung_testkit::TempDir::new("slog");
        let st = SessionStore::at(&dir);
        let mut s = sess("a");
        s.lines.push(Line::user("q"));
        st.save(&s).unwrap();
        let len1 = fs::metadata(st.path("a").unwrap()).unwrap().len();
        s.lines.push(Line::assistant("r"));
        s.status = "completed".into();
        st.save(&s).unwrap();
        assert!(fs::metadata(st.path("a").unwrap()).unwrap().len() > len1);
        assert_eq!(st.load("a").unwrap(), s);
        // last line rewritten and popped, as a turn does
        s.lines.last_mut().unwrap().text = "r2".into();
        st.save(&s).unwrap();
        assert_eq!(st.load("a").unwrap(), s);
        s.lines.pop();
        st.save(&s).unwrap();
        assert_eq!(st.load("a").unwrap(), s);
    }

    #[test]
    fn a_torn_tail_is_ignored_and_cut() {
        let dir = rung_testkit::TempDir::new("slog");
        let st = SessionStore::at(&dir);
        let mut s = sess("t");
        s.lines.push(Line::user("kept"));
        st.save(&s).unwrap();
        let p = st.path("t").unwrap();
        let mut b = fs::read(&p).unwrap();
        b.extend_from_slice(b"{\"t\":\"push\",\"role\":\"ass");
        fs::write(&p, b).unwrap();
        assert_eq!(st.load("t").unwrap(), s);
        s.lines.push(Line::assistant("next"));
        st.save(&s).unwrap();
        assert_eq!(st.load("t").unwrap(), s);
    }

    #[test]
    fn a_stale_writer_cannot_overwrite_an_update() {
        let dir = rung_testkit::TempDir::new("slog");
        let st = SessionStore::at(&dir);
        let mut s = sess("w");
        s.lines.push(Line::user("q"));
        st.save(&s).unwrap();
        let mut a = s.clone();
        let mut b = s.clone();
        for i in 0..3 {
            a.lines.push(Line::assistant(format!("a{i}")));
        }
        st.save(&a).unwrap();
        b.lines.push(Line::assistant("b"));
        assert!(st.save(&b).is_err());
        assert_eq!(st.load("w").unwrap(), a);
    }

    #[test]
    fn an_old_json_session_loads_and_continues_in_a_log() {
        let dir = rung_testkit::TempDir::new("slog");
        let st = SessionStore::at(&dir);
        let old = r#"{"id":"o","kind":"implement","status":"error","cwd":"/w",
            "lines":[{"role":"user","text":"do it"},{"role":"assistant","text":"boom"}]}"#;
        fs::write(st.legacy_path("o").unwrap(), old).unwrap();
        let mut s = st.load("o").unwrap();
        assert_eq!(s.lines.len(), 2);
        assert_eq!(st.list().unwrap().len(), 1);
        s.lines.push(Line::user("again"));
        st.save(&s).unwrap();
        assert_eq!(st.load("o").unwrap(), s);
        assert_eq!(st.list().unwrap().len(), 1);
    }
}
