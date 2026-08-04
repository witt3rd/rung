//! Declaring a population — what each principal can do, and what it may write to.
//!
//! ## Capability declares; kind describes
//!
//! A principal is admitted to a role because it **declares the capabilities the
//! role requires**, and for no other reason. What it is made of — a model, an
//! agent wielding tools, a person — is recorded, and is never read by the
//! filter (`role-not-kind`: *"Kind is what a principal is made of… Role is what
//! the sentence needs done"*).
//!
//! That ordering is not a stylistic preference. Reading kind first would mean
//! `file-editing` was a thing agents do, so that a model which can edit files
//! is refused for being the wrong sort of thing. Reading capability first means
//! the question is only ever *can you do this*, and the answer is a declaration
//! anyone may make and anyone may fail to earn.
//!
//! It also composes in the one direction that stays honest later: filter by
//! capability, then order the survivors however you like. Kind enters neither
//! step, and cost enters only the second — which is why nothing here has an
//! opinion about price.
//!
//! ## Backing is an implementation detail
//!
//! [`Backing`] says how a principal answers when consulted. It is chosen by the
//! principal, not by the filter, and two principals with identical capabilities
//! may be backed differently. Nothing in the qualifying path reads it.

use serde::{Deserialize, Serialize};

/// What a principal is made of. Recorded for the receipt; never filtered on.
///
/// The variants mirror `rung_std::principals::Kind`, which is the supplier's
/// vocabulary for the same distinction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Llm,
    Agent,
    RelationalBeing,
    Human,
}

/// How a principal answers when it is consulted.
///
/// Declared separately from capabilities on purpose: *what* a principal can do
/// and *how it does it* are different facts, and only the first is a condition
/// on being dispatched to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "via", rename_all = "kebab-case")]
pub enum Backing {
    /// One blocking model call.
    Model { model: String },
    /// An agentic turn: drive a model, dispatch tools, iterate.
    Agent { model: String, tools: Vec<String> },
    /// Answers out of band. A person, or anything else that is asked by some
    /// route this driver does not own. The default: a principal nobody wired up
    /// has not been wired up, and says so rather than being assumed reachable.
    #[default]
    Outside,
}

impl Backing {
    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Model { model } | Self::Agent { model, .. } => Some(model),
            Self::Outside => None,
        }
    }
}

/// One principal, as declared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrincipalSpec {
    pub id: String,
    /// Recorded, not filtered on.
    pub kind: Kind,
    /// **The filter reads this and nothing else.** The atoms are the domain's
    /// — `reasoning`, `structured-outputs`, `tool-calling`, `code-generation`,
    /// `file-editing`, `web-research` are ordinary strings here, compared and
    /// never interpreted.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Containers this principal may write to. The authorial filter's second
    /// conjunct; capability alone never authorizes a write.
    #[serde(default)]
    pub standing: Vec<String>,
    /// `π(p)` — what this principal has authored. The judgmental filter's
    /// second conjunct, and the reason a principal cannot rule on its own work.
    #[serde(default)]
    pub authored: Vec<String>,
    #[serde(default = "outside")]
    pub backing: Backing,
}

fn outside() -> Backing {
    Backing::Outside
}

/// What a role requires. The domain's vocabulary, declared alongside the
/// population it will be resolved against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleSpec {
    pub name: String,
    /// Every one of these must be declared by a principal for it to be capable.
    #[serde(default)]
    pub requires: Vec<String>,
}

/// A population and the roles it is meant to fill.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Population {
    #[serde(default)]
    pub roles: Vec<RoleSpec>,
    #[serde(default)]
    pub principals: Vec<PrincipalSpec>,
}

/// What is wrong with a declaration.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    DuplicatePrincipal {
        id: String,
    },
    DuplicateRole {
        name: String,
    },
    /// A capability no role asks for. Not an error in itself — reported so an
    /// unused declaration is visible rather than silently inert.
    Unused {
        id: String,
        capability: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatePrincipal { id } => write!(f, "`{id}` is declared twice"),
            Self::DuplicateRole { name } => write!(f, "role `{name}` is declared twice"),
            Self::Unused { id, capability } => {
                write!(f, "`{id}` declares `{capability}`, which no role requires")
            }
        }
    }
}

impl Population {
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }

    pub fn role(&self, name: &str) -> Option<&RoleSpec> {
        self.roles.iter().find(|r| r.name == name)
    }

    pub fn by_id(&self, id: &str) -> Option<&PrincipalSpec> {
        self.principals.iter().find(|p| p.id == id)
    }

    /// Faults, all of them.
    pub fn check(&self) -> Vec<ConfigError> {
        let mut errs = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for p in &self.principals {
            if seen.contains(&p.id.as_str()) {
                errs.push(ConfigError::DuplicatePrincipal { id: p.id.clone() });
            }
            seen.push(&p.id);
        }
        let mut roles: Vec<&str> = Vec::new();
        for r in &self.roles {
            if roles.contains(&r.name.as_str()) {
                errs.push(ConfigError::DuplicateRole {
                    name: r.name.clone(),
                });
            }
            roles.push(&r.name);
        }
        let wanted: Vec<&str> = self
            .roles
            .iter()
            .flat_map(|r| r.requires.iter().map(String::as_str))
            .collect();
        for p in &self.principals {
            for c in &p.capabilities {
                if !wanted.contains(&c.as_str()) {
                    errs.push(ConfigError::Unused {
                        id: p.id.clone(),
                        capability: c.clone(),
                    });
                }
            }
        }
        errs
    }

    /// Whether a principal declares everything a role requires.
    ///
    /// **This is the whole of admission by competence.** It reads
    /// `capabilities` and nothing else — not kind, not backing, not cost. A
    /// role no one asked about admits nobody rather than everybody: an unknown
    /// role has no requirements to meet and no one has been said to meet it.
    pub fn capable(&self, spec: &PrincipalSpec, role: &str) -> bool {
        let Some(r) = self.role(role) else {
            return false;
        };
        r.requires
            .iter()
            .all(|need| spec.capabilities.iter().any(|has| has == need))
    }

    /// Everyone who could fill this role, by competence alone.
    ///
    /// Non-identity and standing are applied later, by the pool, against the
    /// particular argument — they are not properties of a principal but of a
    /// principal *and* a thing it is being asked about.
    pub fn capable_of(&self, role: &str) -> Vec<&PrincipalSpec> {
        self.principals
            .iter()
            .filter(|p| self.capable(p, role))
            .collect()
    }
}
