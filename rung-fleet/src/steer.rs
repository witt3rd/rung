//! The `steer` verb (`a-steer-is-an-inbox-item`).
//!
//! A steer is a durable item in the target member's inbox, read at the
//! target's next boundary. Nothing else is touched: no process is signalled and
//! no terminal is written to. The inbox is found where the host's own
//! configuration names it, through the member's entry in the instance index.

use std::fmt;
use std::path::Path;

use rung_host::{registry, startup};

use crate::delivery::{self, CorrelationId, Delivery, DeliveryError, Kind, Sent, Target};

/// Why a steer could not be sent.
#[derive(Debug)]
pub enum SteerError {
    /// The instance index could not be read.
    Index(String),
    /// No member has this id.
    NoSuchMember(String),
    /// The member's entry is unreadable, or names no configuration file.
    NoConfig(String),
    /// The configuration does not name an absolute inbox directory.
    NoInbox(String),
    /// The delivery itself failed.
    Delivery(DeliveryError),
}

impl fmt::Display for SteerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SteerError::Index(e) => write!(f, "instance index: {e}"),
            SteerError::NoSuchMember(id) => write!(f, "no member `{id}` in the instance index"),
            SteerError::NoConfig(why) => write!(f, "{why}"),
            SteerError::NoInbox(why) => write!(f, "{why}"),
            SteerError::Delivery(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SteerError {}

/// Where a member's inbox is: the `inbox` its configuration names (an absolute
/// path), and its state directory.
pub fn target_of(index: &Path, member: &str) -> Result<Target, SteerError> {
    let row = registry::row_in(index, member)
        .map_err(SteerError::Index)?
        .ok_or_else(|| SteerError::NoSuchMember(member.to_string()))?;
    let entry = row
        .entry
        .ok_or_else(|| SteerError::NoConfig(format!("member `{member}` has no readable entry")))?;
    let cfg = entry.config.ok_or_else(|| {
        SteerError::NoConfig(format!(
            "member `{member}` was started without a config file"
        ))
    })?;
    let inbox = startup::inbox_of(&cfg)
        .map_err(SteerError::NoConfig)?
        .ok_or_else(|| {
            SteerError::NoInbox(format!("{} names no `inbox` directory", cfg.display()))
        })?;
    if !inbox.is_absolute() {
        return Err(SteerError::NoInbox(format!(
            "{}: `inbox` is relative ({}); the fleet will not guess what it is relative to",
            cfg.display(),
            inbox.display()
        )));
    }
    Ok(Target {
        inbox,
        state: entry.state_dir,
    })
}

/// Steer `member`: put `text` in its inbox as an owner-class item named by
/// `correlation`, from `from`. Idempotent by correlation id. The returned
/// promise is to be confirmed against the member's record.
pub fn steer(
    index: &Path,
    member: &str,
    from: &str,
    correlation: CorrelationId,
    text: &str,
) -> Result<Sent, SteerError> {
    let target = target_of(index, member)?;
    delivery::send(
        &target,
        &Delivery {
            correlation,
            kind: Kind::Steer,
            from: from.to_string(),
            text: text.to_string(),
        },
    )
    .map_err(SteerError::Delivery)
}
