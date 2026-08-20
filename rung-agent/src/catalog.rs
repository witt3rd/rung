//! Named child profiles. Kernel `subagent_type` is a string; this crate
//! interprets it.

use rung_std::tools::{
    Glob, Grep, ListFiles, ReadFile, Skill, ToolCollection, ToolRoster, WebFetch,
    filesystem_tools_with_shell, kernel_tools,
};

/// Built-in catalog. Unknown names are refused, not guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Read and search only. No nested `task`.
    Explore,
    /// Full tools plus nested `task`.
    Implement,
    /// Read and search; report, do not edit. No nested `task`.
    Review,
}

impl Kind {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "explore" => Ok(Self::Explore),
            "implement" | "general-purpose" | "general" => Ok(Self::Implement),
            "review" => Ok(Self::Review),
            other => Err(format!(
                "unknown type '{other}' (explore, implement, review)"
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

    pub fn system(self) -> &'static str {
        match self {
            Self::Explore => {
                "You are a read-only explorer. Search and read. Do not modify files. \
                 Return a concise report of what you found."
            }
            Self::Implement => {
                "You are a coding agent. Prefer unique edit over rewrite. \
                 Use task for a bounded subproblem (explore, implement, or review). \
                 Nested tasks cannot spawn further tasks. Reply with what you did."
            }
            Self::Review => {
                "You are a reviewer. Read and search. Do not modify files. \
                 Report findings: bugs, risks, and what looks sound."
            }
        }
    }

    /// Roster for this profile. Caller admits `task` after isolation `chdir`.
    pub fn roster(self) -> ToolRoster {
        let mut r = ToolRoster::new();
        match self {
            Self::Implement => {
                r.add(filesystem_tools_with_shell());
                r.add(kernel_tools());
            }
            Self::Explore | Self::Review => {
                r.add(read_search());
                let mut extra = ToolCollection::new("read-extra");
                extra.admit(WebFetch);
                extra.admit(Skill::in_cwd());
                r.add(extra);
            }
        }
        r
    }
}

fn read_search() -> ToolCollection {
    let mut c = ToolCollection::new("read-search");
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
    fn implement_has_write_and_shell_explore_does_not() {
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
