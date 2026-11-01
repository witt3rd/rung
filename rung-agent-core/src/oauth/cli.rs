//! `rung-agent auth …`: make, inspect and remove logins.
//!
//! ```text
//! rung-agent auth login  <provider> [--profile NAME] [--enterprise DOMAIN]
//! rung-agent auth status [<provider>]
//! rung-agent auth models <provider> [--profile NAME] [--refresh]
//! rung-agent auth logout <provider> [--profile NAME]
//! ```
//!
//! `login` prints the code and the URL and waits, so it works over SSH with
//! no browser on the host. Nothing here prints a token.

use std::io::Write;
use std::sync::Arc;

use super::store::{DEFAULT_PROFILE, Store, valid_name};
use super::{Clock, DeviceAnnouncement, LoginError};

pub fn usage() -> &'static str {
    "usage:
  rung-agent auth login  <provider> [--profile NAME] [--enterprise DOMAIN]
  rung-agent auth status [<provider>]
  rung-agent auth models <provider> [--profile NAME] [--refresh]
  rung-agent auth logout <provider> [--profile NAME]
providers: github-copilot
"
}

struct Parsed {
    cmd: String,
    provider: Option<String>,
    profile: String,
    enterprise: Option<String>,
    refresh: bool,
}

fn parse(argv: &[String]) -> Result<Parsed, String> {
    let mut it = argv.iter();
    let cmd = it.next().ok_or("missing command")?.clone();
    let mut p = Parsed {
        cmd,
        provider: None,
        profile: DEFAULT_PROFILE.into(),
        enterprise: None,
        refresh: false,
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--profile" => p.profile = it.next().ok_or("--profile needs a name")?.clone(),
            "--enterprise" => {
                p.enterprise = Some(it.next().ok_or("--enterprise needs a domain")?.clone());
            }
            "--refresh" => p.refresh = true,
            s if s.starts_with('-') => return Err(format!("unknown flag {s}")),
            s if p.provider.is_none() => p.provider = Some(s.to_string()),
            s => return Err(format!("unexpected argument {s}")),
        }
    }
    valid_name(&p.profile)?;
    Ok(p)
}

/// Run `auth` (`argv` is what follows it). Returns the exit code.
pub fn run(
    argv: &[String],
    store: &Store,
    clock: Arc<dyn Clock>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let p = match parse(argv) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(err, "rung-agent auth: {e}\n{}", usage());
            return 2;
        }
    };
    match go(&p, store, clock, out, err) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "rung-agent auth: {e}");
            1
        }
    }
}

fn provider_for(p: &Parsed) -> Result<Box<dyn super::Provider>, String> {
    let name = p.provider.as_deref().ok_or("missing provider")?;
    let mut section = crate::config::load_oauth_section(name)?;
    if let Some(domain) = &p.enterprise {
        let mut map = match section.take() {
            Some(serde_yaml::Value::Mapping(m)) => m,
            _ => serde_yaml::Mapping::new(),
        };
        map.insert("enterprise_domain".into(), domain.clone().into());
        section = Some(serde_yaml::Value::Mapping(map));
    }
    super::provider(name, section.as_ref(), None)
}

fn go(
    p: &Parsed,
    store: &Store,
    clock: Arc<dyn Clock>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), String> {
    match p.cmd.as_str() {
        "login" => {
            let provider = provider_for(p)?;
            let mut announce = |a: &DeviceAnnouncement| {
                let _ = writeln!(
                    err,
                    "Open {} and enter the code {}\nWaiting for approval (the code works for {} minutes)…",
                    a.verification_uri,
                    a.user_code,
                    a.expires_in / 60
                );
                let _ = err.flush();
            };
            provider
                .login(store, &p.profile, clock.as_ref(), &mut announce)
                .map_err(|e: LoginError| e.to_string())?;
            let _ = writeln!(
                out,
                "logged in: {} profile '{}' ({})",
                provider.id(),
                p.profile,
                store.login_path(provider.id(), &p.profile).display()
            );
            Ok(())
        }
        "status" => {
            let logins = store.list()?;
            let mut any = false;
            for (prov, profile) in logins {
                if p.provider.as_deref().is_some_and(|w| w != prov) {
                    continue;
                }
                any = true;
                let login = store.load(&prov, &profile)?;
                let now = clock.now_secs();
                let token = match login.and_then(|l| l.access) {
                    Some(m) if m.expires_at > now => {
                        format!("token good for {}s", m.expires_at - now)
                    }
                    Some(_) => "token expired (renewed on next use)".into(),
                    None => "no token minted yet".into(),
                };
                let _ = writeln!(out, "{prov} profile '{profile}': logged in; {token}");
            }
            if !any {
                let _ = writeln!(out, "no logins");
            }
            Ok(())
        }
        "models" => {
            let provider = provider_for(p)?;
            let models = provider.models(store, &p.profile, clock, p.refresh)?;
            for m in models {
                let _ = writeln!(
                    out,
                    "{}{}",
                    m.id,
                    if m.chat_completions {
                        ""
                    } else {
                        "  (not chat-completions: unsupported)"
                    }
                );
            }
            Ok(())
        }
        "logout" => {
            let provider = provider_for(p)?;
            if store.remove(provider.id(), &p.profile)? {
                let _ = writeln!(out, "logged out: {} profile '{}'", provider.id(), p.profile);
            } else {
                let _ = writeln!(out, "no login: {} profile '{}'", provider.id(), p.profile);
            }
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n{}", usage())),
    }
}
