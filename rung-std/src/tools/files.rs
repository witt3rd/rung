//! Read, write, list, glob, grep.

use super::Tool;
use super::fsutil::{
    self, DEFAULT_READ_LIMIT, GLOB_LIMIT, GREP_LIMIT, format_line, glob_match, read_text, resolve,
    walk_files, write_atomic,
};
use regex::RegexBuilder;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ReadFile;

impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read a file or directory. Lines are `N→content` (1-indexed). \
         Use offset/limit for large files (default 2000 lines). Do not include the `N→` prefix when editing."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File or directory"},
                "offset": {"type": "integer", "description": "1-indexed start line (files only)"},
                "limit": {"type": "integer", "description": "Max lines (default 2000)"}
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let path = resolve(fsutil::req_str(input, "path")?);
        if path.is_dir() {
            return list_dir(&path);
        }
        let offset = fsutil::opt_usize(input, "offset").unwrap_or(1).max(1);
        let limit = fsutil::opt_usize(input, "limit").unwrap_or(DEFAULT_READ_LIMIT);
        let (_bom, text) = read_text(&path)?;
        let lines: Vec<&str> = text.split('\n').collect();
        // split('\n') yields a trailing empty on files that end with newline
        let n = if text.ends_with('\n') {
            lines.len().saturating_sub(1)
        } else {
            lines.len()
        };
        if offset > n && n > 0 {
            return Err(format!(
                "offset {offset} exceeds {} lines in {}",
                n,
                path.display()
            ));
        }
        let start = offset - 1;
        let end = (start + limit).min(n);
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate().take(end).skip(start) {
            out.push(format_line(i + 1, line));
        }
        if end < n {
            out.push(format!("… {} more lines", n - end));
        }
        if out.is_empty() {
            Ok(format!("{}: empty", path.display()))
        } else {
            Ok(out.join("\n"))
        }
    }
}

#[derive(Debug)]
pub struct WriteFile;

impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "write_file"
    }
    fn description(&self) -> &'static str {
        "Write a file, creating parent directories. Overwrites. Prefer edit for existing files."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            },
            "required": ["path", "content"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let path = resolve(fsutil::req_str(input, "path")?);
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or("missing 'content'")?;
        if path.is_dir() {
            return Err(format!("{} is a directory", path.display()));
        }
        let bom = path.exists() && read_text(&path).map(|(b, _)| b).unwrap_or(false);
        write_atomic(&path, content, bom)?;
        Ok(format!(
            "wrote {} bytes to {}",
            content.len(),
            path.display()
        ))
    }
}

#[derive(Debug)]
pub struct ListFiles;

impl Tool for ListFiles {
    fn name(&self) -> &'static str {
        "list_files"
    }
    fn description(&self) -> &'static str {
        "List a directory. Directories end with `/`. Sorted."
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
        list_dir(&resolve(fsutil::req_str(input, "path")?))
    }
}

fn list_dir(path: &Path) -> Result<String, String> {
    let mut entries: Vec<String> = std::fs::read_dir(path)
        .map_err(|e| format!("list_files {}: {e}", path.display()))?
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
    entries.sort();
    if entries.is_empty() {
        Ok("(empty directory)".into())
    } else {
        Ok(entries.join("\n"))
    }
}

#[derive(Debug)]
pub struct Glob;

impl Tool for Glob {
    fn name(&self) -> &'static str {
        "glob"
    }
    fn description(&self) -> &'static str {
        "Find files by glob (`*.rs`, `src/**/*.md`). Skips .git and target. Capped at 100."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob pattern"},
                "path": {"type": "string", "description": "Directory to search (default cwd)"}
            },
            "required": ["pattern"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let pattern = fsutil::req_str(input, "pattern")?;
        let root = input["path"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(resolve)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        if root.is_file() {
            return Err(format!("glob path must be a directory: {}", root.display()));
        }
        let mut hits: Vec<String> = walk_files(&root, 20)
            .into_iter()
            .filter_map(|p| {
                let rel = p.strip_prefix(&root).unwrap_or(&p);
                let rel_s = rel.to_string_lossy().replace('\\', "/");
                if glob_match(pattern, &rel_s) {
                    Some(p.display().to_string())
                } else {
                    None
                }
            })
            .collect();
        hits.sort();
        let truncated = hits.len() > GLOB_LIMIT;
        hits.truncate(GLOB_LIMIT);
        if hits.is_empty() {
            Ok("No files found".into())
        } else if truncated {
            Ok(format!(
                "{}\n\n(truncated at {GLOB_LIMIT}; narrow the pattern)",
                hits.join("\n")
            ))
        } else {
            Ok(hits.join("\n"))
        }
    }
}

#[derive(Debug)]
pub struct Grep;

impl Tool for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }
    fn description(&self) -> &'static str {
        "Search file contents with a regex. Optional glob filter. Skips binaries and .git/target. \
         Returns path:line: text. Capped at 200 hits."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Rust regex"},
                "path": {"type": "string", "description": "File or directory (default cwd)"},
                "glob": {"type": "string", "description": "Only files matching this glob"},
                "case_insensitive": {"type": "boolean"}
            },
            "required": ["pattern"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let pattern = fsutil::req_str(input, "pattern")?;
        let re = RegexBuilder::new(pattern)
            .case_insensitive(fsutil::opt_bool(input, "case_insensitive"))
            .build()
            .map_err(|e| format!("grep: bad pattern: {e}"))?;
        let root = input["path"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(resolve)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let glob = input["glob"].as_str();
        let files = if root.is_file() {
            vec![root.clone()]
        } else {
            walk_files(&root, 20)
        };
        let mut results = Vec::new();
        for file in files {
            if let Some(g) = glob {
                let name = file
                    .strip_prefix(&root)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");
                if !glob_match(g, &name)
                    && !glob_match(g, file.file_name().and_then(|n| n.to_str()).unwrap_or(""))
                {
                    continue;
                }
            }
            let Ok((_bom, text)) = read_text(&file) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    results.push(format!("{}:{}: {line}", file.display(), i + 1));
                    if results.len() >= GREP_LIMIT {
                        results.push(format!("(truncated at {GREP_LIMIT} matches)"));
                        return Ok(results.join("\n"));
                    }
                }
            }
        }
        if results.is_empty() {
            Ok("(no matches)".into())
        } else {
            Ok(results.join("\n"))
        }
    }
}
