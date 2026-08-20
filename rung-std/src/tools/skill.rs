//! `skill` — load a SKILL.md by name from configured roots.

use super::Tool;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Skill {
    pub roots: Vec<PathBuf>,
}

impl Skill {
    pub fn in_cwd() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            roots: vec![
                cwd.join("skills"),
                cwd.join(".agents").join("skills"),
                cwd.join(".grok").join("skills"),
            ],
        }
    }

    pub fn discover(&self) -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        for root in &self.roots {
            if !root.is_dir() {
                continue;
            }
            let Ok(rd) = fs::read_dir(root) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let skill = p.join("SKILL.md");
                    if skill.is_file() {
                        let name = e.file_name().to_string_lossy().into_owned();
                        out.push((name, skill));
                    }
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

impl Tool for Skill {
    fn name(&self) -> &'static str {
        "skill"
    }
    fn description(&self) -> &'static str {
        "Load a skill's SKILL.md by directory name. Call with no name to list available skills."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Skill directory name; omit to list"}
            }
        })
    }
    fn execute(&self, input: &Value) -> Result<String, String> {
        let found = self.discover();
        let name = input["name"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match name {
            None => {
                if found.is_empty() {
                    return Ok("(no skills found)".into());
                }
                Ok(found
                    .iter()
                    .map(|(n, p)| format!("{n}\t{}", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            Some(want) => {
                let Some((_, path)) = found.iter().find(|(n, _)| n == want) else {
                    let names: Vec<_> = found.iter().map(|(n, _)| n.as_str()).collect();
                    return Err(format!("unknown skill {want}; have: {}", names.join(", ")));
                };
                load(path)
            }
        }
    }
}

fn load(path: &Path) -> Result<String, String> {
    let body = fs::read_to_string(path).map_err(|e| format!("skill: {e}"))?;
    let dir = path.parent().unwrap_or(path);
    Ok(format!(
        "<skill path=\"{}\">\n{}\n\nBase directory: {}\n</skill>",
        path.display(),
        body.trim(),
        dir.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_loads() {
        let root = std::env::temp_dir().join(format!(
            "rung-skill-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("demo")).unwrap();
        fs::write(
            root.join("demo").join("SKILL.md"),
            "# Demo\nDo the thing.\n",
        )
        .unwrap();
        let s = Skill {
            roots: vec![root.clone()],
        };
        let list = s.execute(&serde_json::json!({})).unwrap();
        assert!(list.contains("demo"), "{list}");
        let loaded = s.execute(&serde_json::json!({"name": "demo"})).unwrap();
        assert!(loaded.contains("Do the thing"), "{loaded}");
        assert!(loaded.contains("Base directory"), "{loaded}");
        let miss = s.execute(&serde_json::json!({"name": "nope"})).unwrap_err();
        assert!(miss.contains("unknown skill"), "{miss}");
        let _ = fs::remove_dir_all(&root);
    }
}
