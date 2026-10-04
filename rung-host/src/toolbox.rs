//! The host's tools: one stable superset, a host-side call gate, and the
//! host-owned sandbox workspace.
//!
//! Every definition is sent on every call (sorted by name, fixed text), so
//! the cached prefix holds; the Tools family only moves the gate. A call to
//! a declared but disabled tool returns
//! `{"error":"not enabled this turn","group":…,"ask":"want_tools"}` and
//! never runs. A call that would run past the tool deadline (or the turn's
//! remaining bound) is refused with the long-work message: the agent breaks
//! the work into steps or commits to it as a project.
//!
//! The agent changes the world only inside the host's workspace directory:
//! the `read` and `workspace_write` groups resolve every path inside it and
//! refuse anything that leaves it. There is no shell in this slice.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use rung_std::llm::ToolDefinition;
use rung_std::tools::Toolset;
use serde_json::{Value, json};

use crate::clock::{Millis, SECOND};
use crate::core::Core;
use crate::gates::LONG_WORK_MESSAGE;
use crate::kernel;
use crate::memory::MemoryHost;
use crate::registers::Check;

pub const CORE: &str = "core";
pub const MEMORY: &str = "memory";
pub const READ: &str = "read";
pub const WORKSPACE_WRITE: &str = "workspace_write";
pub const WEB_READ: &str = "web_read";
/// Reserved for the delegation extension; never in a superset or ceiling.
pub const RESERVED_GROUPS: &[&str] = &["crew"];

/// The groups in a superset, in order.
pub fn groups(memory: bool) -> Vec<&'static str> {
    let mut g = vec![CORE, READ, WORKSPACE_WRITE, WEB_READ];
    if memory {
        g.insert(1, MEMORY);
    }
    g
}

