//! The gateway's own config file and its resolved form.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::registry::Instance;

/// The file as written: names of variables, never the secrets themselves.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Address to listen on; loopback by default (the tailnet's own serve
    /// command puts it on https).
    #[serde(default = "default_listen")]
    pub listen: String,
    /// A built app to serve; a relative path is relative to the config file.
    #[serde(default)]
    pub app_dir: Option<PathBuf>,
    /// Variables each holding one read-only token.
    #[serde(default)]
    pub read_only_token_envs: Vec<String>,
    /// The instances behind the gateway.
    #[serde(default)]
    pub instances: Vec<InstanceConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceConfig {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// The instance's own listener, `http://host:port`.
    pub url: String,
    /// The variable holding the owner key for that instance.
    pub key_env: String,
}

fn default_listen() -> String {
    "127.0.0.1:8787".into()
}

/// The config with every variable resolved. Holds secrets: never printed.
#[derive(Clone)]
pub struct Settings {
    pub listen: SocketAddr,
    pub app_dir: Option<PathBuf>,
    /// Tokens whose bearer may only read.
    pub read_only_tokens: Vec<String>,
    pub instances: Vec<Instance>,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("listen", &self.listen)
            .field("app_dir", &self.app_dir)
            .field("read_only_tokens", &self.read_only_tokens.len())
            .field("instances", &self.instances)
            .finish()
    }
}

impl Config {
    /// Read and parse the file; a relative `app_dir` is anchored beside it.
    pub fn load(path: &Path) -> Result<Config, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut cfg: Config =
            serde_yaml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if let Some(dir) = &cfg.app_dir
            && dir.is_relative()
            && let Some(base) = path.parent()
        {
            cfg.app_dir = Some(base.join(dir));
        }
        Ok(cfg)
    }

    /// Resolve every named variable through `env`. A missing or empty one is
    /// an error naming the variable, never its value.
    pub fn resolve(&self, env: impl Fn(&str) -> Option<String>) -> Result<Settings, String> {
        let listen = self
            .listen
            .parse()
            .map_err(|e| format!("listen {:?}: {e}", self.listen))?;
        let var = |name: &str| -> Result<String, String> {
            match env(name) {
                Some(v) if !v.is_empty() => Ok(v),
                _ => Err(format!("variable {name} is not set or is empty")),
            }
        };
        let read_only_tokens = self
            .read_only_token_envs
            .iter()
            .map(|n| var(n))
            .collect::<Result<Vec<_>, _>>()?;
        let mut instances: Vec<Instance> = Vec::new();
        for c in &self.instances {
            if !valid_id(&c.id) {
                return Err(format!(
                    "instance id {:?}: use letters, digits, '-' and '_' (it is a path segment)",
                    c.id
                ));
            }
            if instances.iter().any(|i| i.id == c.id) {
                return Err(format!("duplicate instance id {:?}", c.id));
            }
            if !c.url.starts_with("http://") {
                return Err(format!(
                    "instance {}: url must start with http:// (a host's own listener)",
                    c.id
                ));
            }
            instances.push(Instance {
                id: c.id.clone(),
                name: c.name.clone().unwrap_or_else(|| c.id.clone()),
                url: c.url.trim_end_matches('/').to_string(),
                key: var(&c.key_env)?,
            });
        }
        Ok(Settings {
            listen,
            app_dir: self.app_dir.clone(),
            read_only_tokens,
            instances,
        })
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
