//! A declared principal, made dispatchable.
//!
//! [`Configured`] is the bridge from a [`PrincipalSpec`](crate::PrincipalSpec)
//! — id, capabilities, standing, provenance, backing — to `rung`'s `Principal`
//! and `Steward`, which is what a `Pool` can run its filters against.
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

use crate::config::{Backing, PrincipalSpec};
use rung::{Pool, Principal, Prov, Raised, Response, Steward, Verdict};

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
    spec: PrincipalSpec,
    oracle: std::sync::Arc<O>,
}

impl<O: Oracle> Configured<O> {
    pub fn new(spec: PrincipalSpec, oracle: std::sync::Arc<O>) -> Self {
        Self { spec, oracle }
    }

    pub fn spec(&self) -> &PrincipalSpec {
        &self.spec
    }
}

impl<O: Oracle> Principal for Configured<O> {
    /// **Capability, and nothing else.**
    ///
    /// The role's requirements are resolved against this principal's declared
    /// capabilities. Kind is not read here and neither is backing: a model that
    /// declares `file-editing` is capable of a role needing it, and an agent
    /// that does not declare it is not, whatever it could in principle do.
    ///
    /// The requirements travel with the role name in
    /// [`population_pool`](crate::population_pool), which resolves them once
    /// and stores the answer per principal — so this stays a lookup rather than
    /// a place where a filter could quietly grow.
    fn capable(&self, role_name: &str) -> bool {
        self.spec
            .capabilities
            .iter()
            .any(|c| c == role_name || c == &format!("role:{role_name}"))
    }

    fn id(&self) -> &str {
        &self.spec.id
    }

    fn authored(&self) -> Prov {
        Prov::of(self.spec.authored.iter().cloned())
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
        self.spec.standing.iter().any(|s| s == over)
    }
}

/// Build a pool from a population, for one role.
///
/// Role requirements are resolved **here**, once: every principal that declares
/// what the role needs is admitted to the pool tagged with that role's name, and
/// `Principal::capable` becomes a lookup. Doing it here rather than inside
/// `capable` keeps the competence rule in one readable place instead of
/// distributed across a trait impl.
///
/// Everyone capable goes in. Non-identity is *not* applied — it is not a
/// property of a principal but of a principal and the thing it is asked about,
/// so the pool applies it per dispatch (`disjointness-against-argument`).
pub fn population_pool<O: Oracle>(
    population: &crate::Population,
    role: &str,
    oracle: std::sync::Arc<O>,
) -> Pool<Configured<O>> {
    let members = population
        .capable_of(role)
        .into_iter()
        .map(|spec| {
            let mut spec = spec.clone();
            // Tag with the role it was admitted for, so `capable` is a lookup
            // and the requirement comparison lives in exactly one place.
            spec.capabilities.push(format!("role:{role}"));
            Configured::new(spec, oracle.clone())
        })
        .collect();
    Pool::new(members)
}
