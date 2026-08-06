//! The [`Carrier`] abstraction — how a driver reads `M(S)`.
//!
//! A `Carrier` is the *model side of the institution*: the set a sort ranges
//! over. Its whole interface is the extensional walk — enumerate the
//! inhabitants, fetch one. Everything else (parsing, judging, ordering) is not
//! a carrier's job.
//!
//! Strategies share one trait so the driver is domain-blind: it audits a
//! `Carrier` without knowing whether the subjects are a folder of question
//! files, a row-wise JSONL portfolio, or an external issue track.

use std::sync::Arc;

use super::{CarrierError, ObjectId};

/// The interface every carrier implementation satisfies.
///
/// `Send + Sync` so a carrier is held cheaply in `Arc<dyn Carrier>` and shared
/// across ladder states without a lock.
pub trait Carrier: std::fmt::Debug + Send + Sync {
    /// Stable identity for the carrier itself.
    fn id(&self) -> ObjectId;

    /// Whether this carrier currently exists and is accessible.
    fn exists(&self) -> bool;

    /// The extensional walk over `M(S)` — lazily, one subject id at a time.
    ///
    /// Satisfaction over a population is a proof about *every* subject, not a
    /// sample, so this yields all of them or faults trying.
    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_>;

    /// Read one subject's content by id.
    ///
    /// The content is **opaque** — the carrier fetches bytes; interpreting
    /// them is the theory's job.
    fn read(&self, item: &ObjectId) -> Result<String, CarrierError>;
}

/// Convenience alias — carriers are always shared by reference.
pub type CarrierRef = Arc<dyn Carrier>;
