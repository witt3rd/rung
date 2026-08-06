//! The commission contribution record — the carrier Q16 ruled on, Q17 built.
//!
//! `authored(p)` is **derived** from this record by **lookup** — never
//! enumerated as a growing array in a principal's declaration. That is the
//! whole point of the carrier: `population.yaml` declares a principal's stable
//! [`family`](crate::PrincipalSpec::family), and this record supplies the
//! artifacts that family produced, so the pool can read them at qualification
//! time.
//!
//! The record holds `C(f, c)` — which artifacts family `f` produced under
//! commission `c` — and the **active commission set** `S`. For a principal `p`
//! of family `f`:
//!
//! ```text
//! authored(p) = \u22c3_{c\u2208S} C(f, c)
//! ```
//!
//! This is Q16's ruling verbatim. The *per-artifact attribution* shape (each
//! artifact names the family that produced it; `authored(f)` is the inverse
//! image) is informationally dual to this one, and either may be chosen; the
//! commission-indexed form matches Q14's language and reads as a lookup rather
//! than a per-principal list, so it is the one implemented here.
//!
//! ## The three conditions, and how the shape meets them
//!
//! - **Decidable at qualification.** `C` and `S` are finite declared facts the
//!   pool already possesses at dispatch; `authored(f)` is a plain set union.
//! - **Non-vacuous.** Inside an open commission a family cannot judge artifacts
//!   it produced under that commission: they are in `C(f, c)` for an active
//!   commission, hence in `authored(f)`, hence in `π(p)`.
//! - **Not total.** Artifacts in commissions that are closed **and not carried
//!   forward** fall out of `S`, so they are not in `authored(f)` — they remain
//!   open to later, disjoint instances of the same family.
//!
//! ## What it refuses to be
//!
//! **A guessed static list.** `authored` is never asserted by hand in
//! `population.yaml`; it is a function of what the harness records here. Until
//! a commission records a contribution, the record is empty and every family's
//! derived `authored` set is open — nothing is disqualified by fiction. The
//! log starts empty for a new commission and is filled by recording work, not
//! by retroactive invention.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A commission contribution record: `C(f, c)` and the active set `S`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommissionLog {
    /// The active commission set `S` — the current commission, plus any prior
    /// commissions the supplier **explicitly** carried forward. A newly opened
    /// commission is empty, and a prior commission enters only by explicit
    /// decision, never automatically.
    #[serde(default)]
    pub active: Vec<String>,
    /// `C`: family -> commission -> artifacts that family produced under that
    /// commission.
    #[serde(default)]
    pub contributions: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl CommissionLog {
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }

    /// `authored(f)` for a family: the union, over active commissions, of the
    /// artifacts that family produced.
    ///
    /// Returns a deduplicated, sorted set so the result is order-stable and
    /// cheaply comparable. Closed, non-carried-forward commissions contribute
    /// nothing, which is the "not total" condition.
    pub fn artifacts_for(&self, family: &str) -> Vec<String> {
        let mut set = BTreeSet::new();
        if let Some(by_commission) = self.contributions.get(family) {
            for c in &self.active {
                if let Some(list) = by_commission.get(c) {
                    for a in list {
                        set.insert(a.clone());
                    }
                }
            }
        }
        set.into_iter().collect()
    }

    /// Whether `family` has produced any artifact under the active commissions.
    /// A false answer means the family's `authored` set is open — nothing is
    /// disqualified, which is the honest, not-yet-recorded state.
    pub fn has_authored(&self, family: &str) -> bool {
        !self.artifacts_for(family).is_empty()
    }
}