fn def(name: &str, description: &str, schema: Value) -> ToolDefinition {
    ToolDefinition::new(name, description, schema)
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

fn s(desc: &str) -> Value {
    json!({"type": "string", "description": desc})
}

/// Every tool a group holds: (name, description, schema).
fn group_tools(group: &str) -> Vec<ToolDefinition> {
    match group {
        CORE => vec![
            def(
                "note",
                "Replace your carried note: what you want to carry into the next context epoch, in your own words.",
                obj(json!({"text": s("The whole note.")}), &["text"]),
            ),
            def(
                "commit",
                "Commit to a project. Your turns continue it until you release it. One commitment at a time.",
                obj(
                    json!({
                        "project": s("An existing project id."),
                        "new": {"type": "object", "properties": {"title": s("Title."), "why": s("Why it pulls you.")}, "required": ["title", "why"]},
                        "done_when": s("How you will know it is done."),
                        "until_s": {"type": "integer", "description": "Optional: seconds from now you mean to finish by."},
                        "checkpoint_every": {"type": "integer", "description": "Optional: turns between checkpoints."}
                    }),
                    &["done_when"],
                ),
            ),
            def(
                "progress",
                "Record the next step of your commitment; the next committed turn shows it.",
                obj(
                    json!({"next_step": s("The next step."), "note": s("Optional.")}),
                    &["next_step"],
                ),
            ),
            def(
                "release",
                "Release your commitment: done, paused or abandoned. Free time again.",
                obj(
                    json!({"outcome": {"type": "string", "enum": ["done", "paused", "abandoned"]}, "reason": s("Why.")}),
                    &["outcome", "reason"],
                ),
            ),
            def(
                "trace",
                "Leave a trace of a free-time session: what pulled you, where it went, what is unresolved. Ends the session.",
                obj(
                    json!({"what_pulled": s("What pulled you."), "where_it_went": s("Where it went."), "still_thinking": s("Optional: what is unresolved.")}),
                    &["what_pulled", "where_it_went"],
                ),
            ),
            def(
                "expect",
                "State an expectation about your world. The host settles it; you cannot.",
                obj(
                    json!({
                        "claim": s("What you expect."), "about": s("What it is about."), "warrant": s("Why you expect it."),
                        "p": {"type": "number", "description": "Your probability that the claim holds, in (0, 1)."},
                        "due_in_s": {"type": "integer", "description": "Seconds until it is due."},
                        "check": {"type": "object", "description": "{\"world_fact\": {\"key\", \"equals\"}} or {\"stimulus_from\": {\"channel\"}} or {\"judged\": {\"principal\"}}."}
                    }),
                    &["claim", "p", "due_in_s", "check"],
                ),
            ),
            def(
                "revise",
                "Revise an open expectation's probability. The old value stays on record.",
                obj(
                    json!({"id": s("Expectation id."), "p": {"type": "number"}, "why": s("Why.")}),
                    &["id", "p", "why"],
                ),
            ),
            def(
                "send",
                "Send a message to a channel (owner, or a peer you have heard from).",
                obj(
                    json!({"channel": s("Channel."), "text": s("Message.")}),
                    &["channel", "text"],
                ),
            ),
            def(
                "want_tools",
                "Ask for a tool group to be enabled; decided at the next boundary.",
                obj(
                    json!({"group": s("Group name."), "why": s("Why you need it.")}),
                    &["group", "why"],
                ),
            ),
            def(
                "todo_add",
                "Add a todo item.",
                obj(
                    json!({"text": s("The item."), "priority": {"type": "number"}}),
                    &["text"],
                ),
            ),
            def(
                "todo_done",
                "Mark a todo item done.",
                obj(json!({"id": s("Todo id.")}), &["id"]),
            ),
            def(
                "question_add",
                "Open a research question.",
                obj(
                    json!({"text": s("The question."), "priority": {"type": "number"}}),
                    &["text"],
                ),
            ),
            def(
                "question_close",
                "Close a research question.",
                obj(json!({"id": s("Question id.")}), &["id"]),
            ),
        ],
        MEMORY => vec![
            def(
                "memory_search",
                "Search your long-term memory.",
                obj(json!({"query": s("What to look for.")}), &["query"]),
            ),
            def(
                "memory_keep",
                "Keep something in long-term memory, in your own words.",
                obj(json!({"text": s("What to keep.")}), &["text"]),
            ),
        ],
        READ => vec![
            def(
                "ws_read",
                "Read a file in your workspace.",
                obj(json!({"path": s("Relative path.")}), &["path"]),
            ),
            def(
                "ws_list",
                "List a directory in your workspace.",
                obj(
                    json!({"path": s("Relative path; empty for the root.")}),
                    &[],
                ),
            ),
        ],
        WORKSPACE_WRITE => vec![
            def(
                "ws_write",
                "Write a file in your workspace (replaces it).",
                obj(
                    json!({"path": s("Relative path."), "text": s("Content.")}),
                    &["path", "text"],
                ),
            ),
            def(
                "ws_remove",
                "Remove a file in your workspace.",
                obj(json!({"path": s("Relative path.")}), &["path"]),
            ),
        ],
        WEB_READ => vec![def(
            "web_fetch",
            "Fetch a web page as text.",
            obj(json!({"url": s("URL.")}), &["url"]),
        )],
        _ => Vec::new(),
    }
}

/// The stable superset: every group's tools, sorted by name.
pub fn superset(memory: bool) -> Vec<ToolDefinition> {
    let mut all: Vec<ToolDefinition> = groups(memory).into_iter().flat_map(group_tools).collect();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    all
}

/// The group a tool belongs to.
pub fn group_of(name: &str) -> Option<&'static str> {
    groups(true)
        .into_iter()
        .find(|g| group_tools(g).iter().any(|d| d.name == name))
}

/// Reads the web for `web_fetch`. Slice 1 has no live implementation.
pub trait WebReader: Send + Sync {
    /// The page text, and how long fetching it takes (simulated, ms).
    fn fetch(&self, url: &str) -> (Result<String, String>, Millis);
}

/// No web: every fetch is refused (no network in this build).
#[derive(Debug, Default)]
pub struct NoWeb;

impl WebReader for NoWeb {
    fn fetch(&self, _url: &str) -> (Result<String, String>, Millis) {
        (Err("web_read is not wired in this build".into()), 0)
    }
}

