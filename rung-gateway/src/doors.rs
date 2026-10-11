//! The door table: every `/v1` door the hosts carry, and whether it only
//! reads. (The gateway streams every answer through; a door's being a stream
//! is not a gateway concern.) Later slices add their doors here as they land; a request for a
//! path the table does not name is classed by its method.

/// What a door does to the instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy)]
pub struct Door {
    pub method: &'static str,
    /// Segments; `{}` matches any one segment.
    pub path: &'static str,
    pub access: Access,
}

const fn door(method: &'static str, path: &'static str, access: Access) -> Door {
    Door {
        method,
        path,
        access,
    }
}

/// The doors named by the design note. Writes are the owner's; validating a
/// change is the owner's too, since it only serves the one who may apply it.
pub const DOORS: &[Door] = &[
    door("GET", "/v1/summary", Access::Read),
    door("GET", "/v1/status", Access::Read),
    door("GET", "/v1/report", Access::Read),
    door("GET", "/v1/record", Access::Read),
    door("GET", "/v1/turns", Access::Read),
    door("GET", "/v1/turns/{}", Access::Read),
    door("GET", "/v1/decisions", Access::Read),
    door("GET", "/v1/pack", Access::Read),
    door("GET", "/v1/spend", Access::Read),
    door("GET", "/v1/events", Access::Read),
    door("GET", "/v1/queue", Access::Read),
    door("POST", "/v1/queue", Access::Write),
    door("DELETE", "/v1/queue/{}", Access::Write),
    door("POST", "/v1/queue/{}/move", Access::Write),
    door("GET", "/v1/config", Access::Read),
    door("POST", "/v1/config/validate", Access::Write),
    door("PUT", "/v1/config", Access::Write),
    door("GET", "/v1/console", Access::Read),
    door("GET", "/v1/console/stream", Access::Read),
    door("POST", "/v1/stop", Access::Write),
    door("POST", "/v1/release", Access::Write),
];

fn matches(pattern: &str, path: &str) -> bool {
    let mut p = pattern.split('/');
    let mut q = path.split('/');
    loop {
        match (p.next(), q.next()) {
            (None, None) => return true,
            (Some("{}"), Some(seg)) if !seg.is_empty() => {}
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
    }
}

/// The door a request is for, if the table names it.
pub fn find(method: &str, path: &str) -> Option<&'static Door> {
    DOORS
        .iter()
        .find(|d| d.method == method && matches(d.path, path))
}

/// Classify a request to `/v1/...` on an instance: the table's word if it
/// names the door, else by method (a read is a GET or HEAD).
pub fn classify(method: &str, path: &str) -> Access {
    if let Some(d) = find(method, path) {
        return d.access;
    }
    match method {
        "GET" | "HEAD" => Access::Read,
        _ => Access::Write,
    }
}
