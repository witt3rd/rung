//! `task` — spawn a nested agent. Kernel primitive, not a product session.
//!
//! OpenCode and grok both cap nesting (default 1). The child is a full
//! agent loop; this tool is only admission + tagging the result. The
//! [`Spawn`] implementation owns how the child actually runs (nested
//! [`crate::agent`] loop, or a test fake).

use super::{Tool, ToolDefinition, Toolset};
use serde_json::Value;
use std::sync::Arc;

/// Default max nesting. Same as OpenCode `subagent_depth` / grok
/// `MAX_SUBAGENT_DEPTH`.
pub const MAX_DEPTH: u32 = 1;

#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub description: String,
    pub prompt: String,
    /// Named child profile (`explore`, `implement`, `review`). Product catalogs interpret this.
    pub subagent_type: Option<String>,
    /// Resume this child session if the Spawn persists transcripts.
    pub task_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub text: String,
    pub api_calls: u32,
    pub task_id: Option<String>,
}

/// How a child agent is actually run. The default is a nested AgentLoop;
/// tests inject a fake.
pub trait Spawn: Send + Sync + std::fmt::Debug {
    fn spawn(&self, req: &TaskRequest) -> Result<TaskResult, String>;
}

/// `task` tool. Holds a [`Spawn`] and the current nesting depth.
#[derive(Clone)]
pub struct Task {
    spawn: Arc<dyn Spawn>,
    pub depth: u32,
    pub max_depth: u32,
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("depth", &self.depth)
            .field("max_depth", &self.max_depth)
            .finish()
    }
}

impl Task {
    pub fn new(spawn: Arc<dyn Spawn>, depth: u32, max_depth: u32) -> Self {
        Self {
            spawn,
            depth,
            max_depth,
        }
    }

    pub fn at_limit(&self) -> bool {
        self.depth >= self.max_depth
    }
}

impl Tool for Task {
    fn name(&self) -> &'static str {
        "task"
    }
    fn description(&self) -> &'static str {
        "Launch a nested agent for a complex subtask. It returns one final message. \
         Do not use this for a single file read or grep — call those tools directly. \
         Nested tasks cannot spawn further tasks. Optional subagent_type: explore, implement, review. \
         Optional task_id resumes a prior child."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "Short (3-5 words) label for the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The full task. Say exactly what to return."
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Named child: explore (read-only), implement (full tools), review (read/search)"
                },
                "task_id": {
                    "type": "string",
                    "description": "Resume a previous child session instead of starting fresh"
                }
            },
            "required": ["description", "prompt"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        if self.at_limit() {
            return Err(format!("subagent depth limit reached ({})", self.max_depth));
        }
        let description = input["description"].as_str().unwrap_or("").trim();
        let prompt = input["prompt"].as_str().unwrap_or("").trim();
        if prompt.is_empty() {
            return Err("missing 'prompt'".into());
        }
        let req = TaskRequest {
            description: if description.is_empty() {
                "task".into()
            } else {
                description.into()
            },
            prompt: prompt.into(),
            subagent_type: input["subagent_type"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            task_id: input["task_id"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        };
        let label = xml_esc(&req.description);
        match self.spawn.spawn(&req) {
            Ok(r) => {
                let id = r
                    .task_id
                    .as_deref()
                    .map(|id| format!(" id=\"{}\"", xml_esc(id)))
                    .unwrap_or_default();
                Ok(format!(
                    "<task description=\"{label}\" state=\"completed\" calls=\"{}\"{id}>\n{}\n</task>",
                    r.api_calls,
                    r.text.trim()
                ))
            }
            Err(e) => Ok(format!(
                "<task description=\"{label}\" state=\"error\">\n{e}\n</task>"
            )),
        }
    }
}

fn xml_esc(s: &str) -> String {
    s.chars()
        .take(80)
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            c => c.to_string(),
        })
        .collect()
}

/// Hide `task` from a child roster so the default depth cap is structural.
#[derive(Debug)]
pub struct WithoutTask {
    inner: Arc<dyn Toolset>,
}

impl WithoutTask {
    pub fn new(inner: Arc<dyn Toolset>) -> Self {
        Self { inner }
    }
}

