//! Generic **Intake / Discharge** (the second-order design note: Intake/Discharge).
//!
//! The two atomic building blocks of cross-theory rectification, served
//! generically over **any** theory's carrier:
//!
//! - **INTAKE** — [`admit`]: a candidate subject must first pass the
//!   *destination* theory's audit — its membership / well-posedness gate
//!   ([`rung_het::Admits`]) — **then** it is added to the carrier set.
//!   Admission is always a re-audit under the destination's law: the source may
//!   say "not a question," but only the Issues theory can say whether it is a
//!   well-formed issue.
//! - **DISCHARGE** — [`discharge`]: remove a subject from the carrier set.
//!
//! This module is domain-blind by construction: it names neither `Questions`
//! nor `Issues` nor any concrete carrier strategy. The world supplies the gate
//! (via [`Admits`]), the carrier supplies the add/remove (via
//! [`ObjectCarrier`]), and the driver composes them.

use rung_het::Admits;

use crate::carrier::{CarrierError, ObjectCarrier, ObjectId};

/// Why an intake/discharge operation could not complete.
#[derive(Debug)]
pub enum IntakeError {
    /// The destination theory **refused** the candidate: it fails its own
    /// membership gate (`content_is_admissible` returned false). The subject
    /// is not admitted — it stays put rather than being degraded or dropped.
    Refused { id: ObjectId, reason: String },
    /// The carrier could not perform the write (add or remove).
    Carrier(CarrierError),
}

impl std::fmt::Display for IntakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused { id, reason } => write!(f, "intake refused {id}: {reason}"),
            Self::Carrier(e) => write!(f, "carrier error: {e}"),
        }
    }
}

impl std::error::Error for IntakeError {}

impl From<CarrierError> for IntakeError {
    fn from(e: CarrierError) -> Self {
        Self::Carrier(e)
    }
}

/// **ADMIT** — gate on the destination's audit, then add to the carrier.
///
/// `host` is the destination world (its [`Admits`] is the membership gate);
/// `carrier` is the carrier it governs; `id` is how the caller refers to the
/// subject (the id the source discharged it under); `candidate` is the
/// subject's content **re-formed as a member of the destination's sort** (a
/// work item's body, say, with the destination's frontmatter). Returns the id
/// the carrier assigned.
///
/// The order matters: the gate runs **first**. A candidate that fails the
/// destination's own membership screen is refused and never touches the
/// carrier — admission is gated on membership, not on the source's say-so.
pub fn admit<H: Admits>(
    host: &H,
    carrier: &dyn ObjectCarrier,
    id: &ObjectId,
    candidate: &str,
) -> Result<ObjectId, IntakeError> {
    if !host.content_is_admissible(candidate) {
        return Err(IntakeError::Refused {
            id: id.clone(),
            reason: "the candidate is not a well-formed member of the destination's sort"
                .to_string(),
        });
    }
    carrier
        .add(id, &host.render(candidate))
        .map_err(IntakeError::from)
}

/// **DISCHARGE** — remove a subject from the carrier set.
pub fn discharge(carrier: &dyn ObjectCarrier, id: &ObjectId) -> Result<(), IntakeError> {
    carrier.remove(id).map_err(IntakeError::from)
}
