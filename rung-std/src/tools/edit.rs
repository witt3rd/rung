//! `edit` — unique search/replace, with the OpenCode fallbacks that make it stick.

use super::Tool;
use super::fsutil::{self, detect_ending, read_text, resolve, to_ending, write_atomic};
use serde_json::Value;
use std::path::Path;

#[derive(Debug)]
pub struct EditFile;

impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "edit"
    }
    fn description(&self) -> &'static str {
        "Replace exact text in a file. old_string must match uniquely unless replace_all is true. \
         Indentation must match the file (not read-tool line prefixes). Empty old_string creates a new file. \
         Prefer this over write for existing files."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File to edit"},
                "old_string": {
                    "type": "string",
                    "description": "Text to find. Must be unique in the file unless replace_all."
                },
                "new_string": {"type": "string", "description": "Replacement text"},
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace every occurrence (default false)"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let path = resolve(fsutil::req_str(input, "path")?);
        let old = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or("missing 'old_string'")?;
        let new = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or("missing 'new_string'")?;
        let replace_all = fsutil::opt_bool(input, "replace_all");
        apply(&path, old, new, replace_all)
    }
}

fn apply(path: &Path, old: &str, new: &str, replace_all: bool) -> Result<String, String> {
    if old == new {
        return Err("no change: old_string and new_string are identical".into());
    }
    if old.is_empty() {
        if path.exists() {
            return Err(
                "old_string cannot be empty on an existing file; use write to overwrite".into(),
            );
        }
        write_atomic(path, new, false)?;
        return Ok(format!("created {}", path.display()));
    }
    if path.is_dir() {
        return Err(format!("{} is a directory", path.display()));
    }
    let (bom, content) = read_text(path)?;
    let ending = detect_ending(&content);
    let old = to_ending(&old.replace("\r\n", "\n"), ending);
    let new = to_ending(&new.replace("\r\n", "\n"), ending);
    let (next, n) = replace(&content, &old, &new, replace_all)?;
    write_atomic(path, &next, bom)?;
    Ok(format!(
        "edited {} ({} replacement{})\n{}",
        path.display(),
        n,
        if n == 1 { "" } else { "s" },
        snippet(&next, &new)
    ))
}

/// OpenCode-style: exact, then indent-tolerant, then whitespace-normalized.
pub fn replace(
    content: &str,
    old: &str,
    new: &str,
    replace_all: bool,
) -> Result<(String, usize), String> {
    let mut not_found = true;
    for search in candidates(content, old) {
        if disproportionate(&search, old) {
            return Err(
                "matched span is much larger than old_string; re-read and pass the exact text"
                    .into(),
            );
        }
        let count = nonoverlap_count(content, &search);
        if count == 0 {
            continue;
        }
        not_found = false;
        if replace_all {
            return Ok((content.replace(&search, new), count));
        }
        if count > 1 {
            continue;
        }
        return Ok((content.replacen(&search, new, 1), 1));
    }
    if not_found {
        Err(not_found_msg(content, old))
    } else {
        Err("old_string matched more than once; add surrounding lines or set replace_all".into())
    }
}

fn candidates(content: &str, old: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() && !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    };
    push(old.to_string());
    for s in line_trimmed(content, old) {
        push(s);
    }
    for s in ws_normalized(content, old) {
        push(s);
    }
    if old.trim() != old && content.contains(old.trim()) {
        push(old.trim().to_string());
    }
    out
}

fn line_trimmed(content: &str, find: &str) -> Vec<String> {
    let orig: Vec<&str> = content.split('\n').collect();
    let mut search: Vec<&str> = find.split('\n').collect();
    if search.last() == Some(&"") {
        search.pop();
    }
    if search.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for i in 0..=orig.len().saturating_sub(search.len()) {
        if (0..search.len()).all(|j| orig[i + j].trim() == search[j].trim()) {
            hits.push(orig[i..i + search.len()].join("\n"));
        }
    }
    hits
}

fn ws_normalized(content: &str, find: &str) -> Vec<String> {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let want = norm(find);
    if want.is_empty() {
        return Vec::new();
    }
    content
        .lines()
        .filter(|line| norm(line) == want)
        .map(|s| s.to_string())
        .collect()
}

fn nonoverlap_count(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    hay.match_indices(needle).count()
}

fn disproportionate(search: &str, old: &str) -> bool {
    let old_lines = old.split('\n').count();
    let search_lines = search.split('\n').count();
    if search_lines >= old_lines.saturating_add(3).max(old_lines.saturating_mul(2)) {
        return true;
    }
    if old_lines == 1 {
        return false;
    }
    search.trim().len()
        > old
            .trim()
            .len()
            .saturating_add(500)
            .max(old.trim().len().saturating_mul(4))
}

fn not_found_msg(content: &str, old: &str) -> String {
    let needle = old.lines().next().unwrap_or(old).trim();
    let mut hints = Vec::new();
    if !needle.is_empty() {
        for (i, line) in content.lines().enumerate() {
            if line.contains(needle) || needle.contains(line.trim()) {
                hints.push(format!("{}→{line}", i + 1));
                if hints.len() == 5 {
                    break;
                }
            }
        }
    }
    if hints.is_empty() {
        "old_string not found (must match whitespace and indentation)".into()
    } else {
        format!("old_string not found. Nearby lines:\n{}", hints.join("\n"))
    }
}

fn snippet(text: &str, inserted: &str) -> String {
    let idx = text.find(inserted).unwrap_or(0);
    let start_line = text[..idx].matches('\n').count();
    let lines: Vec<&str> = text.split('\n').collect();
    let from = start_line.saturating_sub(2);
    let to = (start_line + inserted.split('\n').count() + 2).min(lines.len());
    lines[from..to]
        .iter()
        .enumerate()
        .map(|(i, l)| fsutil::format_line(from + i + 1, l))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_replace() {
        let (out, n) =
            replace("fn a() {}\nfn b() {}\n", "fn b() {}", "fn b() { 1 }", false).unwrap();
        assert_eq!(n, 1);
        assert!(out.contains("fn b() { 1 }"));
    }

    #[test]
    fn ambiguous_without_replace_all() {
        let err = replace("x = 1\nx = 1\n", "x = 1", "x = 2", false).unwrap_err();
        assert!(err.contains("more than once"));
    }

    #[test]
    fn replace_all_ok() {
        let (out, n) = replace("x = 1\nx = 1\n", "x = 1", "x = 2", true).unwrap();
        assert_eq!(n, 2);
        assert!(!out.contains("x = 1"));
    }

    #[test]
    fn indent_tolerant() {
        let file = "    fn go() {\n        x\n    }\n";
        let old = "fn go() {\n        x\n    }";
        let (out, n) = replace(file, old, "fn go() {\n        y\n    }", false).unwrap();
        assert_eq!(n, 1);
        assert!(out.contains("y"));
    }
}
