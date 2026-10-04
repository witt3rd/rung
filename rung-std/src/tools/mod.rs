//! Canonical **tools** framework — the fourth building block in rung-std.
//!
//! Built-in file tools aim at OpenCode / grok-build parity for the work
//! those agents actually do: unique `edit`, numbered `read_file`, atomic
//! `write_file`, `glob`, and regex `grep`. Shell stays opt-in.

mod edit;
mod files;
mod fsutil;
mod patch;
mod shell;
mod skill;
mod task;
mod todo;
mod webfetch;

use crate::llm::{ImageSource, ToolDefinition};
use serde_json::Value;

pub use edit::EditFile;
pub use files::{Glob, Grep, ListFiles, ReadFile, WriteFile};
pub use patch::ApplyPatch;
pub use shell::Shell;
pub use skill::Skill;
pub use task::{MAX_DEPTH, Spawn, TASK_COMPLETED, Task, TaskRequest, TaskResult, WithoutTask};
pub use todo::Todo;
pub use webfetch::WebFetch;

// ─── Tool output ───────────────────────────────────────────────────────────────

/// What a tool hands back: text, and any images for the model to look at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolOutput {
    pub text: String,
    pub images: Vec<ImageSource>,
}

impl ToolOutput {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            images: Vec::new(),
        }
    }

    pub fn with_image(mut self, image: ImageSource) -> Self {
        self.images.push(image);
        self
    }

    /// Text only, for a caller that cannot carry images: each image becomes
    /// an [`ImageSource::omitted_note`] saying why, so none vanishes silently.
    pub fn into_text(self, why: &str) -> String {
        let mut text = self.text;
        for img in &self.images {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&img.omitted_note(why));
        }
        text
    }
}

/// Why an image is missing when a tool is run through the text-only
/// [`Tool::execute`] / [`Toolset::execute`].
pub const TEXT_ONLY_CALLER: &str = "this caller takes text only";

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

    /// Run the tool, keeping any images. The agent loop calls this. A tool
    /// that returns images overrides it, and makes [`Tool::execute`]
    /// `execute_output(..).map(|o| o.into_text(TEXT_ONLY_CALLER))`.
    fn execute_output(&self, input: &Value) -> Result<ToolOutput, String> {
        self.execute(input).map(ToolOutput::text)
    }
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

    fn find(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.as_ref())
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

    fn find(&self, name: &str) -> Result<&dyn Tool, String> {
        self.collections
            .iter()
            .rev()
            .find_map(|coll| coll.find(name))
            .ok_or_else(|| format!("unknown tool: {name}"))
    }

    pub fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        self.find(name)?.execute(input)
    }

    pub fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        self.find(name)?.execute_output(input)
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

pub trait Toolset: Send + Sync + std::fmt::Debug {
    fn definitions(&self) -> Vec<ToolDefinition>;
    fn execute(&self, name: &str, input: &Value) -> Result<String, String>;

    /// Run a tool, keeping any images. The agent loop calls this. The
    /// default wraps [`Toolset::execute`], so a wrapper that does not
    /// override it hands on text only; every wrapper in rung forwards it.
    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        self.execute(name, input).map(ToolOutput::text)
    }
}

