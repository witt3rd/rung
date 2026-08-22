//! CLI flags. No extra parser crate — the binary is the composition example.

use crate::catalog::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationMode {
    None,
    Worktree,
}

impl IsolationMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "worktree" => Ok(Self::Worktree),
            other => Err(format!("unknown isolation '{other}' (none, worktree)")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Worktree => "worktree",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Args {
    pub task_id: Option<String>,
    pub kind: Kind,
    pub isolation: IsolationMode,
    pub background: bool,
    pub json: bool,
    pub stream: bool,
    pub max_iterations: Option<u32>,
    pub system_prompt: Option<String>,
    pub user_prompt: Option<String>,
    /// Comma-separated tool groups, or `none`. Overrides `--type` and config.
    pub tools: Option<String>,
    pub prompt: Option<String>,
    pub help: bool,
}

impl Args {
    pub fn parse(argv: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self, String> {
        let mut task_id = None;
        let mut kind = Kind::Implement;
        let mut isolation = IsolationMode::None;
        let mut background = false;
        let mut json = false;
        let mut stream = false;
        let mut max_iterations = None;
        let mut system_prompt = None;
        let mut user_prompt = None;
        let mut tools = None;
        let mut help = false;
        let mut prompt_parts: Vec<String> = Vec::new();
        let mut rest = false;
        let mut it = argv.into_iter().peekable();
        // skip argv0
        let _ = it.next();
        while let Some(raw) = it.next() {
            let a = raw.as_ref();
            if rest {
                prompt_parts.push(a.to_string());
                continue;
            }
            match a {
                "--" => rest = true,
                "-h" | "--help" => help = true,
                "--background" => background = true,
                "--json" => json = true,
                "--stream" => stream = true,
                "--task-id" => {
                    task_id = Some(need("--task-id", it.next())?);
                }
                "--type" => {
                    kind = Kind::parse(&need("--type", it.next())?)?;
                }
                "--isolation" => {
                    isolation = IsolationMode::parse(&need("--isolation", it.next())?)?;
                }
                "--max-iterations" => {
                    let v = need("--max-iterations", it.next())?;
                    max_iterations = Some(
                        v.parse()
                            .map_err(|_| format!("--max-iterations: not a number ({v})"))?,
                    );
                }
                "--system-prompt" => {
                    system_prompt = Some(need("--system-prompt", it.next())?);
                }
                "--user-prompt" => {
                    user_prompt = Some(need("--user-prompt", it.next())?);
                }
                "--tools" => {
                    tools = Some(need("--tools", it.next())?);
                }
                s if s.starts_with("--system-prompt=") => {
                    system_prompt = Some(s["--system-prompt=".len()..].to_string());
                }
                s if s.starts_with("--user-prompt=") => {
                    user_prompt = Some(s["--user-prompt=".len()..].to_string());
                }
                s if s.starts_with("--tools=") => {
                    tools = Some(s["--tools=".len()..].to_string());
                }
                s if s.starts_with("--task-id=") => {
                    task_id = Some(s["--task-id=".len()..].to_string());
                }
                s if s.starts_with("--type=") => {
                    kind = Kind::parse(&s["--type=".len()..])?;
                }
                s if s.starts_with("--isolation=") => {
                    isolation = IsolationMode::parse(&s["--isolation=".len()..])?;
                }
                s if s.starts_with('-') => return Err(format!("unknown flag {s}")),
                other => prompt_parts.push(other.to_string()),
            }
        }
        let prompt = {
            let joined = prompt_parts.join(" ");
            if joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        };
        Ok(Self {
            task_id,
            kind,
            isolation,
            background,
            json,
            stream,
            max_iterations,
            system_prompt,
            user_prompt,
            tools,
            prompt,
            help,
        })
    }
}

fn need(flag: &str, v: Option<impl AsRef<str>>) -> Result<String, String> {
    v.map(|s| s.as_ref().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{flag} needs a value"))
}

pub fn usage() -> &'static str {
    "\
rung-agent — headless agent (not a coding product; coding is one use)

  rung-agent [OPTIONS] [PROMPT]
  rung-agent --task-id ID              print status / last answer
  rung-agent --task-id ID PROMPT       resume that session

Options:
  --tools none|read,write,shell,web,skill,todo,python,task
                                    tool groups for this call (overrides --type and config)
  --type explore|implement|review   preset alias for --tools (default implement)
  --task-id ID                      resume or poll
  --isolation none|worktree         default none
  --background                      spawn a child, print task_id and pid
  --json                            emit one JSON Outcome object on stdout
  --stream                          emit NDJSON trace events on stdout
  --max-iterations N
  --system-prompt TEXT            system message; TEXT or @file path
  --user-prompt TEXT             first user message; TEXT or @file path
  -h, --help

Config: $XDG_CONFIG_HOME/rung/config.yaml  (llm: base_url, model, api_key_env, …)
        RUNG_CONFIG overrides the path. Env RUNG_* / XAI_API_KEY wins over the file.
        API key is optional; empty means no Authorization header.
Sessions: <cwd>/.rung/sessions/<id>.json
Worktrees: <repo>.wt/rung-task--<id>  (branch rung-task/<id>)
"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_and_prompt() {
        let a = Args::parse([
            "rung-agent",
            "--type",
            "explore",
            "--isolation=worktree",
            "--background",
            "--task-id",
            "abc",
            "look",
            "around",
        ])
        .unwrap();
        assert_eq!(a.kind, Kind::Explore);
        assert_eq!(a.isolation, IsolationMode::Worktree);
        assert!(a.background);
        assert_eq!(a.task_id.as_deref(), Some("abc"));
        assert_eq!(a.prompt.as_deref(), Some("look around"));
    }

    #[test]
    fn parses_tools() {
        let a = Args::parse(["rung-agent", "--tools", "none", "hi"]).unwrap();
        assert_eq!(a.tools.as_deref(), Some("none"));
        let b = Args::parse(["rung-agent", "--tools=read,web", "hi"]).unwrap();
        assert_eq!(b.tools.as_deref(), Some("read,web"));
    }

    #[test]
    fn parses_prompts() {
        let a = Args::parse([
            "rung-agent",
            "--system-prompt",
            "you are X",
            "--user-prompt",
            "material",
            "q",
        ])
        .unwrap();
        assert_eq!(a.system_prompt.as_deref(), Some("you are X"));
        assert_eq!(a.user_prompt.as_deref(), Some("material"));
        assert_eq!(a.prompt.as_deref(), Some("q"));
        assert!(Args::parse(["rung-agent", "--system-prompt"]).is_err());
    }

    #[test]
    fn parses_prompt_equals_forms() {
        let a = Args::parse([
            "rung-agent",
            "--user-prompt=@brief.md",
            "--system-prompt=sys",
            "q",
        ])
        .unwrap();
        assert_eq!(a.user_prompt.as_deref(), Some("@brief.md"));
        assert_eq!(a.system_prompt.as_deref(), Some("sys"));
    }

    #[test]
    fn parses_stream() {
        let a = Args::parse(["rung-agent", "--stream", "q"]).unwrap();
        assert!(a.stream);
        assert!(!a.json);
    }

    #[test]
    fn unknown_flag() {
        assert!(Args::parse(["rung-agent", "--nope"]).is_err());
    }
}
