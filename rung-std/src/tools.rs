//! Canonical **tools** framework — the fourth building block in rung-std.
//!
//! ## What this is
//!
//! A tool roster framework with built-in file-system tools. The [`Tool`]
//! trait declares the interface every tool must satisfy; [`ToolCollection`]
//! groups tools by name; [`ToolRoster`] assembles one or more collections
//! and provides the unified [`Toolset`] interface the [`agent`](crate::agent)
//! ladder consumes.
//!
//! ## Theory here, roster in deployment
//!
//! The [`Tool`] trait and built-in collections live here; the concrete
//! roster (which collections, in what order) is the caller's. The agent
//! ladder takes `&dyn Toolset` — the narrow projection it needs — and has no
//! knowledge of which tools are admitted or where they came from.
//! `nothing-further-required`.
//!
//! ## Tool contract
//!
//! Tools are stateless, blocking, and synchronous. All state lives in the
//! conversation history. Execution happens inside the transition body — the
//! verb on the arrow (`the-law`) law: a tool call is a verb and lives on the arrow
//! (`the-law`).
//!
//! ## What this module could not say
//!
//! The roster is not a pool — there is no qualification filter, no
//! standing predicate, and no gate. Tools are equal; only the name
//! distinguishes them, and duplicates are resolved by declaration order.

use crate::llm::ToolDefinition;
use serde_json::Value;

// ─── Tool trait ────────────────────────────────────────────────────────────────

/// A tool the agent can dispatch to.
///
/// Implementations are zero-size unit structs with `&'static str` constants.
/// The trait is object-safe — tools are stored as `Box<dyn Tool>` inside
/// a [`ToolCollection`].
pub trait Tool: Send + Sync + std::fmt::Debug {
    /// The name exposed to the model (e.g. `"list_files"`).
    fn name(&self) -> &'static str;
    /// A short description the model uses to decide when to call this tool.
    fn description(&self) -> &'static str;
    /// JSON Schema of the tool's parameters.
    fn input_schema(&self) -> Value;
    /// Execute the tool with the given arguments.
    ///
    /// Returns the result string on success, or an error message on failure.
    /// The result is appended to the conversation as a tool-result block.
    fn execute(&self, input: &Value) -> Result<String, String>;
}

// ─── ToolCollection ───────────────────────────────────────────────────────────

/// A named group of tools.
///
/// Tools within a collection must have unique names (enforced by declaration
/// order — later admissions of the same name shadow earlier ones for
/// execution, but both appear in definitions).
#[derive(Debug)]
pub struct ToolCollection {
    pub name: &'static str,
    tools: Vec<(String, Box<dyn Tool>)>,
}

impl ToolCollection {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            tools: Vec::new(),
        }
    }

    /// Admit a tool and return `self` for chaining.
    pub fn admit(&mut self, tool: impl Tool + 'static) -> &mut Self {
        let name = tool.name().to_string();
        self.tools.push((name, Box::new(tool)));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|(_, t)| ToolDefinition::new(t.name(), t.description(), t.input_schema()))
            .collect()
    }

    fn execute(&self, name: &str, input: &Value) -> Option<Result<String, String>> {
        self.tools
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.execute(input))
    }
}

// ─── ToolRoster ─────────────────────────────────────────────────────────────

/// Assembles one or more [`ToolCollection`]s into a unified tool set.
///
/// The `Toolset` trait implementation provides the narrow interface the
/// [`agent`](crate::agent) ladder consumes: definitions for the LLM request
/// and execution for tool dispatch. The agent has no knowledge of which
/// collections are admitted or where a tool came from.
///
/// ## Duplicate names
///
/// Collections added later take priority for execution (last-wins). This
/// lets a caller override a built-in tool by admitting a replacement in a
/// later collection. Definitions are deduplicated by name on the same basis.
#[derive(Debug)]
pub struct ToolRoster {
    collections: Vec<ToolCollection>,
}

impl ToolRoster {
    pub fn new() -> Self {
        Self {
            collections: Vec::new(),
        }
    }

    /// Add a collection and return `self` for chaining.
    pub fn add(&mut self, collection: ToolCollection) -> &mut Self {
        self.collections.push(collection);
        self
    }

