//! A declared principal, made dispatchable.
//!
//! [`Configured`] is the bridge from a [`PrincipalDecl`](rung_std::principals::PrincipalDecl) —
//! the unified principals model — to `rung`'s `Principal` and `Steward`, which
//! is what a `Pool` can run its filters against.
//!
//! ## Where the answer comes from
//!
//! `Principal::rule` is the outside call, and this type does not make it. It
//! holds an [`Oracle`], which is whatever actually asks: a model, an agent
//! wielding tools, a person reached by some other route.
//!
//! Keeping that behind a trait is not indirection for its own sake. Under R2
//! the outside supplies the verdict, so a `Configured` that computed one would
//! be the constant arrow with a config file in front of it. The oracle is the
//! outside, and the only thing this type does with an answer is carry it.

use crate::commission::CommissionLog;
use rung::{Pool, Principal, Prov, Raised, Response, Steward, Verdict};
use rung_std::principals::{Backing, PrincipalDecl, Roster};
use std::sync::Arc;

/// What an outside said when asked.
///
/// Two summands, mirroring `Response`: an answer, or a matter raised instead.
/// Nothing here can construct a `Judgment` — the seal is `rung`'s and this type
/// stays on the far side of it.
#[derive(Clone, Debug)]
pub enum Answer {
    /// It holds, or it does not, and why not.
    Verdict(Verdict),
    /// It could not be settled now, and here is what was raised.
    Raised(Raised),
}

impl Answer {
    /// Affirm. The shorthand a test double wants and a real oracle rarely does.
    pub fn holds() -> Self {
        Self::Verdict(Verdict::Conforming)
    }

    pub fn fails(reason: impl Into<String>) -> Self {
        Self::Verdict(Verdict::NonConforming {
            reason: reason.into(),
        })
    }
}

/// Whatever actually asks the outside.
///
/// Implemented by a model client, an agent runner, or a test double. The driver
/// does not care which, and the qualifying filters never see it.
pub trait Oracle: Send + Sync {
    /// Ask this principal about this matter.
    ///
    /// `backing` is passed so one oracle can serve a whole population — it says
    /// how this particular principal is meant to be reached.
    fn ask(&self, id: &str, backing: &Backing, matter: &str) -> Answer;
}

/// An oracle that raises a matter for everything it is asked.
///
/// Useful as a default, and honest as one: a driver with nothing wired up has
/// not answered anything, and saying so leaves the runs suspended and visible
/// rather than quietly conforming.
pub struct Unwired {
    pub reference: String,
}

impl Oracle for Unwired {
    fn ask(&self, _id: &str, _backing: &Backing, matter: &str) -> Answer {
        Answer::Raised(Raised::new(self.reference.clone(), matter.to_string()))
    }
}

/// A declared principal, dispatchable.
pub struct Configured<O: Oracle> {
    spec: PrincipalDecl,
    oracle: Arc<O>,
    /// The commission contribution record this principal's `authored` is
    /// derived from, when it has a [`family`](PrincipalDecl::family).
    log: Option<Arc<CommissionLog>>,
    /// The role this principal was admitted into a pool for, when built by
    /// [`population_pool`]. The pool's judgmental filter re-asks `capable` of
    /// the configured principal; this records that admission so the two cannot
    /// drift — a principal admitted into the pool for a role is capable of it.
    admitted: Option<String>,
}

impl<O: Oracle> Configured<O> {
    pub fn new(spec: PrincipalDecl, oracle: Arc<O>) -> Self {
        Self {
            spec,
            oracle,
            log: None,
            admitted: None,
        }
    }

    /// A principal whose provenance is **derived** from the commission record,
    /// keyed on its family. Use this (or
    /// [`population_pool_with_log`](crate::population_pool_with_log)) for a
    /// population of models; the plain [`Configured::new`] keeps `authored` as
    /// the principal's own static declaration, which is right for a person and
    /// wrong for a discontinuous kind.
    pub fn with_log(spec: PrincipalDecl, oracle: Arc<O>, log: Arc<CommissionLog>) -> Self {
        Self {
            spec,
            oracle,
            log: Some(log),
            admitted: None,
        }
    }

    pub fn spec(&self) -> &PrincipalDecl {
        &self.spec
    }
}

impl<O: Oracle> Principal for Configured<O> {
    /// **Capability, and nothing else.**
    ///
    /// Delegates to the unified model's claim-vs-earn check: a principal plays
    /// a role only when it both claims it and declares what the role's minimum
    /// qualifications require. A principal admitted into a pool for a role is
    /// also capable of it (the pool's filter re-asks this, so admission is
    /// recorded here rather than recomputed). Kind is not read and neither is
    /// backing.
    fn capable(&self, role_name: &str) -> bool {
        self.admitted.as_deref() == Some(role_name) || self.spec.capable(role_name)
    }

    fn id(&self) -> &str {
        &self.spec.id
    }

    fn authored(&self) -> Prov {
        // A principal with a family derives its stake from the commission
        // record — that is Q16's carrier, and it is why `authored` is a lookup
        // rather than a growing array in the declaration. A principal without
        // a family (a continuous kind, e.g. a person) carries its own genuine
        // record.
        match (&self.spec.family, &self.log) {
            (Some(family), Some(log)) => Prov::of(log.artifacts_for(family)),
            _ => Prov::of(self.spec.provenance.iter().cloned()),
        }
    }

    fn rule(&self, matter: &str) -> Response {
        match self.oracle.ask(&self.spec.id, &self.spec.backing, matter) {
            Answer::Verdict(v) => Response::Rendered(v),
            Answer::Raised(r) => Response::Deferred(r),
        }
    }
}

impl<O: Oracle> Steward for Configured<O> {
    fn has_standing(&self, over: &str) -> bool {
        self.spec.stewards.iter().any(|s| s == over)
    }
}

/// Build a pool from a roster, for one role.
///
/// Role requirements are resolved when the roster is loaded (its `from_yaml`
/// derives each principal's `plays` from the role vocabulary), so admission
/// here is a plain capability lookup over the unified model. Everyone capable
/// goes in. Non-identity is *not* applied — it is not a property of a principal
/// but of a principal and the thing it is asked about, so the pool applies it
/// per dispatch (`disjointness-against-argument`).
pub fn population_pool<O: Oracle>(
    roster: &Roster,
    role: &str,
    oracle: Arc<O>,
) -> Pool<Configured<O>> {
    population_pool_inner(roster, role, oracle, None)
}

/// Build a pool whose model principals derive `authored` from the commission
/// record. The real population route: pass the [`CommissionLog`] this
/// population's families record their work in, and `authored(p)` becomes a
/// lookup against it rather than a static declaration.
pub fn population_pool_with_log<O: Oracle>(
    roster: &Roster,
    role: &str,
    oracle: Arc<O>,
    log: Arc<CommissionLog>,
) -> Pool<Configured<O>> {
    population_pool_inner(roster, role, oracle, Some(log))
}

fn population_pool_inner<O: Oracle>(
    roster: &Roster,
    role: &str,
    oracle: Arc<O>,
    log: Option<Arc<CommissionLog>>,
) -> Pool<Configured<O>> {
    let members = roster
        .capable_of(role)
        .into_iter()
        .map(|spec| {
            let mut cfg = match &log {
                Some(log) => Configured::with_log(spec.clone(), oracle.clone(), log.clone()),
                None => Configured::new(spec.clone(), oracle.clone()),
            };
            cfg.admitted = Some(role.to_string());
            cfg
        })
        .collect();
    Pool::new(members)
}
