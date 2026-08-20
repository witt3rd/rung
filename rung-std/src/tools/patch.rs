//! `apply_patch` — Codex / OpenCode patch language (Add / Update / Delete).

use super::Tool;
use super::fsutil::{self, read_text, write_atomic};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ApplyPatch;

impl Tool for ApplyPatch {
    fn name(&self) -> &'static str {
        "apply_patch"
    }
    fn description(&self) -> &'static str {
        "Apply a multi-file patch. Use *** Begin Patch / *** End Patch with \
         *** Add File: path, *** Delete File: path, or *** Update File: path \
         and @@ hunks. Prefer this when several files or hunks change together; \
         use edit for a single unique replace."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "patch": {"type": "string", "description": "Full patch text"}
            },
            "required": ["patch"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let patch = fsutil::req_str(input, "patch")?;
        apply_patch(patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hunk {
    Add { path: PathBuf, contents: String },
    Delete { path: PathBuf },
    Update { path: PathBuf, chunks: Vec<Chunk> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub context: Option<String>,
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,
}

pub fn parse_patch(text: &str) -> Result<Vec<Hunk>, String> {
    let lines: Vec<&str> = text.trim().lines().collect();
    if lines.first().map(|s| s.trim()) != Some("*** Begin Patch") {
        return Err("patch must start with *** Begin Patch".into());
    }
    if lines.last().map(|s| s.trim()) != Some("*** End Patch") {
        return Err("patch must end with *** End Patch".into());
    }
    let mut hunks = Vec::new();
    let mut i = 1;
    while i + 1 < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("*** Add File: ") {
            let path = PathBuf::from(line.trim_start_matches("*** Add File: ").trim());
            i += 1;
            let mut body = String::new();
            while i + 1 < lines.len() && !lines[i].trim_start().starts_with("*** ") {
                let l = lines[i];
                let rest = l.strip_prefix('+').unwrap_or(l);
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(rest);
                i += 1;
            }
            if !body.ends_with('\n') {
                body.push('\n');
            }
            hunks.push(Hunk::Add {
                path,
                contents: body,
            });
        } else if line.starts_with("*** Delete File: ") {
            let path = PathBuf::from(line.trim_start_matches("*** Delete File: ").trim());
            hunks.push(Hunk::Delete { path });
            i += 1;
        } else if line.starts_with("*** Update File: ") {
            let path = PathBuf::from(line.trim_start_matches("*** Update File: ").trim());
            i += 1;
            let mut chunks = Vec::new();
            while i + 1 < lines.len() && !lines[i].trim_start().starts_with("*** ") {
                let mut context = None;
                if lines[i].trim() == "@@" || lines[i].starts_with("@@ ") {
                    context = Some(lines[i].trim_start_matches("@@").trim().to_string())
                        .filter(|s| !s.is_empty());
                    i += 1;
                }
                let mut old_lines = Vec::new();
                let mut new_lines = Vec::new();
                while i + 1 < lines.len() {
                    let t = lines[i];
                    if t.trim() == "@@"
                        || t.starts_with("@@ ")
                        || t.trim_start().starts_with("*** ")
                    {
                        break;
                    }
                    if let Some(r) = t.strip_prefix('-') {
                        old_lines.push(r.to_string());
                    } else if let Some(r) = t.strip_prefix('+') {
                        new_lines.push(r.to_string());
                    } else if let Some(r) = t.strip_prefix(' ') {
                        old_lines.push(r.to_string());
                        new_lines.push(r.to_string());
                    } else if t.is_empty() {
                        old_lines.push(String::new());
                        new_lines.push(String::new());
                    } else {
                        return Err(format!("bad patch line: {t}"));
                    }
                    i += 1;
                }
                chunks.push(Chunk {
                    context,
                    old_lines,
                    new_lines,
                });
            }
            hunks.push(Hunk::Update { path, chunks });
        } else {
            return Err(format!("unexpected patch line: {line}"));
        }
    }
    if hunks.is_empty() {
        return Err("empty patch".into());
    }
    Ok(hunks)
}

pub fn apply_patch(text: &str) -> Result<String, String> {
    let hunks = parse_patch(text)?;
    let mut notes = Vec::new();
    for h in hunks {
        match h {
            Hunk::Add { path, contents } => {
                let p = resolve(&path);
                if p.exists() {
                    return Err(format!("add: {} already exists", p.display()));
                }
                write_atomic(&p, &contents, false)?;
                notes.push(format!("added {}", p.display()));
            }
            Hunk::Delete { path } => {
                let p = resolve(&path);
                std::fs::remove_file(&p).map_err(|e| format!("delete {}: {e}", p.display()))?;
                notes.push(format!("deleted {}", p.display()));
            }
            Hunk::Update { path, chunks } => {
                let p = resolve(&path);
                let (bom, content) = read_text(&p)?;
                let next =
                    apply_chunks(&content, &chunks).map_err(|e| format!("{}: {e}", p.display()))?;
                write_atomic(&p, &next, bom)?;
                notes.push(format!("updated {}", p.display()));
            }
        }
    }
    Ok(notes.join("\n"))
}

fn resolve(path: &Path) -> PathBuf {
    fsutil::resolve(&path.to_string_lossy())
}

pub fn apply_chunks(content: &str, chunks: &[Chunk]) -> Result<String, String> {
    let mut lines: Vec<String> = content.split('\n').map(String::from).collect();
    if lines.last().is_some_and(|s| s.is_empty()) {
        lines.pop();
    }
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut from = 0;
    for chunk in chunks {
        if let Some(ctx) = &chunk.context
            && let Some(idx) = lines.iter().skip(from).position(|l| l.trim() == ctx.trim())
        {
            from += idx + 1;
        }
        if chunk.old_lines.is_empty() {
            replacements.push((lines.len(), 0, chunk.new_lines.clone()));
            continue;
        }
        let start = seek(&lines, &chunk.old_lines, from)
            .ok_or_else(|| format!("hunk not found:\n{}", chunk.old_lines.join("\n")))?;
        replacements.push((start, chunk.old_lines.len(), chunk.new_lines.clone()));
        from = start + chunk.old_lines.len();
    }
    replacements.sort_by_key(|(s, _, _)| *s);
    for (start, old_len, new) in replacements.into_iter().rev() {
        for _ in 0..old_len {
            if start < lines.len() {
                lines.remove(start);
            }
        }
        for (off, n) in new.into_iter().enumerate() {
            lines.insert(start + off, n);
        }
    }
    if !lines.last().is_some_and(|s| s.is_empty()) {
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

fn seek(hay: &[String], needle: &[String], from: usize) -> Option<usize> {
    if needle.is_empty() || from > hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_add_update_delete() {
        let p = "*** Begin Patch\n*** Add File: a.txt\n+hi\n*** Update File: b.txt\n@@\n-old\n+new\n*** Delete File: c.txt\n*** End Patch\n";
        let h = parse_patch(p).unwrap();
        assert_eq!(h.len(), 3);
        match &h[0] {
            Hunk::Add { contents, .. } => assert_eq!(contents, "hi\n"),
            _ => panic!(),
        }
    }

    #[test]
    fn chunk_replaces_unique_block() {
        let out = apply_chunks(
            "fn a() {}\nfn b() {}\n",
            &[Chunk {
                context: None,
                old_lines: vec!["fn b() {}".into()],
                new_lines: vec!["fn b() { 1 }".into()],
            }],
        )
        .unwrap();
        assert_eq!(out, "fn a() {}\nfn b() { 1 }\n");
    }

    #[test]
    fn execute_add_and_update() {
        let dir = std::env::temp_dir().join(format!(
            "rung-patch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        std::fs::write("b.txt", "old\n").unwrap();
        let patch = "*** Begin Patch\n*** Add File: a.txt\n+hello\n*** Update File: b.txt\n@@\n-old\n+new\n*** End Patch\n";
        ApplyPatch
            .execute(&serde_json::json!({"patch": patch}))
            .unwrap();
        assert_eq!(std::fs::read_to_string("a.txt").unwrap(), "hello\n");
        assert_eq!(std::fs::read_to_string("b.txt").unwrap(), "new\n");
        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