    /// All tool definitions, in declaration order, last-wins for duplicates.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<ToolDefinition> = Vec::new();
        // Iterate in reverse so first occurrence of a duplicate (which is the
        // later-added collection's version) wins. Then reverse the output
        // to restore declaration order.
        for coll in self.collections.iter().rev() {
            for def in coll.definitions().into_iter().rev() {
                let name = def.name.clone();
                if !seen.contains(&name) {
                    seen.push(name);
                    out.push(def);
                }
            }
        }
        out.reverse();
        out
    }

    /// Execute a tool by name. Last-added collection wins for duplicates.
    pub fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        for coll in self.collections.iter().rev() {
            if let Some(result) = coll.execute(name, input) {
                return result;
            }
        }
        Err(format!("unknown tool: {name}"))
    }

    /// Which collection a named tool comes from.
    pub fn collection_of(&self, name: &str) -> Option<&'static str> {
        for coll in self.collections.iter().rev() {
            if coll.definitions().iter().any(|d| d.name == name) {
                return Some(coll.name);
            }
        }
        None
    }
}

impl Default for ToolRoster {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Toolset trait ────────────────────────────────────────────────────────────

/// The narrow interface the agent ladder consumes.
///
/// Separated from [`ToolRoster`] so the agent carry can hold a `&dyn
/// Toolset` trait object — the ladder is not generic over a concrete
/// roster type, and integration tests can supply a mock executor.
pub trait Toolset: std::fmt::Debug {
    fn definitions(&self) -> Vec<ToolDefinition>;
    fn execute(&self, name: &str, input: &Value) -> Result<String, String>;
}

impl Toolset for ToolRoster {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.definitions()
    }
    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        self.execute(name, input)
    }
}

// ─── Built-in tool collections ────────────────────────────────────────────────

/// Return the file-system tools WITHOUT the shell.
///
/// Shell grants arbitrary code execution — it is opt-in only. Use
/// [`filesystem_tools_with_shell`] if the caller explicitly opts in.
pub fn filesystem_tools() -> ToolCollection {
    let mut c = ToolCollection::new("filesystem");
    c.admit(ReadFile);
    c.admit(WriteFile);
    c.admit(ListFiles);
    c.admit(Grep);
    c
}

/// Return the file-system tools WITH the shell enabled.
pub fn filesystem_tools_with_shell() -> ToolCollection {
    let mut c = filesystem_tools();
    c.admit(Shell);
    c
}

// ─── File-system tool implementations ─────────────────────────────────────────

#[derive(Debug)]
struct ReadFile;
impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read the contents of a file at the given path. Returns the file text or an error."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file to read"}
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("missing 'path'")?;
        std::fs::read_to_string(path).map_err(|e| format!("read_file: {e}"))
    }
}

#[derive(Debug)]
struct WriteFile;
impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "write_file"
    }
    fn description(&self) -> &'static str {
        "Write content to a file. Creates the file if it does not exist, overwrites if it does."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the file to write"},
                "content": {"type": "string", "description": "Content to write to the file"}
            },
            "required": ["path", "content"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("missing 'path'")?;
        let content = input["content"].as_str().ok_or("missing 'content'")?;
        std::fs::write(path, content).map_err(|e| format!("write_file: {e}"))?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    }
}

#[derive(Debug)]
struct ListFiles;
impl Tool for ListFiles {
    fn name(&self) -> &'static str {
        "list_files"
    }
    fn description(&self) -> &'static str {
        "List files and directories at the given path. Returns one entry per line."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Directory to list"}
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("missing 'path'")?;
        let entries: Vec<String> = std::fs::read_dir(path)
            .map_err(|e| format!("list_files: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.path().is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();
        if entries.is_empty() {
            Ok("(empty directory)".into())
        } else {
            Ok(entries.join("\n"))
        }
    }
}