/// The host's toolset for one turn.
pub struct HostTools {
    pub(crate) core: Arc<Core>,
    pub(crate) turn: u64,
    pub(crate) enabled: BTreeSet<String>,
    /// The turn's hard end (host clock).
    pub(crate) turn_deadline: Millis,
    pub(crate) memory: Option<Arc<MemoryHost>>,
    pub(crate) web: Arc<dyn WebReader>,
    pub(crate) workspace: PathBuf,
    defs: Vec<ToolDefinition>,
}

impl std::fmt::Debug for HostTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostTools")
            .field("turn", &self.turn)
            .field("enabled", &self.enabled)
            .finish()
    }
}

/// The most a tool result carries, characters.
pub const RESULT_CHARS: usize = 4_000;

impl HostTools {
    pub(crate) fn new(
        core: Arc<Core>,
        turn: u64,
        enabled: BTreeSet<String>,
        turn_deadline: Millis,
        memory: Option<Arc<MemoryHost>>,
        web: Arc<dyn WebReader>,
        workspace: PathBuf,
    ) -> Self {
        let defs = superset(memory.is_some());
        Self {
            core,
            turn,
            enabled,
            turn_deadline,
            memory,
            web,
            workspace,
            defs,
        }
    }

    fn refuse(&self, name: &str, group: &str, why: &str, message: String) -> String {
        self.core.emit(
            "tool.refused",
            json!({"turn": self.turn, "name": name, "group": group, "why": why, "message": message}),
        );
        message
    }

    /// The tool deadline for a call now: 30 s or what is left of the turn.
    fn deadline_ms(&self) -> Millis {
        let left = self.turn_deadline - self.core.clock.now();
        self.core.config.tool_deadline_ms.min(left).max(0)
    }

