//! The instance registry: which hosts exist on this machine, and whether
//! each still lives.
//!
//! One small JSON file per instance, `<id>.json`, in the registry folder
//! (`$RUNG_HOME/instances`, `~/.rung/instances` without `RUNG_HOME`). A host
//! writes its entry when it starts ([`Registration::register`]) and nothing
//! ever removes it, so a stopped or dead instance is still listed.
//!
//! An entry never says whether its host is alive: a dead host cannot say
//! anything. Liveness is a lock. While it runs the host holds an exclusive
//! `flock` on `<id>.lock` beside the entry; the kernel drops it when the
//! process ends, however it ends, so a crash cannot leave a false
//! "running". [`list`] joins each entry with that lock and with the last
//! line of the instance's record:
//!
//! | word | when |
//! |---|---|
//! | `running` | the lock is held |
//! | `stopped` | the lock is free and the record's last line is `halted` |
//! | `down` | the lock is free and the last line is not a halt: it died |
//! | `unreadable` | the entry (or its lock) cannot be read; shown with the problem |
//!
//! The finer words (working, answering, free time, waiting, stuck) refine
//! `running` and come from the record; they belong to the summary door.
//!
//! One id names one state directory, and one state directory has one id.
//! A start that would break either is refused, naming the entry it
//! collides with; nothing is replaced behind the operator's back.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::clock::Millis;
use crate::record::Record;

/// The entry file format version.
pub const SCHEMA: u32 = 1;

/// The rung home: `$RUNG_HOME`, else `~/.rung`.
pub fn home() -> PathBuf {
    match std::env::var("RUNG_HOME") {
        Ok(h) if !h.trim().is_empty() => PathBuf::from(h),
        _ => PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".rung"),
    }
}

/// The registry folder.
pub fn dir() -> PathBuf {
    home().join("instances")
}

/// An id from a name: ASCII letters and digits lowercased, every other run
/// of characters one `-`, none at the ends. `None` when nothing is left.
pub fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    (!out.is_empty()).then_some(out)
}

/// What a host says about itself when it registers.
#[derive(Debug, Clone)]
pub struct Instance {
    pub name: String,
    pub state_dir: PathBuf,
    pub config: Option<PathBuf>,
    pub workspace: PathBuf,
}

/// One entry file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub state_dir: PathBuf,
    pub config: Option<PathBuf>,
    pub workspace: PathBuf,
    /// The HTTP address the host serves, once it is bound (the real port).
    pub address: Option<String>,
    pub pid: u32,
    /// Wall time of this start, milliseconds since the epoch.
    pub started: Millis,
    pub version: String,
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The same directory however it is spelled, for comparing.
fn same_dir(a: &Path, b: &Path) -> bool {
    let c = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| absolute(p));
    c(a) == c(b)
}

fn entry_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn lock_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.lock"))
}

fn read_entry(path: &Path) -> Result<Entry, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// Every `*.json` file in `dir`, sorted by name; an absent folder has none.
fn entry_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == "json")
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    out.sort();
    Ok(out)
}

fn write_entry(dir: &Path, e: &Entry) -> Result<(), String> {
    let tmp = dir.join(format!(".{}.json.tmp", e.id));
    let text = serde_json::to_string_pretty(e).map_err(|x| x.to_string())?;
    let mut f = File::create(&tmp).map_err(|x| format!("{}: {x}", tmp.display()))?;
    f.write_all(text.as_bytes())
        .and_then(|()| f.write_all(b"\n"))
        .and_then(|()| f.sync_all())
        .map_err(|x| format!("{}: {x}", tmp.display()))?;
    let to = entry_path(dir, &e.id);
    std::fs::rename(&tmp, &to).map_err(|x| format!("{}: {x}", to.display()))
}

