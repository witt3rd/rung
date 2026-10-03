//! Who owns memory for a process.
//!
//! The setting is chosen once per run from rung's own surfaces, in this
//! order: the `--memory` flag, then `RUNG_MEMORY`, then `memory.provider` in
//! `config.yaml`, then the default, `off`. rung never infers it from who the
//! caller is, from an MCP server's name, or from a request's `_meta`.

/// Who owns memory for this process.
///
/// - `Off`: no memory. The default; rung behaves as it did before memory.
/// - `External`: the caller owns memory. It composes recall into the prompt
///   itself, judges retention itself, and gives the agent whatever memory
///   tools it wants as MCP tools. rung opens no store, recalls and retains
///   nothing, and admits no memory tools of its own, so it is never a second
///   source of truth.
/// - `Provider { name, arg }`: rung keeps memory through the registered
///   provider called `name` ([`crate::Registry`]), written `name` or
///   `name:arg` (`baseline`, `mcp:http://127.0.0.1:9000/mcp`). The name is
///   case-insensitive; `arg` is kept as written and is the provider's to read.
///
/// Every `match` on this enum has to say what it does in each case.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MemoryAuthority {
    #[default]
    Off,
    External,
    Provider {
        name: String,
        arg: Option<String>,
    },
}

/// Refuse a name that is reserved or not `[a-z0-9_-]+`.
pub(crate) fn check_provider_name(name: &str) -> Result<(), String> {
    if name == "off" || name == "external" {
        return Err(format!("'{name}' is a memory setting, not a provider name"));
    }
    let ok = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "bad memory provider name '{name}' (off | external | a provider name, [a-z0-9_-])"
        ))
    }
}

impl MemoryAuthority {
    /// Parse one setting: `off`, `external`, `name` or `name:arg`. Space
    /// around it is ignored, and case in all but `arg`. Whether the provider
    /// exists is the registry's question.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let (name, arg) = match s.split_once(':') {
            Some((n, a)) => (n.trim().to_ascii_lowercase(), Some(a.trim().to_string())),
            None => (s.to_ascii_lowercase(), None),
        };
        match (name.as_str(), arg) {
            ("off", None) => Ok(Self::Off),
            ("external", None) => Ok(Self::External),
            (_, Some(a)) if a.is_empty() => Err(format!("'{s}': nothing after ':'")),
            (n, arg) => check_provider_name(n).map(|()| Self::Provider {
                name: n.to_string(),
                arg,
            }),
        }
    }

    /// The setting's name: `off`, `external`, or the provider's name.
    pub fn name(&self) -> &str {
        match self {
            Self::Off => "off",
            Self::External => "external",
            Self::Provider { name, .. } => name,
        }
    }

    /// Shorthand for a provider without an argument.
    pub fn provider(name: &str) -> Self {
        Self::Provider {
            name: name.to_string(),
            arg: None,
        }
    }

    /// Resolve the setting from its three surfaces: an explicit flag wins,
    /// then the env value, then the file value. A blank env or file value
    /// counts as unset. Nothing set is `Off`. A malformed value is an error,
    /// never a fallback, so a typo cannot silently change who owns memory.
    pub fn resolve(
        flag: Option<Self>,
        env: Option<&str>,
        file: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(a) = flag {
            return Ok(a);
        }
        if let Some(v) = set(env) {
            return Self::parse(v).map_err(|e| format!("RUNG_MEMORY: {e}"));
        }
        if let Some(v) = set(file) {
            return Self::parse(v).map_err(|e| format!("memory.provider: {e}"));
        }
        Ok(Self::Off)
    }
}

fn set(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

impl std::fmt::Display for MemoryAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider {
                name,
                arg: Some(arg),
            } => write!(f, "{name}:{arg}"),
            other => f.write_str(other.name()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_off() {
        assert_eq!(MemoryAuthority::default(), MemoryAuthority::Off);
        assert_eq!(
            MemoryAuthority::resolve(None, None, None).unwrap(),
            MemoryAuthority::Off
        );
    }

    #[test]
    fn parse_reads_settings_and_provider_names() {
        assert_eq!(
            MemoryAuthority::parse(" External ").unwrap(),
            MemoryAuthority::External
        );
        assert_eq!(MemoryAuthority::parse("off").unwrap(), MemoryAuthority::Off);
        assert_eq!(
            MemoryAuthority::parse("Baseline").unwrap(),
            MemoryAuthority::provider("baseline")
        );
        let mcp = MemoryAuthority::parse("MCP:http://Host:9/mcp").unwrap();
        assert_eq!(
            mcp,
            MemoryAuthority::Provider {
                name: "mcp".into(),
                arg: Some("http://Host:9/mcp".into())
            }
        );
        assert_eq!(mcp.to_string(), "mcp:http://Host:9/mcp");
        assert!(MemoryAuthority::parse("no such/thing").is_err());
        assert!(MemoryAuthority::parse("mcp:").is_err());
        assert!(MemoryAuthority::parse("off:x").is_err());
    }

    #[test]
    fn flag_beats_env_beats_file() {
        use MemoryAuthority::*;
        let r = MemoryAuthority::resolve;
        assert_eq!(r(None, None, Some("external")).unwrap(), External);
        assert_eq!(r(None, Some("off"), Some("external")).unwrap(), Off);
        assert_eq!(r(None, Some("external"), Some("off")).unwrap(), External);
        assert_eq!(
            r(Some(Off), Some("external"), Some("external")).unwrap(),
            Off
        );
        assert_eq!(
            r(None, Some("baseline"), None).unwrap(),
            MemoryAuthority::provider("baseline")
        );
        assert_eq!(r(Some(External), None, None).unwrap(), External);
        // Blank counts as unset.
        assert_eq!(r(None, Some("  "), Some("external")).unwrap(), External);
        assert_eq!(r(None, Some(""), Some("")).unwrap(), Off);
    }

    #[test]
    fn an_unknown_value_is_an_error_naming_its_surface() {
        let env = MemoryAuthority::resolve(None, Some("x y"), None).unwrap_err();
        assert!(env.starts_with("RUNG_MEMORY:"), "{env}");
        let file = MemoryAuthority::resolve(None, None, Some("x y")).unwrap_err();
        assert!(file.starts_with("memory.provider:"), "{file}");
        // A bad file value is not read when the env decides.
        assert!(MemoryAuthority::resolve(None, Some("off"), Some("x y")).is_ok());
    }
}
