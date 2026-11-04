//! The `spawn` verb (`fleet-verbs`): bring a new member into being with its
//! brief, as an ordinary host.
//!
//! `spawn` prepares the member's directories and its host configuration under a
//! root, and delivers the task (its brief) to the member's inbox before it
//! starts, so the host's first boundary reads it. A task cannot be spawned
//! without a brief: the argument is a [`Spawn`], which holds a validated one
//! (`a-brief-is-a-typed-contract`). [`launch`] starts the host process; the
//! host registers itself in the instance index, so no entry is written here.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use rung_host::registry;

use crate::delivery::{self, CorrelationId, Delivery, DeliveryError, Kind, Sent, Target};
use crate::lifecycle::Spawn;
use crate::steer::brief_text;

/// Why a spawn was refused.
#[derive(Debug)]
pub enum SpawnError {
    /// The member name is not its own registry id (letters, digits, `-`).
    BadName(String),
    /// The engine word is empty or not a plain word.
    BadEngine(String),
    /// A directory for this member already exists: spawn never overwrites.
    Exists(PathBuf),
    /// A file or directory could not be written.
    Io(std::io::Error),
    /// The task could not be put in the inbox.
    Delivery(DeliveryError),
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnError::BadName(n) => write!(f, "member name {n:?} is not a registry id"),
            SpawnError::BadEngine(e) => write!(f, "engine {e:?} is not a plain word"),
            SpawnError::Exists(p) => {
                write!(f, "{} already exists; spawn never overwrites", p.display())
            }
            SpawnError::Io(e) => write!(f, "{e}"),
            SpawnError::Delivery(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SpawnError {}

/// A member brought into being, not yet started.
#[must_use = "a spawned member holds a promise: launch it, and confirm the task against its record"]
#[derive(Debug)]
pub struct Spawned {
    /// The host configuration file to start the member with.
    pub config: PathBuf,
    pub inbox: PathBuf,
    pub state: PathBuf,
    pub workspace: PathBuf,
    /// The task, delivered to the inbox; confirm it once the host runs.
    pub task: Sent,
}

/// Prepare member `name` under `root/name` and deliver `spawn`'s brief to its
/// inbox as `task`, from `from`. `engine` is the host's `engine.kind` word.
pub fn spawn(
    root: &Path,
    name: &str,
    from: &str,
    task: &str,
    what: &Spawn,
    engine: &str,
) -> Result<Spawned, SpawnError> {
    if registry::slug(name).as_deref() != Some(name) {
        return Err(SpawnError::BadName(name.to_string()));
    }
    if engine.is_empty()
        || !engine
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(SpawnError::BadEngine(engine.to_string()));
    }
    let dir = std::path::absolute(root.join(name)).map_err(SpawnError::Io)?;
    if dir.exists() {
        return Err(SpawnError::Exists(dir));
    }
    let (inbox, state, workspace) = (dir.join("inbox"), dir.join("state"), dir.join("workspace"));
    for d in [&inbox, &workspace] {
        std::fs::create_dir_all(d).map_err(SpawnError::Io)?;
    }
    let config = dir.join("rung-host.yaml");
    std::fs::write(
        &config,
        format!(
            "name: {name}\nstate: {state}\nworkspace: {workspace}\ninbox: {inbox}\n\
             stop_file: {stop}\nengine:\n  kind: {engine}\nmemory: false\n",
            state = state.display(),
            workspace = workspace.display(),
            inbox = inbox.display(),
            stop = dir.join("STOP").display(),
        ),
    )
    .map_err(SpawnError::Io)?;
    let correlation = CorrelationId::new(format!("spawn-{name}")).map_err(SpawnError::Delivery)?;
    let sent = delivery::send(
        &Target {
            inbox: inbox.clone(),
            state: state.clone(),
        },
        &Delivery {
            correlation,
            kind: Kind::Assign,
            from: from.to_string(),
            text: brief_text(task, what.brief()),
        },
    )
    .map_err(SpawnError::Delivery)?;
    Ok(Spawned {
        config,
        inbox,
        state,
        workspace,
        task: sent,
    })
}

/// Start the host for a spawned member: `host run --config CONFIG`, with no
/// terminal. The caller owns the child; the host registers itself.
pub fn launch(host_binary: &Path, config: &Path) -> std::io::Result<Child> {
    Command::new(host_binary)
        .arg("run")
        .arg("--config")
        .arg(config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}