fn flock(f: &File, op: libc::c_int) -> std::io::Result<()> {
    // SAFETY: `flock` on a file descriptor this process owns.
    match unsafe { libc::flock(f.as_raw_fd(), op | libc::LOCK_NB) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

fn is_contended(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::EWOULDBLOCK)
}

/// A host's registration: the entry, and the lock that says it lives. Hold
/// it for the life of the process; dropping it frees the lock.
#[derive(Debug)]
pub struct Registration {
    dir: PathBuf,
    entry: Mutex<Entry>,
    _lock: File,
}

impl Registration {
    /// Register `instance` in the folder [`dir`].
    pub fn register(instance: &Instance, started: Millis) -> Result<Self, String> {
        Self::register_in(&dir(), instance, started)
    }

    /// Register `instance` in `dir`: refused when its id belongs to a live
    /// host or to another state directory, or its state directory to another
    /// id. A restart (same id, same state, lock free) renews the entry.
    pub fn register_in(dir: &Path, instance: &Instance, started: Millis) -> Result<Self, String> {
        let id = slug(&instance.name)
            .ok_or_else(|| format!("instance name `{}` has no letter or digit", instance.name))?;
        std::fs::create_dir_all(dir).map_err(|e| format!("registry {}: {e}", dir.display()))?;
        let state_dir = absolute(&instance.state_dir);

        // One state, one id.
        for path in entry_files(dir)? {
            let Ok(other) = read_entry(&path) else {
                continue;
            };
            if other.id != id && same_dir(&other.state_dir, &state_dir) {
                return Err(format!(
                    "state {} is registered as `{}` ({}); start it under that name",
                    state_dir.display(),
                    other.id,
                    path.display()
                ));
            }
        }

        // One live host per id.
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path(dir, &id))
            .map_err(|e| format!("registry lock {id}: {e}"))?;
        let held = {
            // `ls` holds a shared lock for an instant while it probes.
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
            loop {
                match flock(&lock, libc::LOCK_EX) {
                    Ok(()) => break Ok(()),
                    Err(e) if is_contended(&e) && std::time::Instant::now() < deadline => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => break Err(e),
                }
            }
        };
        if let Err(e) = held {
            if !is_contended(&e) {
                return Err(format!("registry lock {id}: {e}"));
            }
            let pid = read_entry(&entry_path(dir, &id))
                .map(|e| format!("pid {}", e.pid))
                .unwrap_or_else(|_| "another process".into());
            return Err(format!(
                "instance `{id}` is already running ({pid}); one id has one live host"
            ));
        }
        if let Ok(old) = read_entry(&entry_path(dir, &id))
            && !same_dir(&old.state_dir, &state_dir)
        {
            return Err(format!(
                "instance `{id}` is registered for state {}; pick another name for {}",
                old.state_dir.display(),
                state_dir.display()
            ));
        }

        let entry = Entry {
            schema: SCHEMA,
            id,
            name: instance.name.clone(),
            state_dir,
            config: instance.config.as_deref().map(absolute),
            workspace: absolute(&instance.workspace),
            address: None,
            pid: std::process::id(),
            started,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        write_entry(dir, &entry)?;
        Ok(Registration {
            dir: dir.to_path_buf(),
            entry: Mutex::new(entry),
            _lock: lock,
        })
    }

    /// The entry as written.
    pub fn entry(&self) -> Entry {
        crate::core::lock(&self.entry).clone()
    }

    /// Record the address the host serves (the real, bound one).
    pub fn set_address(&self, address: &str) -> Result<(), String> {
        let mut e = crate::core::lock(&self.entry);
        e.address = Some(address.to_string());
        write_entry(&self.dir, &e)
    }
}

/// The record's last line, as `ls` shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Last {
    pub seq: u64,
    pub at: Millis,
    pub kind: String,
}

/// What an instance is, by its lock and its record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Word {
    Running,
    Stopped,
    Down,
    Unreadable,
}

impl Word {
    pub fn as_str(self) -> &'static str {
        match self {
            Word::Running => "running",
            Word::Stopped => "stopped",
            Word::Down => "down",
            Word::Unreadable => "unreadable",
        }
    }
}

/// One listed instance. `entry` is `None` only for an unreadable entry file.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: String,
    pub entry: Option<Entry>,
    pub word: Word,
    pub last: Option<Last>,
    /// Why something could not be read, when it could not.
    pub problem: Option<String>,
}

