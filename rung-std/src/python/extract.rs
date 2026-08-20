//! Recognize Python in a model draft. The guest never sees bash waffle.
//!
//! Native tool calling is one way the agent acts. The other is this: the
//! model writes code in the assistant text and the harness extracts it.
//! A fenced shell one-liner is rejected even when a later line looks like
//! Python. Bare prose is not code.

/// What a draft is, before any strike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Draft {
    /// Extracted Python the guest can run.
    Python(String),
    /// A shell dump (fenced bash, or a fence that is not Python).
    Rejected,
    /// Prose, empty, or anything that is not code.
    Text,
}

/// Pull Python out of a draft, or `None` for waffle / prose.
pub fn extract_python(draft: &str) -> Option<String> {
    match classify_draft(draft) {
        Draft::Python(code) => Some(code),
        Draft::Rejected | Draft::Text => None,
    }
}

/// Classify a draft so the agent can nudge, finish, or strike.
pub fn classify_draft(draft: &str) -> Draft {
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        return Draft::Text;
    }
    if let Some(code) = fenced(trimmed, &["python", "py"]) {
        return Draft::Python(code);
    }
    if fenced(trimmed, &["bash", "sh", "zsh", "shell"]).is_some() && !looks_like_python(trimmed) {
        return Draft::Rejected;
    }
    if let Some(code) = fenced(trimmed, &[""]) {
        if looks_like_python(&code) {
            return Draft::Python(code);
        }
        return Draft::Rejected;
    }
    if looks_like_python(trimmed) && !looks_like_prose(trimmed) {
        return Draft::Python(trimmed.to_string());
    }
    Draft::Text
}

fn fenced(text: &str, langs: &[&str]) -> Option<String> {
    let mut i = 0;
    while let Some(rel) = text[i..].find("```") {
        let start = i + rel + 3;
        let rest = &text[start..];
        let (lang, after_lang) = match rest.find('\n') {
            Some(n) => (rest[..n].trim(), &rest[n + 1..]),
            None => break,
        };
        let lang_ok = langs.iter().any(|want| {
            if want.is_empty() {
                lang.is_empty()
            } else {
                lang.eq_ignore_ascii_case(want)
            }
        });
        if let Some(end) = after_lang.find("```") {
            if lang_ok {
                return Some(after_lang[..end].trim().to_string());
            }
            i = start + (rest.len() - after_lang.len()) + end + 3;
            continue;
        }
        break;
    }
    None
}

fn looks_like_python(text: &str) -> bool {
    let body = text.trim();
    if body.is_empty() {
        return false;
    }
    let first = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let t = first.trim_start();
    t.starts_with("import ")
        || t.starts_with("from ")
        || t.starts_with("print(")
        || t.starts_with("def ")
        || t.starts_with("class ")
        || t.starts_with("async ")
        || t.starts_with("for ")
        || t.starts_with("if ")
        || t.starts_with("with ")
        || t.starts_with("try:")
        || t.contains("Path(")
}

fn looks_like_prose(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("here's")
        || lower.contains("here is")
        || lower.contains("### ")
        || lower.contains("one-liner")
        || lower.contains("what this does")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The waffle `anvil complete` returned for a symlink-count prompt.
    const WAFFLE: &str = r#"The interpretation of "have synlinks" usually means: **count all symbolic link files** found recursively inside `~/dotfiles/`.

Here's the standard one-liner to get that count:

```bash
find ~/dotfiles -type l | wc -l
```

### What this does
- `find ~/dotfiles` — searches recursively
"#;

    #[test]
    fn extract_skips_bash_waffle() {
        assert_eq!(classify_draft(WAFFLE), Draft::Rejected);
        assert!(extract_python(WAFFLE).is_none());
    }

    #[test]
    fn extract_takes_python_fence() {
        let draft = "sure\n```python\nprint(1)\n```\n";
        assert_eq!(extract_python(draft).as_deref(), Some("print(1)"));
    }

    #[test]
    fn unlabeled_fence_that_is_python() {
        let draft = "```\nprint(2)\n```";
        assert_eq!(extract_python(draft).as_deref(), Some("print(2)"));
    }

    #[test]
    fn unlabeled_fence_that_is_not_python_is_rejected() {
        assert_eq!(classify_draft("```\nls -la\n```"), Draft::Rejected);
    }

    #[test]
    fn bare_python_without_fence() {
        assert_eq!(
            extract_python("from pathlib import Path\nprint(1)").as_deref(),
            Some("from pathlib import Path\nprint(1)")
        );
    }

    #[test]
    fn prose_is_text_not_code() {
        assert_eq!(classify_draft("The answer is 4."), Draft::Text);
        assert!(extract_python("The answer is 4.").is_none());
    }
}
