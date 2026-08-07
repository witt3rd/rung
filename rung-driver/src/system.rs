//! System-wide rung configuration: `~/.rung/`.
//!
//! The endpoint catalog and the credentials are **system-wide**, shared by every
//! `population.yaml`, so a project never re-declares its providers. A
//! population may still declare an inline `providers:` override; the driver
//! prefers the roster's inline entry and falls back to this catalog.
//!
//! ```text
//! ~/.rung/providers.yaml     # the provider catalog + the DEFAULT provider
//! ~/.rung/auth.yaml          # provider name -> api key (or the environment)
//! ```
//!
//! Credentials are sourced from the **real environment first** (a provider's
//! `api_key_env`, then its name), and only then from `auth.yaml` — so the file
//! is a last resort, and nothing ever needs to be committed.

use rung_std::principals::Provider;
use serde::Deserialize;
use std::collections::BTreeMap;

/// The on-disk shape of `~/.rung/providers.yaml`.
#[derive(Deserialize)]
struct Catalog {
    /// The provider used when a principal's backing names no provider.
    #[serde(default)]
    default: Option<String>,
    #[serde(default)]
    providers: Vec<Provider>,
}

/// The resolved system-wide config: the provider catalog, the default provider,
/// and the credentials in `auth.yaml`.
pub struct SystemConfig {
    pub providers: Vec<Provider>,
    pub default_provider: Option<String>,
    pub auth: BTreeMap<String, String>,
}

fn rung_dir() -> std::path::PathBuf {
    std::env::var("RUNG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| std::path::Path::new(&h).join(".rung"))
                .unwrap_or_default()
        })
}

impl SystemConfig {
    /// Load the system-wide catalog and credentials from `~/.rung` (or
    /// `$RUNG_HOME` when set). Missing files degrade to empty — the roster's
    /// inline `providers:` can be a complete override.
    pub fn load() -> Self {
        Self::load_from(&rung_dir())
    }

    pub fn load_from(dir: &std::path::Path) -> Self {
        let catalog = std::fs::read_to_string(dir.join("providers.yaml"))
            .ok()
            .and_then(|t| serde_yaml::from_str::<Catalog>(&t).ok());
        let auth = std::fs::read_to_string(dir.join("auth.yaml"))
            .ok()
            .and_then(|t| serde_yaml::from_str::<BTreeMap<String, String>>(&t).ok())
            .unwrap_or_default();
        SystemConfig {
            providers: catalog
                .as_ref()
                .map(|c| c.providers.clone())
                .unwrap_or_default(),
            default_provider: catalog.and_then(|c| c.default),
            auth,
        }
    }

    /// For tests / portable callers: construct a catalog by hand instead of
    /// reading the filesystem.
    pub fn from_parts(
        providers: Vec<Provider>,
        default_provider: Option<String>,
        auth: BTreeMap<String, String>,
    ) -> Self {
        Self {
            providers,
            default_provider,
            auth,
        }
    }

    pub fn provider(&self, name: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.name == name)
    }

    pub fn default(&self) -> Option<&str> {
        self.default_provider.as_deref()
    }

    pub fn api_key(&self, provider: &str) -> Option<&str> {
        self.auth.get(provider).map(String::as_str)
    }
}
