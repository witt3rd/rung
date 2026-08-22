//! Drive [`rung_std::agent::run`] with a catalog roster, optional nested
//! [`CatalogSpawn`], and a session file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rung_std::agent::{self, LoopState, Thread, agentloop};
use rung_std::llm::{ChatMessage, LlmConfig};
use rung_std::tools::{
    MAX_DEPTH, Spawn, Task, TaskRequest, TaskResult, ToolCollection, ToolRoster, Toolset,
    WithoutTask,
};

use serde::Serialize;

use crate::args::{Args, IsolationMode};
use crate::catalog::Kind;
use crate::session::{Line, Session, SessionStore};

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub task_id: String,
    pub text: String,
    pub status: String,
    pub api_calls: u32,
    pub isolation_path: Option<String>,
}

/// Nested `task` Spawn: pick a catalog kind, persist a child session, run a
/// depth-capped loop. Isolation stays the process cwd (parent already chdir'd).
#[derive(Clone)]
pub struct CatalogSpawn {
    pub config: LlmConfig,
    pub store: SessionStore,
    pub max_iterations: u32,
    pub emitter: Option<Arc<crate::stream::Emitter>>,
}

impl std::fmt::Debug for CatalogSpawn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalogSpawn")
            .field("store", &self.store.dir)
            .field("max_iterations", &self.max_iterations)
            .finish()
    }
}