    /// Run `f` (which reports its own simulated duration) under the
    /// deadline. A sim clock charges the duration (or the deadline, when
    /// the work would overrun it); a real clock runs it on a thread and
    /// stops waiting at the deadline.
    fn timed<F>(&self, name: &str, group: &str, f: F) -> Result<String, String>
    where
        F: FnOnce() -> (Result<String, String>, Millis) + Send + 'static,
    {
        let deadline = self.deadline_ms();
        let clock = self.core.clock.clone();
        let overran = || Err(self.refuse(name, group, "overran", LONG_WORK_MESSAGE.into()));
        if clock.is_sim() {
            let (out, took) = f();
            if took > deadline {
                clock.advance(deadline);
                return overran();
            }
            clock.advance(took);
            return out;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (out, took) = f();
            clock.advance(took);
            let _ = tx.send(out);
        });
        match rx.recv_timeout(std::time::Duration::from_millis(deadline.max(0) as u64)) {
            Ok(out) => out,
            Err(_) => overran(),
        }
    }

    fn path(&self, rel: &str) -> Result<PathBuf, String> {
        confine(&self.workspace, rel)
    }

    fn run(&self, name: &str, group: &str, input: &Value) -> Result<String, String> {
        let core = &*self.core;
        let turn = self.turn;
        let str_of = |k: &str| -> Result<String, String> {
            input
                .get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("`{k}` is required"))
        };
        match name {
            "note" => {
                let text = str_of("text")?;
                core.emit("note.written", json!({"turn": turn, "text": text}));
                Ok("note kept; it rides into the next epoch".into())
            }
            "commit" => kernel::tool_commit(core, turn, input),
            "progress" => kernel::tool_progress(core, turn, input),
            "release" => kernel::tool_release(core, turn, input),
            "trace" => kernel::tool_trace(core, turn, input),
            "expect" => {
                let p = input.get("p").and_then(Value::as_f64).unwrap_or(-1.0);
                if !(p > 0.0 && p < 1.0) {
                    return Err("`p` must be in (0, 1)".into());
                }
                let due_in = input.get("due_in_s").and_then(Value::as_i64).unwrap_or(0);
                if due_in <= 0 {
                    return Err("`due_in_s` must be positive".into());
                }
                let check: Check =
                    serde_json::from_value(input.get("check").cloned().unwrap_or_default())
                        .map_err(|e| format!("`check`: {e}"))?;
                let id = core.state().registers.next_id("e");
                core.emit(
                    "expectation.made",
                    json!({"id": id, "turn": turn, "claim": str_of("claim")?,
                           "about": input.get("about").and_then(Value::as_str).unwrap_or(""),
                           "warrant": input.get("warrant").and_then(Value::as_str).unwrap_or(""),
                           "p": p, "due": core.clock.now().saturating_add(due_in.saturating_mul(SECOND)), "check": check}),
                );
                Ok(format!(
                    "expectation {id} recorded; the host will settle it"
                ))
            }
            "revise" => {
                let id = str_of("id")?;
                let p = input.get("p").and_then(Value::as_f64).unwrap_or(-1.0);
                if !(p > 0.0 && p < 1.0) {
                    return Err("`p` must be in (0, 1)".into());
                }
                let open = core
                    .state()
                    .registers
                    .expectations
                    .get(&id)
                    .is_some_and(|e| e.state == crate::registers::ExpState::Open);
                if !open {
                    return Err(format!("no open expectation `{id}`"));
                }
                core.emit(
                    "expectation.revised",
                    json!({"id": id, "p": p, "why": str_of("why")?, "turn": turn}),
                );
                Ok(format!("expectation {id} revised"))
            }
            "send" => {
                let channel = str_of("channel")?;
                let text = str_of("text")?;
                core.emit(
                    "outbox.queued",
                    json!({"turn": turn, "channel": channel, "text": text, "source": "agent"}),
                );
                Ok(format!("queued for {channel}"))
            }
            "want_tools" => {
                let group = str_of("group")?;
                let why = str_of("why")?;
                let outside = !groups(true).contains(&group.as_str());
                let allowed = core.config.ceiling.contains(&group);
                core.emit(
                    "tools.wanted",
                    json!({"turn": turn, "group": group, "why": why, "outside": outside, "allowed": allowed}),
                );
                if allowed {
                    Ok(format!("asked for `{group}`; decided at the next boundary"))
                } else {
                    Err(format!("`{group}` is outside the operator ceiling"))
                }
            }
            "todo_add" | "question_add" => {
                let (kind, prefix) = if name == "todo_add" {
                    ("todo.added", "t")
                } else {
                    ("question.added", "q")
                };
                let id = core.state().registers.next_id(prefix);
                core.emit(
                    kind,
                    json!({"id": id, "turn": turn, "text": str_of("text")?, "priority": input.get("priority")}),
                );
                Ok(format!("added {id}"))
            }
            "todo_done" | "question_close" => {
                let id = str_of("id")?;
                let known = if name == "todo_done" {
                    core.state().registers.todo.contains_key(&id)
                } else {
                    core.state().registers.questions.contains_key(&id)
                };
                if !known {
                    return Err(format!("no `{id}`"));
                }
                let kind = if name == "todo_done" {
                    "todo.done"
                } else {
                    "question.closed"
                };
                core.emit(kind, json!({"id": id, "turn": turn}));
                Ok(format!("{id} closed"))
            }
            "memory_search" => {
                let Some(m) = self.memory.clone() else {
                    return Err("no memory provider".into());
                };
                let query = str_of("query")?;
                let core2 = self.core.clone();
                self.timed(name, group, move || {
                    let (report, block) = m.recall(&query, Vec::new());
                    core2.emit(
                        "memory.recall",
                        json!({"turn": turn, "cue": "tool", "report": report}),
                    );
                    (Ok(block.unwrap_or_else(|| "nothing recalled".into())), 0)
                })
            }
            "memory_keep" => {
                let Some(m) = self.memory.clone() else {
                    return Err("no memory provider".into());
                };
                let text = str_of("text")?;
                let attrs = BTreeMap::from([
                    ("source".to_string(), "agent".to_string()),
                    ("turn".to_string(), turn.to_string()),
                ]);
                let report = m.retain(&text, attrs);
                core.emit(
                    "memory.retain",
                    json!({"turn": turn, "candidate": "tool", "report": report}),
                );
                Ok(report.status.to_string())
            }
            "ws_read" => {
                let p = self.path(&str_of("path")?)?;
                let t = std::fs::read_to_string(&p).map_err(|e| format!("read: {e}"))?;
                Ok(t.chars().take(RESULT_CHARS).collect())
            }
            "ws_list" => {
                let rel = input.get("path").and_then(Value::as_str).unwrap_or("");
                let p = if rel.is_empty() {
                    self.workspace.clone()
                } else {
                    self.path(rel)?
                };
                let mut names: Vec<String> = std::fs::read_dir(&p)
                    .map_err(|e| format!("list: {e}"))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                names.sort();
                Ok(names.join("\n"))
            }
            "ws_write" => {
                let p = self.path(&str_of("path")?)?;
                let text = str_of("text")?;
                if text.len() > 64 * 1024 {
                    return Err("over 64 KiB".into());
                }
                if let Some(dir) = p.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir: {e}"))?;
                }
                std::fs::write(&p, text).map_err(|e| format!("write: {e}"))?;
                Ok("written".into())
            }
            "ws_remove" => {
                let p = self.path(&str_of("path")?)?;
                std::fs::remove_file(&p).map_err(|e| format!("remove: {e}"))?;
                Ok("removed".into())
            }
            "web_fetch" => {
                let url = str_of("url")?;
                let web = self.web.clone();
                self.timed(name, group, move || {
                    let (out, took) = web.fetch(&url);
                    (out.map(|t| t.chars().take(RESULT_CHARS).collect()), took)
                })
            }
            _ => Err(format!("unknown tool `{name}`")),
        }
    }
}

