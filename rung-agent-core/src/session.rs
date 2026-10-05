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

    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        check_id(id)?;
        Ok(self.dir.join(format!("{id}.json")))
    }

    pub fn load(&self, id: &str) -> Result<Session, String> {
        let p = self.path(id)?;
        let body = fs::read_to_string(&p).map_err(|e| format!("session {id}: {e}"))?;
        serde_json::from_str(&body).map_err(|e| format!("session {id}: {e}"))
    }

    pub fn try_load(&self, id: &str) -> Result<Option<Session>, String> {
        let p = self.path(id)?;
        if !p.is_file() {
            return Ok(None);
        }
        Ok(Some(self.load(id)?))
    }

    pub fn list(&self) -> Result<Vec<Session>, String> {
        let mut out = Vec::new();
        if !self.dir.is_dir() {
            return Ok(out);
        }
        let ents = fs::read_dir(&self.dir).map_err(|e| format!("sessions dir: {e}"))?;
        for ent in ents {
            let ent = ent.map_err(|e| format!("sessions dir: {e}"))?;
            let p = ent.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match self.load(stem) {
                Ok(s) => out.push(s),
                Err(_) => continue,
            }
        }
        Ok(out)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let p = self.path(id)?;
        if p.is_file() {
            fs::remove_file(&p).map_err(|e| format!("session {id}: {e}"))?;
        }
        Ok(())
    }

    pub fn save(&self, session: &Session) -> Result<(), String> {
        check_id(&session.id)?;
        fs::create_dir_all(&self.dir).map_err(|e| format!("sessions dir: {e}"))?;
        let p = self.path(&session.id)?;
        let tmp = p.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
        fs::write(&tmp, body).map_err(|e| format!("session {}: {e}", session.id))?;
        fs::rename(&tmp, &p).map_err(|e| format!("session {}: {e}", session.id))?;
        Ok(())
    }
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
