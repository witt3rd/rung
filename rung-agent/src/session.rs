//! Persist role+text lines. [`rung_std::llm::ChatMessage`] is serialize-only
//! (blocks, cache hints); resume does not replay tool_use.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::Kind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Line {
    pub role: String,
    pub text: String,
}

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

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rung-sess-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn round_trip_role_text() {
        let dir = tmp();
        let store = SessionStore::at(&dir);
        let mut s = Session::new("abc-1", Kind::Explore, Path::new("/work"));
        s.lines.push(Line {
            role: "user".into(),
            text: "look around".into(),
        });
        s.lines.push(Line {
            role: "assistant".into(),
            text: "found Cargo.toml".into(),
        });
        s.status = "completed".into();
        store.save(&s).unwrap();
        let got = store.load("abc-1").unwrap();
        assert_eq!(got, s);
        assert_eq!(got.kind().unwrap(), Kind::Explore);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unsafe_id() {
        assert!(check_id("../etc").is_err());
        assert!(check_id("a/b").is_err());
        assert!(check_id("ok_id-1").is_ok());
    }
}
