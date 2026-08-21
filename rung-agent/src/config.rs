//! [`rung_std::llm::LlmConfig`] from XDG `config.yaml`, then the environment.
//!
//! ```text
//! $XDG_CONFIG_HOME/rung/config.yaml   # or ~/.config/rung/config.yaml
//! ```
//!
//! Env wins over the file. The file may name the credential's *environment
//! variable* (`api_key_env`); it does not hold the key. A missing key is
//! empty: LAN llama.cpp / vLLM do not authenticate. Cloud endpoints that
//! need a key will 401 at the wire.

use rung_std::llm::{CachePolicy, LlmConfig, Protocol};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const DEFAULT_BASE: &str = "https://api.x.ai/v1";
const DEFAULT_MODEL: &str = "grok-4";

#[derive(Debug, Clone, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    llm: Option<LlmFile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LlmFile {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    model: Option<String>,
    /// Name of the env var holding the key. Never the key itself.
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    idle_timeout_secs: Option<u64>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    reasoning_level: Option<String>,
    #[serde(default)]
    cache: Option<String>,
}

/// Directory for the product config: `$XDG_CONFIG_HOME/rung`.
pub fn config_dir() -> PathBuf {
    config_dir_from(
        std::env::var("XDG_CONFIG_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

pub fn config_dir_from(xdg: Option<&str>, home: Option<&str>) -> PathBuf {
    if let Some(xdg) = xdg.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(xdg).join("rung");
    }
    PathBuf::from(home.unwrap_or("."))
        .join(".config")
        .join("rung")
}

/// `RUNG_CONFIG` if set, otherwise `<config_dir>/config.yaml`.
pub fn config_path() -> PathBuf {
    match std::env::var("RUNG_CONFIG") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => config_dir().join("config.yaml"),
    }
}

/// Load LLM settings: env over XDG file over defaults.
pub fn load() -> Result<LlmConfig, String> {
    load_from_path(&config_path())
}

pub fn load_from_path(path: &Path) -> Result<LlmConfig, String> {
    let file = read_file(path)?;
    resolve(file.as_ref().and_then(|f| f.llm.as_ref()), |k| {
        std::env::var(k).ok()
    })
}

fn read_file(path: &Path) -> Result<Option<FileConfig>, String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("config {}: {e}", path.display())),
        Ok(body) => serde_yaml::from_str(&body)
            .map(Some)
            .map_err(|e| format!("config {}: {e}", path.display())),
    }
}

fn resolve(
    file: Option<&LlmFile>,
    getenv: impl Fn(&str) -> Option<String>,
) -> Result<LlmConfig, String> {
    let pick = |env_key: &str, file_val: Option<&str>| -> Option<String> {
        getenv(env_key)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                file_val
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            })
    };
    let api_key = getenv("RUNG_API_KEY")
        .or_else(|| getenv("XAI_API_KEY"))
        .or_else(|| {
            file.and_then(|f| f.api_key_env.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(&getenv)
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    let protocol = parse_protocol(
        getenv("RUNG_PROTOCOL")
            .or_else(|| file.and_then(|f| f.protocol.clone()))
            .as_deref(),
    )?;
    let cache = parse_cache(file.and_then(|f| f.cache.as_deref()))?;
    let timeout_secs = match getenv("RUNG_TIMEOUT_SECS") {
        Some(s) => parse_num("RUNG_TIMEOUT_SECS", &s)?,
        None => file.and_then(|f| f.timeout_secs).unwrap_or(120),
    };
    let max_tokens = match getenv("RUNG_MAX_TOKENS") {
        Some(s) => parse_num("RUNG_MAX_TOKENS", &s)?,
        None => file.and_then(|f| f.max_tokens).unwrap_or(8192),
    };
    let idle_timeout_secs = match getenv("RUNG_IDLE_TIMEOUT_SECS") {
        Some(s) => Some(parse_num("RUNG_IDLE_TIMEOUT_SECS", &s)?),
        None => file.and_then(|f| f.idle_timeout_secs),
    };
    let temperature = match getenv("RUNG_TEMPERATURE") {
        Some(s) => Some(parse_num("RUNG_TEMPERATURE", &s)?),
        None => file.and_then(|f| f.temperature),
    };

    Ok(LlmConfig {
        base_url: pick("RUNG_BASE_URL", file.and_then(|f| f.base_url.as_deref()))
            .unwrap_or_else(|| DEFAULT_BASE.into()),
        api_key,
        model: pick("RUNG_MODEL", file.and_then(|f| f.model.as_deref()))
            .unwrap_or_else(|| DEFAULT_MODEL.into()),
        timeout_secs,
        idle_timeout_secs,
        max_tokens,
        temperature,
        top_p: None,
        top_k: None,
        seed: None,
        stop: Vec::new(),
        reasoning_level: getenv("RUNG_REASONING_LEVEL")
            .or_else(|| file.and_then(|f| f.reasoning_level.clone())),
        structured_outputs: false,
        protocol,
        cache,
        stream_listener: None,
    })
}

fn parse_protocol(value: Option<&str>) -> Result<Protocol, String> {
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(Protocol::Auto),
        Some(s) => match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Protocol::Auto),
            "anthropic-messages" | "anthropic" => Ok(Protocol::AnthropicMessages),
            "openai-chat" | "openai" | "openai-compatible" => Ok(Protocol::OpenAiChat),
            other => Err(format!("protocol: unknown '{other}'")),
        },
    }
}