impl Toolset for ToolRoster {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.definitions()
    }
    fn execute(&self, name: &str, input: &Value) -> Result<String, String> {
        self.execute(name, input)
    }
    fn execute_output(&self, name: &str, input: &Value) -> Result<ToolOutput, String> {
        self.execute_output(name, input)
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

/// Kernel extras that are not filesystem: patch, todos, fetch, skills.
/// Admit `task` separately — it needs a [`Spawn`].
pub fn kernel_tools() -> ToolCollection {
    let mut c = ToolCollection::new("kernel");
    c.admit(ApplyPatch);
    c.admit(Todo::new());
    c.admit(WebFetch);
    c.admit(Skill::in_cwd());
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(tag: &str) -> rung_testkit::TempDir {
        rung_testkit::TempDir::new(tag)
    }

    #[test]
    fn read_file_tool_executes() {
        let result = ReadFile
            .execute(
                &serde_json::json!({"path": concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")}),
            )
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
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn read_file_returns_an_image_file_as_an_image() {
        let dir = tmp("read-img");
        // Named .bin: the content decides, not the extension.
        let p = dir.join("frame-0001.bin");
        std::fs::write(&p, PNG).unwrap();
        let out = ReadFile
            .execute_output(&serde_json::json!({"path": p.to_str().unwrap()}))
            .unwrap();
        assert_eq!(
            out.text,
            format!("image {} (image/png, {} bytes)", p.display(), PNG.len())
        );
        assert_eq!(out.images, vec![ImageSource::from_bytes(PNG).unwrap()]);
    }

    #[test]
    fn read_file_through_the_roster_keeps_the_image() {
        let dir = tmp("read-img-roster");
        let p = dir.join("frame.png");
        std::fs::write(&p, PNG).unwrap();
        let mut r = ToolRoster::new();
        r.add(filesystem_tools());
        let out =
            Toolset::execute_output(&r, "read_file", &serde_json::json!({"path": p})).unwrap();
        assert_eq!(out.images.len(), 1);
    }

    #[test]
    fn read_file_text_only_caller_gets_a_note_not_the_data() {
        let dir = tmp("read-img-text");
        let p = dir.join("frame.png");
        std::fs::write(&p, PNG).unwrap();
        let text = ReadFile
            .execute(&serde_json::json!({"path": p.to_str().unwrap()}))
            .unwrap();
        assert!(
            text.ends_with(&format!(
                "[image omitted: image/png, {} bytes; {TEXT_ONLY_CALLER}]",
                PNG.len()
            )),
            "{text}"
        );
    }

    #[test]
    fn read_file_refuses_an_image_over_the_cap_without_reading_it() {
        let dir = tmp("read-img-big");
        let p = dir.join("huge.png");
        let mut f = std::fs::File::create(&p).unwrap();
        std::io::Write::write_all(&mut f, PNG).unwrap();
        f.set_len(crate::llm::image::MAX_IMAGE_BYTES as u64 + 1)
            .unwrap();
        let err = ReadFile
            .execute_output(&serde_json::json!({"path": p.to_str().unwrap()}))
            .unwrap_err();
        assert!(err.contains("over the 3750000-byte limit"), "{err}");
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
    }

    #[test]
    fn write_creates_parents() {
        let dir = tmp("tools");
        let tmp = dir.join("nested").join("w.txt");
        let result = WriteFile
            .execute(&serde_json::json!({"path": tmp.to_str().unwrap(), "content": "hello tools"}))
            .unwrap();
        assert!(result.contains("wrote"));
        assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "hello tools");
    }

    #[test]
    fn list_files_tool_executes() {
        let result = ListFiles
            .execute(&serde_json::json!({"path": concat!(env!("CARGO_MANIFEST_DIR"), "/src")}))
            .unwrap();
        assert!(result.contains("llm"));
    }

    #[test]
    fn grep_regex_and_glob() {
        let result = Grep
            .execute(&serde_json::json!({
                "pattern": "pub trait Tool",
                "path": concat!(env!("CARGO_MANIFEST_DIR"), "/src"),
                "glob": "**/*.rs"
            }))
            .unwrap();
        assert!(result.contains("pub trait Tool"));
    }

    #[test]
    fn glob_finds_rs() {
        let result = Glob
            .execute(&serde_json::json!({"pattern": "**/*.rs", "path": concat!(env!("CARGO_MANIFEST_DIR"), "/src")}))
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
        assert!(!names.contains(&"task".into()), "{names:?}");
        let k: Vec<_> = kernel_tools()
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        for n in ["apply_patch", "todo", "webfetch", "skill"] {
            assert!(k.contains(&n.into()), "{k:?}");
        }
        assert!(!k.contains(&"task".into()), "{k:?}");
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