impl Row {
    /// The row as the JSON `ls --json` and the gateway read: the entry's
    /// fields, then `word`, `last` and `problem`.
    pub fn to_json(&self) -> Value {
        let mut v = match &self.entry {
            Some(e) => serde_json::to_value(e).unwrap_or_else(|_| json!({})),
            None => json!({"id": self.id}),
        };
        v["word"] = json!(self.word.as_str());
        v["last"] = serde_json::to_value(&self.last).unwrap_or(Value::Null);
        v["problem"] = json!(self.problem);
        v
    }
}

/// Is a host holding the lock? A shared, non-blocking probe: it succeeds
/// only when no host holds the exclusive lock, and it is released at once.
fn lock_held(dir: &Path, id: &str) -> Result<bool, String> {
    let f = match File::open(lock_path(dir, id)) {
        Ok(f) => f,
        // No lock file: no host ever took it.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("lock {id}: {e}")),
    };
    match flock(&f, libc::LOCK_SH) {
        Ok(()) => Ok(false),
        Err(e) if is_contended(&e) => Ok(true),
        Err(e) => Err(format!("lock {id}: {e}")),
    }
}

fn last_of(state_dir: &Path) -> Result<Option<Last>, String> {
    Record::last_line(state_dir.join("record"))
        .map(|l| {
            l.map(|l| Last {
                seq: l.seq,
                at: l.at,
                kind: l.kind,
            })
        })
        .map_err(|e| format!("record: {e}"))
}

/// Every registered instance in the folder [`dir`], by id.
pub fn list() -> Result<Vec<Row>, String> {
    list_in(&dir())
}