/// `rel` inside `root`, or why not. Absolute paths, `..`, and anything a
/// symlink takes outside the root are refused.
pub fn confine(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    if rel.is_empty()
        || rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("`{rel}` is not a path inside your workspace"));
    }
    std::fs::create_dir_all(root).map_err(|e| format!("workspace: {e}"))?;
    let real_root = root.canonicalize().map_err(|e| format!("workspace: {e}"))?;
    let joined = real_root.join(rel_path);
    // The deepest existing ancestor must still be inside the root.
    let mut probe = joined.as_path();
    loop {
        if probe.symlink_metadata().is_ok() {
            let real = probe.canonicalize().map_err(|e| format!("{rel}: {e}"))?;
            if !real.starts_with(&real_root) {
                return Err(format!("`{rel}` leaves your workspace"));
            }
            break;
        }
        match probe.parent() {
            Some(p) => probe = p,
            None => break,
        }
    }
    Ok(joined)
}

impl Toolset for HostTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.defs.clone()
    }

    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        let Some(group) = group_of(name).filter(|g| *g != MEMORY || self.memory.is_some()) else {
            return Err(format!("unknown tool `{name}`"));
        };
        if !self.enabled.contains(group) {
            let msg =
                json!({"error": "not enabled this turn", "group": group, "ask": "want_tools"})
                    .to_string();
            return Err(self.refuse(name, group, "disabled", msg));
        }
        let out = self.run(name, group, input);
        let refused_long = matches!(&out, Err(m) if m == LONG_WORK_MESSAGE);
        if !refused_long {
            self.core.emit(
                "tool.call",
                json!({"turn": self.turn, "name": name, "group": group, "ok": out.is_ok()}),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_superset_is_sorted_and_stable() {
        let a = superset(true);
        let names: Vec<&str> = a.iter().map(|d| d.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(crate::canon::of(&a), crate::canon::of(&superset(true)));
        assert_eq!(group_of("ws_write"), Some(WORKSPACE_WRITE));
        assert_eq!(group_of("commit"), Some(CORE));
        assert!(RESERVED_GROUPS.iter().all(|g| !groups(true).contains(g)));
    }

    #[test]
    fn paths_stay_in_the_workspace() {
        let guard = crate::sim::temp_dir_guard("ws");
        let root = guard.path().join("ws");
        assert!(confine(&root, "a/b.txt").is_ok());
        assert!(confine(&root, "../x").is_err());
        assert!(confine(&root, "/etc/passwd").is_err());
        assert!(confine(&root, "").is_err());
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", root.join("out")).unwrap();
            assert!(confine(&root, "out/x").is_err());
        }
    }
}
