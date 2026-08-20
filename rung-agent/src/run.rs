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

use crate::args::{Args, IsolationMode};
use crate::catalog::Kind;
use crate::session::{Line, Session, SessionStore};

#[derive(Debug, Clone)]
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
        match drive(&self.config, kind, &sess.lines, self.max_iterations) {
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
) -> Result<(String, u32), String> {
    let cap = max_iterations.min(kind.max_iterations()).max(1);
    let tools: Arc<dyn Toolset> = Arc::new(WithoutTask::new(Arc::new(kind.roster())));
    let thread = thread_from(kind, lines);
    let carry = agentloop::Carry {
        state: LoopState::new(cap, cap),
        tools,
        config: config.clone(),
        python: None,
    };
    match agent::run(thread, carry) {
        Ok(r) => Ok((r.final_response, r.api_calls_made)),
        Err(f) => Err(f.reason),
    }
}

fn thread_from(kind: Kind, lines: &[Line]) -> Thread {
    Thread {
        system_prompt: kind.system().into(),
        messages: lines
            .iter()
            .filter_map(|l| match l.role.as_str() {
                "user" => Some(ChatMessage::user(l.text.clone())),
                "assistant" => Some(ChatMessage::assistant(l.text.clone())),
                _ => None,
            })
            .collect(),
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

    let config = match crate::config::load() {
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
        };
        let mut tasks = ToolCollection::new("task");
        tasks.admit(Task::new(Arc::new(spawn), 0, MAX_DEPTH));
        roster.add(tasks);
    }
    let tools: Arc<dyn Toolset> = Arc::new(roster);
    let thread = thread_from(args.kind, &sess.lines);
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
        let t = thread_from(Kind::Explore, &lines);
        assert_eq!(t.messages.len(), 1);
        assert_eq!(t.system_prompt, Kind::Explore.system());
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
