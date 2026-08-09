//! The **catalog theory** — a second-order theory over [`Instance`]s
//! (a second-order theory, in the second-order design note).
//!
//! The set of available instances in a context is itself governed. This is the
//! theory over **that collection** — one level up from the theories the
//! instances audit. Its subjects are [`Instance`]s (each a theory bound to a
//! carrier, population and state home); its sentences audit the whole pool, and
//! its edits are the higher-order moves a subject-routing endpoint needs.
//!
//! This is what lets a pass say *"there is an Issues instance, its carrier is
//! reachable, route this work item there"* instead of guessing — the router's
//! selection reads the catalog, and intake's gate runs under the chosen
//! instance's own law.

use std::path::PathBuf;

use rung::theory;
use rung_het::{Applies, EnactError, Verify};

use crate::instance::Instance;

/// One admitted instance: a name, the sidecar declaration, and the directory
/// the declaration resolves against (so a colocated carrier/population/state
/// path is resolved *beside* the instance, per Q18).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// The instance's name in the catalog, e.g. `rung-questions`.
    pub name: String,
    pub instance: Instance,
    /// The instance's sidecar directory (`.het/<instance>/`).
    pub base: PathBuf,
}

/// A routing rule: an ejection rationale maps to a target instance by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The ejection rationale that triggers this route, e.g.
    /// `not-well-posed-not-a-question`.
    pub rationale: String,
    /// The target instance's name (an admitted [`CatalogEntry`]).
    pub target: String,
}

/// The catalog: the admitted instances plus the routing set.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
    pub routes: Vec<Route>,
}

impl Catalog {
    pub fn entry(&self, name: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

impl CatalogEntry {
    /// The concrete carrier this instance audits, with relative colocated
    /// paths resolved against the instance's sidecar directory.
    pub fn carrier(&self) -> Result<crate::carrier::CarrierRef, String> {
        self.instance.build_carrier_at(&self.base)
    }
}

/// A higher-order move over the collection of instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogEdit {
    /// Admit an instance to the pool.
    Admit(CatalogEntry),
    /// Evict an instance by name (and drop routes pointing at it).
    Evict(String),
    /// Route an ejection rationale to a target instance.
    Route { rationale: String, target: String },
}

impl Applies<CatalogEdit> for Catalog {
    fn territory(&self) -> &'static str {
        "catalog"
    }

    fn apply(&mut self, _object: &str, edit: &CatalogEdit) -> Result<(), EnactError> {
        match edit {
            CatalogEdit::Admit(entry) => {
                if self.entry(&entry.name).is_some() {
                    return Err(EnactError::TargetRefused {
                        target: entry.name.clone(),
                        reason: "an instance with this name is already admitted".into(),
                    });
                }
                self.entries.push(entry.clone());
            }
            CatalogEdit::Evict(name) => {
                if self.entry(name).is_none() {
                    return Err(EnactError::ObjectNotFound {
                        object: name.clone(),
                    });
                }
                self.entries.retain(|e| e.name != *name);
                // a route pointing at the evicted instance would dangle; drop it
                self.routes.retain(|r| r.target != *name);
            }
            CatalogEdit::Route { rationale, target } => {
                if self.routes.iter().any(|r| r.rationale == *rationale) {
                    return Err(EnactError::TargetRefused {
                        target: rationale.clone(),
                        reason: "an ejection rationale is routed at most once".into(),
                    });
                }
                if self.entry(target).is_none() {
                    return Err(EnactError::TargetRefused {
                        target: target.clone(),
                        reason: "a route must target an admitted instance".into(),
                    });
                }
                self.routes.push(Route {
                    rationale: rationale.clone(),
                    target: target.clone(),
                });
            }
        }
        Ok(())
    }
}

impl Verify<CatalogEdit> for Catalog {
    fn confirms(&self, edit: &CatalogEdit, _object: &str) -> bool {
        match edit {
            CatalogEdit::Admit(e) => self.entry(&e.name).is_some(),
            CatalogEdit::Evict(name) => self.entry(name).is_none(),
            CatalogEdit::Route { rationale, target } => self
                .routes
                .iter()
                .any(|r| r.rationale == *rationale && r.target == *target),
        }
    }
}

/// The judging role for `routing_is_complete` — a competent reasoner
/// (someone who can match an ejection rationale to an instance's description).
#[derive(Clone, Copy)]
pub struct Router;
impl rung::Role for Router {
    const NAME: &'static str = "router";
}

/// The catalog is itself provenance-bearing (its provenance is the catalog —
/// one level up from the instances it holds).
impl rung::Provenanced for Catalog {
    fn provenance(&self) -> rung::Prov {
        rung::Prov::of(["catalog".to_string()])
    }
}

theory!(the_catalog for Catalog {
    decidable instances_are_named_uniquely = |c: &Catalog|
        c.entries.iter().map(|e| &e.name)
            .collect::<std::collections::BTreeSet<_>>().len() == c.entries.len();

    // The lived-instance discipline as data, one level up: every admitted
    // instance's carrier is reachable (its `exists()` holds).
    decidable every_carrier_is_present = |c: &Catalog|
        c.entries.iter().all(|e| e.carrier().map(|cr| cr.exists()).unwrap_or(false));

    // Routing must not point at a ghost: every route's target is an admitted
    // instance.
    decidable routes_target_admitted_instances = |c: &Catalog|
        c.routes.iter().all(|r| c.entry(&r.target).is_some());

    // Whether some instance is a candidate destination for every ejection
    // rationale — a reasoner matches rationales to descriptions; no predicate
    // settles it.
    judgmental routing_is_complete: Router;
});

/// `Sen(Σ)` for the catalog theory.
pub fn sentences() -> Vec<(&'static str, &'static str)> {
    the_catalog::SENTENCES.to_vec()
}
