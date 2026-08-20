//! Canonical **tools** framework — the fourth building block in rung-std.
//!
//! Built-in file tools aim at OpenCode / grok-build parity for the work
//! those agents actually do: unique `edit`, numbered `read_file`, atomic
//! `write_file`, `glob`, and regex `grep`. Shell stays opt-in.

mod edit;
mod files;
mod fsutil;
mod shell;

use crate::llm::ToolDefinition;
use serde_json::Value;

pub use edit::EditFile;
pub use files::{Glob, Grep, ListFiles, ReadFile, WriteFile};
pub use shell::Shell;

// ─── Tool trait ────────────────────────────────────────────────────────────────

/// A tool the agent can dispatch to.
///
/// The trait is object-safe — tools are stored as `Box<dyn Tool>` inside
/// a [`ToolCollection`].
pub trait Tool: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, String>;
}

// ─── ToolCollection ───────────────────────────────────────────────────────────

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

    pub fn add(&mut self, collection: ToolCollection) -> &mut Self {
        self.collections.push(collection);
        self
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<ToolDefinition> = Vec::new();
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

    pub fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        for coll in self.collections.iter().rev() {
            if let Some(result) = coll.execute(name, input) {
                return result;
            }
        }
        Err(format!("unknown tool: {name}"))
    }

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

/// File tools without shell. Includes `edit` (unique search/replace).
pub fn filesystem_tools() -> ToolCollection {
    let mut c = ToolCollection::new("filesystem");
    c.admit(ReadFile);
    c.admit(WriteFile);
    c.admit(EditFile);
    c.admit(ListFiles);
    c.admit(Glob);
    c.admit(Grep);
    c
}

