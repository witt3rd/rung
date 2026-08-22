//! Tool **scopes**, not jobs.
//!
//! Rung is an agent. Coding is one thing it might do. What changes per call
//! is which tools it may use. Named groups are composed from CLI `--tools`
//! and/or config `tools:`; `--toolset explore|implement|review` names a
//! preset. `--type` is an alias of `--toolset`.

use rung_std::tools::{
    ApplyPatch, EditFile, Glob, Grep, ListFiles, ReadFile, Shell, Skill, Todo, ToolCollection,
    ToolRoster, WebFetch, WriteFile,
};

/// Built-in `--toolset` names. Unknown names are refused, not guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Preset: read + web + skill. No nested `task`.
    Explore,
    /// Preset: write filesystem, shell, kernel extras, nested `task`.
    Implement,
    /// Preset: same tools as explore (read-only).
    Review,
}

impl Kind {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "explore" => Ok(Self::Explore),
            "implement" | "general-purpose" | "general" => Ok(Self::Implement),
            "review" => Ok(Self::Review),
            other => Err(format!(
                "unknown type '{other}' (explore, implement, review — these are tool presets, not jobs)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explore => "explore",
            Self::Implement => "implement",
            Self::Review => "review",
        }
    }

    pub fn allows_task(self) -> bool {
        matches!(self, Self::Implement)
    }

    pub fn max_iterations(self) -> u32 {
        match self {
            Self::Implement => 32,
            Self::Explore | Self::Review => 16,
        }
    }

    /// Groups this `--toolset` expands to.
    pub fn groups(self) -> &'static [&'static str] {
        match self {
            Self::Explore | Self::Review => &["read", "web", "skill"],
            Self::Implement => &["write", "shell", "todo", "web", "skill", "task"],
        }
    }

    pub fn roster(self) -> ToolRoster {
        Scope::from_groups(self.groups()).roster()
    }
}

/// Composed tool access for one run. `none` is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    groups: Vec<String>,
}

impl Scope {
    pub fn none() -> Self {
        Self { groups: Vec::new() }
    }

    pub fn from_kind(kind: Kind) -> Self {
        Self::from_groups(kind.groups())
    }

    pub fn from_groups(names: &[&str]) -> Self {
        Self {
            groups: names.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// `"none"` or comma-separated groups: `read`, `write`, `shell`, `web`,
    /// `skill`, `todo`, `python`, `task`.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if spec.is_empty() || spec.eq_ignore_ascii_case("none") {
            return Ok(Self::none());
        }
        let mut groups = Vec::new();
        for part in spec.split(',') {
            let g = part.trim().to_ascii_lowercase();
            if g.is_empty() {
                continue;
            }
            if !GROUPS.contains(&g.as_str()) {
                return Err(format!(
                    "unknown tool group '{g}' (none, read, write, shell, web, skill, todo, python, task)"
                ));
            }
            if !groups.contains(&g) {
                groups.push(g);
            }
        }
        Ok(Self { groups })
    }

    pub fn from_config_list(names: &[String]) -> Result<Self, String> {
        if names.is_empty() {
            return Ok(Self::none());
        }
        Self::parse(&names.join(","))
    }

    pub fn allows_task(&self) -> bool {
        self.groups.iter().any(|g| g == "task")
    }

    pub fn allows_python(&self) -> bool {
        self.groups.iter().any(|g| g == "python")
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn roster(&self) -> ToolRoster {
        let mut r = ToolRoster::new();
        for g in &self.groups {
            match g.as_str() {
                "read" => {
                    r.add(read_search());
                }
                "write" => {
                    r.add(filesystem_tools_with_shell_write());
                }
                "shell" => {
                    let mut c = ToolCollection::new("shell");
                    c.admit(Shell);
                    r.add(c);
                }
                "web" => {
                    let mut c = ToolCollection::new("web");
                    c.admit(WebFetch);
                    r.add(c);
                }
                "skill" => {
                    let mut c = ToolCollection::new("skill");
                    c.admit(Skill::in_cwd());
                    r.add(c);
                }
                "todo" => {
                    let mut c = ToolCollection::new("todo");
                    c.admit(Todo::new());
                    r.add(c);
                }
                "python" => {} // admitted in run.rs (needs a live sandbox)
                "task" => {}   // admitted in run.rs after Spawn exists
                _ => {}
            }
        }
        r
    }
}

const GROUPS: &[&str] = &[
    "read", "write", "shell", "web", "skill", "todo", "python", "task",
];

fn read_search() -> ToolCollection {
    let mut c = ToolCollection::new("read-search");
    c.admit(ReadFile);
    c.admit(ListFiles);
    c.admit(Glob);
    c.admit(Grep);
    c
}

fn filesystem_tools_with_shell_write() -> ToolCollection {
    let mut c = ToolCollection::new("write");
    c.admit(WriteFile);
    c.admit(EditFile);
    c.admit(ApplyPatch);
    c.admit(ReadFile);
    c.admit(ListFiles);
    c.admit(Glob);
    c.admit(Grep);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for n in ["explore", "implement", "review"] {
            assert_eq!(Kind::parse(n).unwrap().as_str(), n);
        }
        assert_eq!(Kind::parse("general-purpose").unwrap(), Kind::Implement);
        assert!(Kind::parse("plan").is_err());
    }

    #[test]
    fn none_has_no_tools() {
        assert!(Scope::none().roster().definitions().is_empty());
        assert!(Scope::parse("none").unwrap().is_empty());
    }

    #[test]
    fn parse_groups() {
        let s = Scope::parse("read,web").unwrap();
        let names: Vec<_> = s
            .roster()
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        for n in ["read_file", "grep", "webfetch"] {
            assert!(names.contains(&n.into()), "{names:?}");
        }
        assert!(!names.iter().any(|n| n == "edit"));
        assert!(Scope::parse("python").unwrap().allows_python());
        assert!(!Scope::parse("read").unwrap().allows_python());
    }

    #[test]
    fn implement_preset_has_write_and_shell() {
        let impl_names: Vec<_> = Kind::Implement
            .roster()
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        for n in ["edit", "write_file", "shell", "apply_patch", "todo"] {
            assert!(impl_names.contains(&n.into()), "{impl_names:?}");
        }
        assert!(!impl_names.contains(&"task".into()));

        let ex: Vec<_> = Kind::Explore
            .roster()
            .definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        for n in ["read_file", "grep", "glob", "webfetch", "skill"] {
            assert!(ex.contains(&n.into()), "{ex:?}");
        }
        for n in ["edit", "write_file", "shell", "apply_patch"] {
            assert!(!ex.contains(&n.into()), "{ex:?}");
        }
        assert!(!Kind::Explore.allows_task());
        assert!(Kind::Implement.allows_task());
    }
}