#[derive(Debug)]
struct Grep;
impl Tool for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }
    fn description(&self) -> &'static str {
        "Search for a pattern in files under a directory. Returns matching lines with file:line prefix."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Pattern to search for (basic substring match)"},
                "path": {"type": "string", "description": "Directory or file to search in"}
            },
            "required": ["pattern", "path"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let pattern = input["pattern"].as_str().ok_or("missing 'pattern'")?;
        let path = input["path"].as_str().ok_or("missing 'path'")?;
        let p = std::path::Path::new(path);
        let mut results = Vec::new();

        if p.is_file() {
            grep_file(p, pattern, &mut results).map_err(|e| format!("grep: {e}"))?;
        } else if p.is_dir() {
            for entry in walkdir::WalkDir::new(p)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
            {
                let _ = grep_file(entry.path(), pattern, &mut results);
                if results.len() > 200 {
                    results.push("(truncated at 200 matches)".into());
                    break;
                }
            }
        } else {
            return Err(format!("grep: {path}: no such file or directory"));
        }

        if results.is_empty() {
            Ok("(no matches)".into())
        } else {
            Ok(results.join("\n"))
        }
    }
}

fn grep_file(path: &std::path::Path, pattern: &str, out: &mut Vec<String>) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path)?;
    for (i, line) in content.lines().enumerate() {
        if line.contains(pattern) {
            out.push(format!("{}:{}: {line}", path.display(), i + 1));
        }
    }
    Ok(())
}

/// Local recursive file walker. We ship our own rather than pulling in the
/// `walkdir` crate, so the grep tool has no extra dependency.
mod walkdir {
    pub struct WalkDir {
        entries: Vec<std::path::PathBuf>,
    }

    impl WalkDir {
        pub fn new(root: &std::path::Path) -> Self {
            let mut entries = Vec::new();
            let _ = collect_files(root, 0, &mut entries);
            Self { entries }
        }

        pub fn into_iter(self) -> impl Iterator<Item = Result<DirEntry, std::io::Error>> {
            self.entries.into_iter().map(|p| Ok(DirEntry { path: p }))
        }
    }

    pub struct DirEntry {
        path: std::path::PathBuf,
    }

    impl DirEntry {
        pub fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    fn collect_files(
        root: &std::path::Path,
        depth: usize,
        out: &mut Vec<std::path::PathBuf>,
    ) -> std::io::Result<()> {
        if depth > 10 {
            return Ok(());
        }
        if root.is_file() {
            out.push(root.to_path_buf());
            return Ok(());
        }
        if root.is_dir() {
            for entry in std::fs::read_dir(root)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    out.push(path);
                } else if path.is_dir() {
                    collect_files(&path, depth + 1, out)?;
                }
            }
        }
        Ok(())
    }
}

