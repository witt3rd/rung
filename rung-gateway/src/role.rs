//! Who a request is, and what that allows.

/// The owner is anyone on the tailnet with no token; a read-only bearer may
/// only read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    ReadOnly,
}

/// What a request's token says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Known(Role),
    /// A token was presented and matches no role.
    Unknown,
}

/// Resolve the role from the `Authorization: Bearer` value and the `token`
/// query value (an event stream cannot set headers).
pub fn resolve(bearer: Option<&str>, query_token: Option<&str>, read_only: &[String]) -> Who {
    let mut presented = [bearer, query_token].into_iter().flatten().peekable();
    if presented.peek().is_none() {
        return Who::Known(Role::Owner);
    }
    // Every token presented must be a read-only token: a bad one anywhere
    // refuses, so a stray credential never silently widens a request.
    if presented.all(|t| !t.is_empty() && read_only.iter().any(|r| same(r, t))) {
        Who::Known(Role::ReadOnly)
    } else {
        Who::Unknown
    }
}

/// Compare without stopping at the first differing byte.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}