fn parse_cache(value: Option<&str>) -> Result<CachePolicy, String> {
    match value.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(CachePolicy::Auto),
        Some(s) => match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(CachePolicy::Auto),
            "none" => Ok(CachePolicy::None),
            other => Err(format!("cache: unknown '{other}'")),
        },
    }
}

fn parse_num<T: std::str::FromStr>(key: &str, s: &str) -> Result<T, String> {
    s.trim()
        .parse()
        .map_err(|_| format!("{key}: not a number ({s})"))
}

/// Config that never hits the network. Tests only.
pub fn dummy() -> LlmConfig {
    LlmConfig {
        base_url: "http://127.0.0.1:9/v1".into(),
        api_key: "test".into(),
        model: "dummy".into(),
        timeout_secs: 1,
        idle_timeout_secs: None,
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        seed: None,
        stop: Vec::new(),
        reasoning_level: None,
        structured_outputs: false,
        protocol: Protocol::OpenAiChat,
        cache: CachePolicy::None,
        stream_listener: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn getenv<'a>(map: &'a HashMap<&str, &str>) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| map.get(k).map(|s| (*s).to_string())
    }

    fn parse_llm(yaml: &str) -> LlmFile {
        serde_yaml::from_str::<FileConfig>(yaml)
            .unwrap()
            .llm
            .unwrap()
    }

    #[test]
    fn xdg_dir_prefers_xdg_config_home() {
        assert_eq!(
            config_dir_from(Some("/tmp/xdg"), Some("/home/me")),
            PathBuf::from("/tmp/xdg/rung")
        );
        assert_eq!(
            config_dir_from(None, Some("/home/me")),
            PathBuf::from("/home/me/.config/rung")
        );
    }

    #[test]
    fn file_supplies_llm_settings() {
        let file = parse_llm(
            r#"
llm:
  base_url: http://localhost:20128/v1
  model: grok-4
  api_key_env: OMNI_ROUTE_API_KEY
  timeout_secs: 30
  max_tokens: 4096
  protocol: anthropic-messages
  cache: none
"#,
        );
        let env = HashMap::from([("OMNI_ROUTE_API_KEY", "k")]);
        let c = resolve(Some(&file), getenv(&env)).unwrap();
        assert_eq!(c.base_url, "http://localhost:20128/v1");
        assert_eq!(c.model, "grok-4");
        assert_eq!(c.api_key, "k");
        assert_eq!(c.timeout_secs, 30);
        assert_eq!(c.max_tokens, 4096);
        assert_eq!(c.protocol, Protocol::AnthropicMessages);
        assert_eq!(c.cache, CachePolicy::None);
    }

    #[test]
    fn env_overrides_file() {
        let file = parse_llm("llm:\n  model: from-file\n  api_key_env: FILE_KEY\n");
        let env = HashMap::from([
            ("RUNG_MODEL", "from-env"),
            ("RUNG_API_KEY", "env-key"),
            ("FILE_KEY", "file-key"),
        ]);
        let c = resolve(Some(&file), getenv(&env)).unwrap();
        assert_eq!(c.model, "from-env");
        assert_eq!(c.api_key, "env-key");
    }

    #[test]
    fn missing_file_uses_defaults_when_key_present() {
        let env = HashMap::from([("XAI_API_KEY", "x")]);
        let c = resolve(None, getenv(&env)).unwrap();
        assert_eq!(c.base_url, DEFAULT_BASE);
        assert_eq!(c.model, DEFAULT_MODEL);
        assert_eq!(c.api_key, "x");
        assert_eq!(c.protocol, Protocol::Auto);
    }

    #[test]
    fn missing_key_is_empty() {
        let env: HashMap<&str, &str> = HashMap::new();
        let c = resolve(None, getenv(&env)).unwrap();
        assert!(c.api_key.is_empty());
        assert_eq!(c.base_url, DEFAULT_BASE);
        assert_eq!(c.model, DEFAULT_MODEL);
    }

    #[test]
    fn missing_path_is_ok() {
        let path = PathBuf::from("/no/such/rung-agent-config.yaml");
        assert!(read_file(&path).unwrap().is_none());
    }

    #[test]
    fn disk_yaml_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "rung-cfg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        std::fs::write(
            &path,
            "llm:\n  model: file-model\n  api_key_env: FILE_KEY\n",
        )
        .unwrap();
        let file = read_file(&path).unwrap().unwrap();
        let env = HashMap::from([("FILE_KEY", "from-file-env")]);
        let c = resolve(file.llm.as_ref(), getenv(&env)).unwrap();
        assert_eq!(c.model, "file-model");
        assert_eq!(c.api_key, "from-file-env");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn protocol_names() {
        assert_eq!(parse_protocol(None).unwrap(), Protocol::Auto);
        assert_eq!(
            parse_protocol(Some("anthropic-messages")).unwrap(),
            Protocol::AnthropicMessages
        );
        assert_eq!(
            parse_protocol(Some("openai")).unwrap(),
            Protocol::OpenAiChat
        );
        assert!(parse_protocol(Some("nope")).is_err());
    }
}