// ─── Shell tool ────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Shell;
impl Tool for Shell {
    fn name(&self) -> &'static str {
        "shell"
    }
    fn description(&self) -> &'static str {
        "Execute a shell command via `bash -c`. Returns stdout, stderr, and exit code. \
         Commands that modify the filesystem (rm -rf, dd, mkfs) will execute but are \
         logged with a warning."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to execute"}
            },
            "required": ["command"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let command = input["command"].as_str().ok_or("missing 'command'")?;
        if command.trim().is_empty() {
            return Err("shell: empty command".into());
        }

        let destructive = ["rm -rf /", "dd if=", "mkfs.", ":(){ :|:& };:", "rm -rf "]
            .iter()
            .any(|pat| command.contains(pat));
        if destructive {
            eprintln!("\x1b[1;33m⚠ shell: destructive pattern detected — '{command}'\x1b[0m");
        }

        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| format!("shell: {e}"))?;

        let mut parts = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);

        if !stdout.trim().is_empty() {
            parts.push(stdout.trim().to_string());
        }
        if !stderr.trim().is_empty() {
            parts.push(format!("[stderr]\n{}", stderr.trim()));
        }
        parts.push(format!("[exit: {code}]"));

        Ok(parts.join("\n"))
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_file_tool_executes() {
        let result = ReadFile
            .execute(&serde_json::json!({"path": "Cargo.toml"}))
            .unwrap();
        assert!(result.contains("[package]"));
    }

    #[test]
    fn write_file_tool_executes() {
        let tmp = std::env::temp_dir().join("rung-std-tools-test-write.txt");
        let result = WriteFile
            .execute(&serde_json::json!({"path": tmp.to_str().unwrap(), "content": "hello tools"}))
            .unwrap();
        assert!(result.contains("wrote"));
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert_eq!(content, "hello tools");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn list_files_tool_executes() {
        let result = ListFiles
            .execute(&serde_json::json!({"path": "src"}))
            .unwrap();
        // rung-std/src has llm/, principals.rs, questions.rs, lib.rs, (tools.rs)
        assert!(result.contains("llm"));
    }

    #[test]
    fn grep_tool_executes() {
        let result = Grep
            .execute(&serde_json::json!({"pattern": "Tool", "path": "src"}))
            .unwrap();
        assert!(result.contains("pub trait Tool"));
    }

    #[test]
    fn shell_tool_output_format() {
        let result = Shell
            .execute(&serde_json::json!({"command": "echo hello"}))
            .unwrap();
        assert!(result.contains("hello"));
        assert!(result.contains("[exit: 0]"));
    }

    #[test]
    fn shell_tool_empty_command_rejected() {
        let result = Shell.execute(&serde_json::json!({"command": "  "}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty command"));
    }

    #[test]
    fn roster_collections_last_wins_for_execute() {
        // A "mock-grep" tool that overrides the built-in grep.
        #[derive(Debug)]
        struct MockGrep;
        impl Tool for MockGrep {
            fn name(&self) -> &'static str {
                "grep"
            }
            fn description(&self) -> &'static str {
                "mock"
            }
            fn input_schema(&self) -> Value {
                serde_json::json!({})
            }
            fn execute(&self, _input: &Value) -> Result<String, String> {
                Ok("mock result".into())
            }
        }

        let mut roster = ToolRoster::new();
        roster.add(filesystem_tools());
        let mut override_coll = ToolCollection::new("test-override");
        override_coll.admit(MockGrep);
        roster.add(override_coll);

        let result = roster
            .execute("grep", &serde_json::json!({"pattern": "x", "path": "."}))
            .unwrap();
        assert_eq!(result, "mock result");
    }

    #[test]
    fn roster_unknown_tool_returns_error() {
        let roster = ToolRoster::new();
        let result = roster.execute("nonexistent", &Value::Null);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown tool"));
    }

    #[test]
    fn roster_definitions_deduplicates_by_name() {
        #[derive(Debug)]
        struct MockGrep;
        impl Tool for MockGrep {
            fn name(&self) -> &'static str {
                "grep"
            }
            fn description(&self) -> &'static str {
                "mock"
            }
            fn input_schema(&self) -> Value {
                serde_json::json!({})
            }
            fn execute(&self, _input: &Value) -> Result<String, String> {
                Ok("mock result".into())
            }
        }

        let mut roster = ToolRoster::new();
        roster.add(filesystem_tools());
        let mut override_coll = ToolCollection::new("test-override");
        override_coll.admit(MockGrep);
        roster.add(override_coll);

        let defs = roster.definitions();
        let grep_defs: Vec<_> = defs.iter().filter(|d| d.name == "grep").collect();
        assert_eq!(grep_defs.len(), 1, "duplicate names must be deduplicated");
        assert_eq!(grep_defs[0].description, "mock", "last-added wins");
    }

    #[test]
    fn filesystem_tools_excludes_shell() {
        let coll = filesystem_tools();
        let defs = coll.definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(!names.contains(&"shell"), "shell must be opt-in only");
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"grep"));
    }

    #[test]
    fn filesystem_tools_with_shell_includes_shell() {
        let coll = filesystem_tools_with_shell();
        let defs = coll.definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"shell"));
    }

    #[test]
    fn toolset_trait_object_works() {
        let mut roster = ToolRoster::new();
        roster.add(filesystem_tools());
        let tools: &dyn Toolset = &roster;
        let defs = tools.definitions();
        assert!(defs.iter().any(|d| d.name == "read_file"));
        let result = tools
            .execute("list_files", &serde_json::json!({"path": "src"}))
            .unwrap();
        assert!(result.contains("llm"));
    }
}