impl Spawn for CatalogSpawn {
    fn spawn(&self, req: &TaskRequest) -> Result<TaskResult, String> {
        let kind = match &req.subagent_type {
            Some(s) => Kind::parse(s)?,
            None => Kind::Explore,
        };
        let id = match &req.task_id {
            Some(id) => {
                crate::session::check_id(id)?;
                id.clone()
            }
            None => crate::session::new_id(),
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut sess = match self.store.try_load(&id)? {
            Some(s) => s,
            None => Session::new(&id, kind, &cwd),
        };
        sess.kind = kind.as_str().into();
        sess.lines.push(Line {
            role: "user".into(),
            text: req.prompt.clone(),
        });
        sess.status = "running".into();
        sess.pid = Some(std::process::id());
        self.store.save(&sess)?;
        match drive(
            &self.config,
            kind,
            &sess.lines,
            self.max_iterations,
            self.emitter.clone(),
        ) {
            Ok((text, api_calls)) => {
                sess.lines.push(Line {
                    role: "assistant".into(),
                    text: text.clone(),
                });
                sess.status = "completed".into();
                self.store.save(&sess)?;
                Ok(TaskResult {
                    text,
                    api_calls,
                    task_id: Some(id),
                })
            }
            Err(e) => {
                sess.status = "error".into();
                sess.lines.push(Line {
                    role: "assistant".into(),
                    text: e.clone(),
                });
                let _ = self.store.save(&sess);
                Err(e)
            }
        }
    }
}

fn drive(
    config: &LlmConfig,
    kind: Kind,
    lines: &[Line],
    max_iterations: u32,
    emitter: Option<Arc<crate::stream::Emitter>>,
) -> Result<(String, u32), String> {
    let cap = max_iterations.min(kind.max_iterations()).max(1);
    let base: Arc<dyn Toolset> = Arc::new(WithoutTask::new(Arc::new(kind.roster())));
    let tools: Arc<dyn Toolset> = match &emitter {
        Some(em) => Arc::new(crate::stream::ObservingToolset {
            inner: base,
            emitter: em.clone(),
        }),
        None => base,
    };
    let mut config = config.clone();
    if let Some(em) = &emitter {
        config.stream_listener = Some(em.clone() as Arc<dyn rung_std::llm::StreamListener>);
    }
    let thread = thread_from(kind, lines, None, None);
    let carry = agentloop::Carry {
        state: LoopState::new(cap, cap),
        tools,
        config,
        python: None,
    };
    match agent::run(thread, carry) {
        Ok(r) => Ok((r.final_response, r.api_calls_made)),
        Err(f) => Err(f.reason),
    }
}

fn thread_from(
    kind: Kind,
    lines: &[Line],
    system_text: Option<&str>,
    user_material: Option<&str>,
) -> Thread {
    // System slot: caller-supplied system prompt FIRST, then the catalog
    // guarantee (read-only guard, etc.) so the mode's operating instructions
    // are never shadowed.
    let system_prompt = match system_text {
        Some(s) => format!("{s}\n\n---\n\n{}", kind.system()),
        None => kind.system().into(),
    };
    // User slot: prepared material becomes the FIRST user message, preceding
    // the session lines (which already end with the current ask).
    let mut messages = Vec::new();
    if let Some(m) = user_material {
        messages.push(ChatMessage::user(m));
    }
    messages.extend(lines.iter().filter_map(|l| match l.role.as_str() {
        "user" => Some(ChatMessage::user(l.text.clone())),
        "assistant" => Some(ChatMessage::assistant(l.text.clone())),
        _ => None,
    }));
    Thread {
        system_prompt,
        messages,
    }
}

fn last_assistant(lines: &[Line]) -> String {
    lines
        .iter()
        .rev()
        .find(|l| l.role == "assistant")
        .map(|l| l.text.clone())
        .unwrap_or_default()
}

fn read_text(origin: &Path, spec: &str) -> Result<String, String> {
    // Inline text unless the value starts with "@", in which case it names a
    // file (absolute, or relative to origin) whose bytes become the value.
    if let Some(path) = spec.strip_prefix('@') {
        let resolved = if Path::new(path).is_absolute() {
            Path::new(path).to_path_buf()
        } else {
            origin.join(path)
        };
        return std::fs::read_to_string(&resolved).map_err(|e| format!("{path}: {e}"));
    }
    Ok(spec.to_string())
}

fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

struct CwdGuard(PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

pub fn run_job(args: &Args, origin: &Path) -> Result<Outcome, String> {
    if let Some(id) = &args.task_id {
        crate::session::check_id(id)?;
    }
    let origin = origin
        .canonicalize()
        .unwrap_or_else(|_| origin.to_path_buf());
    let store = SessionStore::in_cwd(&origin);
    let id = match &args.task_id {
        Some(id) => id.clone(),
        None => crate::session::new_id(),
    };

    if args.background && !crate::background::in_child() {
        let prompt = args
            .prompt
            .as_ref()
            .ok_or_else(|| "background needs a prompt".to_string())?;
        let mut sess = store
            .try_load(&id)?
            .unwrap_or_else(|| Session::new(&id, args.kind, &origin));
        sess.kind = args.kind.as_str().into();
        sess.status = "queued".into();
        sess.lines.push(Line {
            role: "user".into(),
            text: prompt.clone(),
        });
        store.save(&sess)?;
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        let launch = crate::background::spawn_child(&exe, args, &origin, &id, &store)?;
        sess.pid = Some(launch.pid);
        sess.status = "running".into();
        store.save(&sess)?;
        return Ok(Outcome {
            task_id: id,
            text: format!("pid={} log={}", launch.pid, launch.log.display()),
            status: "running".into(),
            api_calls: 0,
            isolation_path: sess.isolation_path,
        });
    }

    let mut sess = store
        .try_load(&id)?
        .unwrap_or_else(|| Session::new(&id, args.kind, &origin));

    if args.prompt.is_none() {
        if store.try_load(&id)?.is_none() {
            return Err(format!("no session {id}"));
        }
        if sess.status == "running" && sess.pid.is_some_and(|p| !pid_alive(p)) {
            sess.status = "interrupted".into();
            store.save(&sess)?;
        }
        return Ok(Outcome {
            task_id: id,
            text: last_assistant(&sess.lines),
            status: sess.status,
            api_calls: 0,
            isolation_path: sess.isolation_path.clone(),
        });
    }

    let prompt = args.prompt.clone().unwrap();
    // New session or resume: don't duplicate the last identical user line
    // (background parent already wrote it).
    let already = sess
        .lines
        .last()
        .is_some_and(|l| l.role == "user" && l.text == prompt);
    if !already {
        sess.lines.push(Line {
            role: "user".into(),
            text: prompt,
        });
    }
    sess.kind = args.kind.as_str().into();
    sess.pid = Some(std::process::id());

    let _guard = CwdGuard(origin.clone());
    if args.isolation == IsolationMode::Worktree {
        let wt = if let Some(p) = &sess.isolation_path {
            let path = PathBuf::from(p);
            if path.is_dir() {
                crate::isolation::Worktree {
                    path,
                    branch: format!("rung-task/{id}"),
                    created: false,
                }
            } else {
                crate::isolation::ensure(&id, &origin)?
            }
        } else {
            crate::isolation::ensure(&id, &origin)?
        };
        std::env::set_current_dir(&wt.path)
            .map_err(|e| format!("chdir {}: {e}", wt.path.display()))?;
        sess.isolation_path = Some(wt.path.to_string_lossy().into_owned());
    }

    sess.status = "running".into();
    store.save(&sess)?;

    let emitter = if args.stream {
        Some(crate::stream::Emitter::new())
    } else {
        None
    };

    let mut config = match crate::config::load() {
        Ok(c) => c,
        Err(e) => {
            sess.status = "error".into();
            sess.lines.push(Line {
                role: "assistant".into(),
                text: e.clone(),
            });
            let _ = store.save(&sess);
            return Err(e);
        }
    };
    if let Some(em) = &emitter {
        config.stream_listener = Some(em.clone() as Arc<dyn rung_std::llm::StreamListener>);
    }
    let model = config.model.clone();

    let cap = args
        .max_iterations
        .unwrap_or_else(|| args.kind.max_iterations())
        .max(1);
    let mut roster: ToolRoster = args.kind.roster();
    if args.kind.allows_task() {
        let spawn = CatalogSpawn {
            config: config.clone(),
            store: store.clone(),
            max_iterations: cap,
            emitter: emitter.clone(),
        };
        let mut tasks = ToolCollection::new("task");
        tasks.admit(Task::new(Arc::new(spawn), 0, MAX_DEPTH));
        roster.add(tasks);
    }
    let base: Arc<dyn Toolset> = Arc::new(roster);
    let tools: Arc<dyn Toolset> = match &emitter {
        Some(em) => Arc::new(crate::stream::ObservingToolset {
            inner: base,
            emitter: em.clone(),
        }),
        None => base,
    };
    let system_prompt = match &args.system_prompt {
        Some(s) => Some(read_text(&origin, s)?),
        None => None,
    };
    let user_material = match &args.user_prompt {
        Some(u) => Some(read_text(&origin, u)?),
        None => None,
    };
    let thread = thread_from(
        args.kind,
        &sess.lines,
        system_prompt.as_deref(),
        user_material.as_deref(),
    );
    let carry = agentloop::Carry {
        state: LoopState::new(cap, cap),
        tools,
        config,
        python: None,
    };
    match agent::run(thread, carry) {
        Ok(r) => {
            sess.lines.push(Line {
                role: "assistant".into(),
                text: r.final_response.clone(),
            });
            sess.status = "completed".into();
            store.save(&sess)?;
            if let Some(em) = &emitter {
                em.emit_result(&id, &r, &model, sess.isolation_path.as_deref());
            }
            Ok(Outcome {
                task_id: id,
                text: r.final_response,
                status: "completed".into(),
                api_calls: r.api_calls_made,
                isolation_path: sess.isolation_path,
            })
        }
        Err(f) => {
            sess.status = "error".into();
            sess.lines.push(Line {
                role: "assistant".into(),
                text: f.reason.clone(),
            });
            let _ = store.save(&sess);
            if let Some(em) = &emitter {
                em.emit_error(&id, &f.reason, &model);
            }
            Err(f.reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Kind;

    #[test]
    fn thread_skips_non_chat_roles() {
        let lines = vec![
            Line {
                role: "system".into(),
                text: "nope".into(),
            },
            Line {
                role: "user".into(),
                text: "hi".into(),
            },
        ];
        let t = thread_from(Kind::Explore, &lines, None, None);
        assert_eq!(t.messages.len(), 1);
        assert_eq!(t.system_prompt, Kind::Explore.system());
    }

    #[test]
    fn system_prompt_prepends_catalog() {
        let lines = vec![Line {
            role: "user".into(),
            text: "q".into(),
        }];
        let t = thread_from(Kind::Explore, &lines, Some("caller system"), None);
        assert_eq!(
            t.system_prompt,
            format!("caller system\n\n---\n\n{}", Kind::Explore.system())
        );
        assert_eq!(t.messages.len(), 1);
    }

    #[test]
    fn user_material_becomes_first_user_message() {
        let lines = vec![Line {
            role: "user".into(),
            text: "q".into(),
        }];
        let t = thread_from(Kind::Explore, &lines, None, Some("## brief\nprepared"));
        assert_eq!(t.messages.len(), 2);
        let first = format!("{:?}", t.messages[0]);
        assert!(first.contains("user"), "{first}");
        assert!(first.contains("## brief\\nprepared"), "{first}");
        let second = format!("{:?}", t.messages[1]);
        assert!(second.contains("user"), "{second}");
        assert!(second.contains("q"), "{second}");
    }

    #[test]
    fn last_assistant_picks_tail() {
        let lines = vec![
            Line {
                role: "assistant".into(),
                text: "old".into(),
            },
            Line {
                role: "user".into(),
                text: "q".into(),
            },
            Line {
                role: "assistant".into(),
                text: "new".into(),
            },
        ];
        assert_eq!(last_assistant(&lines), "new");
    }
}