impl Toolset for WithoutTask {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.inner
            .definitions()
            .into_iter()
            .filter(|d| d.name != "task")
            .collect()
    }
    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        if name == "task" {
            return Err("subagent depth limit reached: child cannot spawn task".into());
        }
        self.inner.execute(name, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{Tool, ToolCollection, ToolRoster, Toolset};

    #[derive(Debug)]
    struct Fake;

    impl Spawn for Fake {
        fn spawn(&self, req: &TaskRequest) -> Result<TaskResult, String> {
            Ok(TaskResult {
                text: format!("done: {}", req.prompt),
                api_calls: 2,
                task_id: None,
            })
        }
    }

    #[derive(Debug)]
    struct Boom;

    impl Spawn for Boom {
        fn spawn(&self, _req: &TaskRequest) -> Result<TaskResult, String> {
            Err("child exploded".into())
        }
    }

    #[test]
    fn depth_zero_runs_and_tags_result() {
        let t = Task::new(Arc::new(Fake), 0, MAX_DEPTH);
        let out = t
            .execute(&serde_json::json!({
                "description": "sum",
                "prompt": "2+2"
            }))
            .unwrap();
        assert!(out.contains("state=\"completed\""), "{out}");
        assert!(out.contains("done: 2+2"), "{out}");
        assert!(out.contains("calls=\"2\""), "{out}");
    }

    #[test]
    fn at_max_depth_refuses_to_spawn() {
        let t = Task::new(Arc::new(Fake), 1, MAX_DEPTH);
        let err = t
            .execute(&serde_json::json!({
                "description": "nope",
                "prompt": "anything"
            }))
            .unwrap_err();
        assert!(err.contains("depth limit"), "{err}");
    }

    #[test]
    fn forwards_type_and_task_id() {
        #[derive(Debug)]
        struct Capture(std::sync::Mutex<Option<TaskRequest>>);
        impl Spawn for Capture {
            fn spawn(&self, req: &TaskRequest) -> Result<TaskResult, String> {
                *self.0.lock().unwrap() = Some(req.clone());
                Ok(TaskResult {
                    text: "ok".into(),
                    api_calls: 1,
                    task_id: req.task_id.clone(),
                })
            }
        }
        let cap = Arc::new(Capture(std::sync::Mutex::new(None)));
        let t = Task::new(cap.clone(), 0, MAX_DEPTH);
        let out = t
            .execute(&serde_json::json!({
                "description": "look",
                "prompt": "find it",
                "subagent_type": "explore",
                "task_id": "abc-1"
            }))
            .unwrap();
        assert!(out.contains("id=\"abc-1\""), "{out}");
        let got = cap.0.lock().unwrap().clone().unwrap();
        assert_eq!(got.subagent_type.as_deref(), Some("explore"));
        assert_eq!(got.task_id.as_deref(), Some("abc-1"));
    }

    #[test]
    fn empty_prompt_is_admission_error() {
        let t = Task::new(Arc::new(Fake), 0, MAX_DEPTH);
        let err = t
            .execute(&serde_json::json!({"description": "x", "prompt": "  "}))
            .unwrap_err();
        assert!(err.contains("prompt"), "{err}");
    }

    #[test]
    fn child_failure_is_tagged_not_a_tool_error() {
        let t = Task::new(Arc::new(Boom), 0, MAX_DEPTH);
        let out = t
            .execute(&serde_json::json!({
                "description": "boom",
                "prompt": "go"
            }))
            .unwrap();
        assert!(out.contains("state=\"error\""), "{out}");
        assert!(out.contains("child exploded"), "{out}");
    }

    #[test]
    fn without_task_hides_task_from_child() {
        let mut c = ToolCollection::new("t");
        c.admit(Task::new(Arc::new(Fake), 0, MAX_DEPTH));
        let mut r = ToolRoster::new();
        r.add(c);
        let inner: Arc<dyn Toolset> = Arc::new(r);
        let child = WithoutTask::new(inner.clone());
        assert!(inner.definitions().iter().any(|d| d.name == "task"));
        assert!(child.definitions().iter().all(|d| d.name != "task"));
        let err = child
            .execute("task", &serde_json::json!({"prompt": "x"}))
            .unwrap_err();
        assert!(err.contains("depth limit"), "{err}");
    }
}