pub fn filesystem_tools_with_shell() -> ToolCollection {
    let mut c = filesystem_tools();
    c.admit(Shell);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rung-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn read_file_tool_executes() {
        let result = ReadFile
            .execute(&serde_json::json!({"path": "Cargo.toml"}))
            .unwrap();
        assert!(result.contains("[package]") || result.contains("→"));
        assert!(result.contains("package") || result.contains("[package]"));
    }

    #[test]
    fn read_file_numbers_offset_and_limit() {
        let dir = tmp("read-slice");
        let p = dir.join("lines.txt");
        std::fs::write(&p, "alpha\nbeta\ngamma\ndelta\n").unwrap();
        let out = ReadFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "offset": 2,
                "limit": 2
            }))
            .unwrap();
        assert_eq!(out, "2→beta\n3→gamma\n… 1 more lines");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_file_refuses_binary() {
        let dir = tmp("read-bin");
        let p = dir.join("blob.bin");
        std::fs::write(&p, b"ok\0nope").unwrap();
        let err = ReadFile
            .execute(&serde_json::json!({"path": p.to_str().unwrap()}))
            .unwrap_err();
        assert!(err.contains("binary"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_directory_lists_with_slash() {
        let dir = tmp("read-dir");
        std::fs::create_dir(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        let out = ReadFile
            .execute(&serde_json::json!({"path": dir.to_str().unwrap()}))
            .unwrap();
        assert!(out.contains("a.txt"), "{out}");
        assert!(out.contains("sub/"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_creates_parents() {
        let dir = std::env::temp_dir().join(format!("rung-tools-{}", std::process::id()));
        let tmp = dir.join("nested").join("w.txt");
        let result = WriteFile
            .execute(&serde_json::json!({"path": tmp.to_str().unwrap(), "content": "hello tools"}))
            .unwrap();
        assert!(result.contains("wrote"));
        assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "hello tools");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_files_tool_executes() {
        let result = ListFiles
            .execute(&serde_json::json!({"path": "src"}))
            .unwrap();
        assert!(result.contains("llm"));
    }

    #[test]
    fn grep_regex_and_glob() {
        let result = Grep
            .execute(&serde_json::json!({
                "pattern": "pub trait Tool",
                "path": "src",
                "glob": "**/*.rs"
            }))
            .unwrap();
        assert!(result.contains("pub trait Tool"));
    }

    #[test]
    fn glob_finds_rs() {
        let result = Glob
            .execute(&serde_json::json!({"pattern": "**/*.rs", "path": "src"}))
            .unwrap();
        assert!(result.contains("tools"));
    }

    #[test]
    fn edit_round_trip() {
        let dir = tmp("edit-unique");
        let p = dir.join("a.rs");
        std::fs::write(&p, "fn a() {}\nfn b() {}\n").unwrap();
        EditFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "old_string": "fn b() {}",
                "new_string": "fn b() { 1 }"
            }))
            .unwrap();
        let body = std::fs::read_to_string(&p).unwrap();
        assert_eq!(body, "fn a() {}\nfn b() { 1 }\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_ambiguous_fails_unless_replace_all() {
        let dir = tmp("edit-amb");
        let p = dir.join("x.txt");
        std::fs::write(&p, "x = 1\nx = 1\n").unwrap();
        let err = EditFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "old_string": "x = 1",
                "new_string": "x = 2"
            }))
            .unwrap_err();
        assert!(err.contains("more than once"), "{err}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "x = 1\nx = 1\n");
        EditFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "old_string": "x = 1",
                "new_string": "x = 2",
                "replace_all": true
            }))
            .unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "x = 2\nx = 2\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_mixed_indent_duplicates_fail_closed() {
        let dir = tmp("edit-mixed-indent");
        let p = dir.join("d.txt");
        let original = "  x = 1\nx = 1\n";
        std::fs::write(&p, original).unwrap();
        let err = EditFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "old_string": "x = 1",
                "new_string": "x = 2"
            }))
            .unwrap_err();
        assert!(err.contains("more than once"), "{err}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_indent_tolerant_unique() {
        let dir = tmp("edit-indent");
        let p = dir.join("g.rs");
        std::fs::write(&p, "    fn go() {\n        x\n    }\n").unwrap();
        EditFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "old_string": "fn go() {\n        x\n    }",
                "new_string": "fn go() {\n        y\n    }"
            }))
            .unwrap();
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("y"), "{body}");
        assert!(!body.contains("        x\n"), "{body}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_missing_old_hints() {
        let dir = tmp("edit-miss");
        let p = dir.join("m.txt");
        std::fs::write(&p, "alpha\nbeta line\ngamma\n").unwrap();
        let err = EditFile
            .execute(&serde_json::json!({
                "path": p.to_str().unwrap(),
                "old_string": "beta line extra",
                "new_string": "nope"
            }))
            .unwrap_err();
        assert!(err.contains("not found"), "{err}");
        assert!(err.contains("beta") || err.contains("Nearby"), "{err}");
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "alpha\nbeta line\ngamma\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn glob_skips_git_and_target() {
        let dir = tmp("glob-skip");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("target")).unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join(".git").join("hidden.rs"), "fn hide() {}").unwrap();
        std::fs::write(dir.join("target").join("out.rs"), "fn out() {}").unwrap();
        std::fs::write(dir.join("src").join("keep.rs"), "fn keep() {}").unwrap();
        let out = Glob
            .execute(&serde_json::json!({
                "pattern": "**/*.rs",
                "path": dir.to_str().unwrap()
            }))
            .unwrap();
        assert!(out.contains("keep.rs"), "{out}");
        assert!(!out.contains("hidden.rs"), "{out}");
        assert!(!out.contains("out.rs"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_roster_has_edit_not_shell() {
        let names: Vec<_> = filesystem_tools()
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert!(names.contains(&"edit".into()), "{names:?}");
        assert!(names.contains(&"read_file".into()), "{names:?}");
        assert!(names.contains(&"write_file".into()), "{names:?}");
        assert!(names.contains(&"glob".into()), "{names:?}");
        assert!(names.contains(&"grep".into()), "{names:?}");
        assert!(!names.contains(&"shell".into()), "{names:?}");
        let with: Vec<_> = filesystem_tools_with_shell()
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert!(with.contains(&"shell".into()), "{with:?}");
        assert!(with.contains(&"edit".into()), "{with:?}");
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
        assert_eq!(
            roster.execute("grep", &serde_json::json!({})).unwrap(),
            "mock result"
        );
        assert!(roster.definitions().iter().any(|d| d.name == "edit"));
    }

    #[test]
    fn roster_unknown_tool() {
        let roster = ToolRoster::new();
        assert!(roster.execute("nope", &serde_json::json!({})).is_err());
    }
}