/// Every registered instance in `dir`, by id. No entry is dropped: one that
/// cannot be read is a row with a problem.
pub fn list_in(dir: &Path) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for path in entry_files(dir)? {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let entry = match read_entry(&path) {
            Ok(e) => e,
            Err(why) => {
                rows.push(Row {
                    id: stem,
                    entry: None,
                    word: Word::Unreadable,
                    last: None,
                    problem: Some(format!("{}: {why}", path.display())),
                });
                continue;
            }
        };
        let held = lock_held(dir, &entry.id);
        let last = last_of(&entry.state_dir);
        let (word, problem) = match (&held, &last) {
            (Err(why), _) => (Word::Unreadable, Some(why.clone())),
            (Ok(true), Err(why)) => (Word::Running, Some(why.clone())),
            (Ok(true), Ok(_)) => (Word::Running, None),
            (Ok(false), Ok(Some(l))) if l.kind == "halted" => (Word::Stopped, None),
            (Ok(false), Ok(_)) => (Word::Down, None),
            (Ok(false), Err(why)) => (Word::Down, Some(why.clone())),
        };
        rows.push(Row {
            id: entry.id.clone(),
            last: last.ok().flatten(),
            entry: Some(entry),
            word,
            problem,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

/// The rows as `ls` prints them: a table, one row per instance, a problem
/// on a line of its own under its row.
pub fn render(rows: &[Row]) -> String {
    if rows.is_empty() {
        return "no instances registered\n".into();
    }
    let cells: Vec<[String; 6]> = rows
        .iter()
        .map(|r| {
            let e = r.entry.as_ref();
            [
                r.id.clone(),
                r.word.as_str().to_string(),
                match (r.word, e) {
                    (Word::Running, Some(e)) => e.pid.to_string(),
                    _ => "-".into(),
                },
                e.and_then(|e| e.address.clone())
                    .unwrap_or_else(|| "-".into()),
                r.last
                    .as_ref()
                    .map_or("-".into(), |l| format!("{} #{}", l.kind, l.seq)),
                e.map_or("-".into(), |e| e.state_dir.display().to_string()),
            ]
        })
        .collect();
    let head = ["ID", "STATE", "PID", "ADDRESS", "LAST", "STATE DIR"];
    let mut width = head.map(str::len);
    for c in &cells {
        for (w, s) in width.iter_mut().zip(c) {
            *w = (*w).max(s.chars().count());
        }
    }
    let line = |c: &[String; 6]| {
        let mut s = String::new();
        for (i, (cell, w)) in c.iter().zip(width).enumerate() {
            if i + 1 == c.len() {
                s.push_str(cell);
            } else {
                s.push_str(&format!("{cell:<w$}  "));
            }
        }
        s
    };
    let mut out = line(&head.map(String::from));
    out.push('\n');
    for (c, r) in cells.iter().zip(rows) {
        out.push_str(&line(c));
        out.push('\n');
        if let Some(p) = &r.problem {
            out.push_str(&format!("  ! {p}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(name: &str, state: &Path) -> Instance {
        Instance {
            name: name.into(),
            state_dir: state.to_path_buf(),
            config: None,
            workspace: state.join("workspace"),
        }
    }

    #[test]
    fn ids_are_slugs_and_empty_ones_are_refused() {
        assert_eq!(slug("Deep Thought").as_deref(), Some("deep-thought"));
        assert_eq!(slug("  a__b  ").as_deref(), Some("a-b"));
        assert_eq!(slug("alpha-1").as_deref(), Some("alpha-1"));
        assert_eq!(slug("---"), None);
        assert_eq!(slug(""), None);
        let d = crate::sim::temp_dir_guard("registry-slug");
        let e = Registration::register_in(d.path(), &instance("!!!", d.path()), 1).unwrap_err();
        assert!(e.contains("no letter or digit"), "{e}");
    }

    #[test]
    fn a_held_lock_reads_running_and_a_dropped_one_does_not() {
        let d = crate::sim::temp_dir_guard("registry-lock");
        let reg = d.path().join("reg");
        let state = d.path().join("s");
        let held = Registration::register_in(&reg, &instance("one", &state), 5).unwrap();
        let rows = list_in(&reg).unwrap();
        assert_eq!((rows.len(), rows[0].word), (1, Word::Running));
        assert_eq!(rows[0].entry.as_ref().unwrap().pid, std::process::id());
        // A second registration of the id is refused while the first lives.
        let e = Registration::register_in(&reg, &instance("one", &state), 6).unwrap_err();
        assert!(e.contains("already running"), "{e}");
        drop(held);
        let rows = list_in(&reg).unwrap();
        assert_eq!(rows[0].word, Word::Down, "no record, no halt: down");
        assert!(rows[0].problem.is_none() || rows[0].last.is_none());
    }

    #[test]
    fn the_address_is_rewritten_into_the_entry() {
        let d = crate::sim::temp_dir_guard("registry-address");
        let reg = d.path().join("reg");
        let r = Registration::register_in(&reg, &instance("one", d.path()), 5).unwrap();
        assert_eq!(r.entry().address, None);
        r.set_address("127.0.0.1:4567").unwrap();
        let rows = list_in(&reg).unwrap();
        assert_eq!(
            rows[0].entry.as_ref().unwrap().address.as_deref(),
            Some("127.0.0.1:4567")
        );
    }

    #[test]
    fn the_table_shows_every_row_and_its_problem() {
        let d = crate::sim::temp_dir_guard("registry-render");
        assert_eq!(render(&[]), "no instances registered\n");
        let reg = d.path().join("reg");
        std::fs::create_dir_all(&reg).unwrap();
        std::fs::write(reg.join("broken.json"), "{").unwrap();
        let text = render(&list_in(&reg).unwrap());
        assert!(text.starts_with("ID"), "{text}");
        assert!(
            text.contains("broken") && text.contains("unreadable"),
            "{text}"
        );
        assert!(text.contains("  ! "), "{text}");
    }

    #[test]
    fn the_last_line_is_read_from_the_end_across_segments() {
        use crate::record::Record;
        let d = crate::sim::temp_dir_guard("registry-last");
        assert!(Record::last_line(d.path().join("none")).unwrap().is_none());
        let dir = d.path().join("record");
        let opened = Record::open_with(&dir, 200).unwrap();
        for i in 0..40 {
            opened
                .record
                .append(i, "note", json!({"text": "x".repeat(50)}));
        }
        drop(opened);
        // A torn tail is ignored, not an error.
        let mut segs: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        segs.sort_by_key(|e| e.file_name());
        let newest = segs.last().unwrap().path();
        let mut f = OpenOptions::new().append(true).open(newest).unwrap();
        f.write_all(b"{\"seq\":99,\"at\"").unwrap();
        let last = Record::last_line(&dir).unwrap().unwrap();
        assert_eq!((last.seq, last.kind.as_str()), (40, "note"));
    }
}
