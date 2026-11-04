//! The reporting line (`reporting-line-is-a-tree`).
//!
//! A chief directs stewards, a steward directs its own crew, and a crew member
//! reports only to the steward that started it: a tree of depth three. A role
//! is the first segment of a member's instance id (`a-member-is-named-by-its-role`).
//!
//! [`Line`] holds the parent of each member and refuses, at the edge that
//! would break it, anything that makes the relation more than a tree: a member
//! with a second parent, an edge whose roles are not the next tier down (so a
//! crew member cannot start crew, and a chief has no parent), and a cycle.
//! Since the only way to add an edge is [`Line::attach`], whatever sequence of
//! verbs ran, the relation stays acyclic and single-valued.

use std::collections::BTreeMap;
use std::fmt;

use rung_host::registry;

/// A member's position in the fleet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Chief,
    Steward,
    Crew,
}

impl Role {
    /// The role an instance name begins with, or `None` for a name that is
    /// not a member of a fleet.
    pub fn of(name: &str) -> Option<Role> {
        let id = registry::slug(name)?;
        match id.split('-').next()? {
            "chief" => Some(Role::Chief),
            "steward" => Some(Role::Steward),
            "crew" => Some(Role::Crew),
            _ => None,
        }
    }

    /// The role one tier down, which this role may direct.
    pub fn directs(self) -> Option<Role> {
        match self {
            Role::Chief => Some(Role::Steward),
            Role::Steward => Some(Role::Crew),
            Role::Crew => None,
        }
    }
}

/// Why an edge of the reporting line was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum LineError {
    /// A name does not begin with a role.
    NotAMember(String),
    /// The parent's role does not direct the child's role.
    WrongTier { parent: Role, child: Role },
    /// The child already reports to someone else.
    SecondParent { child: String, has: String },
    /// The edge would make a member its own ancestor.
    Cycle(String),
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineError::NotAMember(n) => write!(f, "{n:?} does not begin with a role"),
            LineError::WrongTier { parent, child } => {
                write!(f, "a {parent:?} does not direct a {child:?}")
            }
            LineError::SecondParent { child, has } => {
                write!(f, "{child} already reports to {has}")
            }
            LineError::Cycle(n) => write!(f, "{n} would be its own ancestor"),
        }
    }
}

impl std::error::Error for LineError {}

/// Who reports to whom, by instance id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    parent: BTreeMap<String, String>,
}

impl Line {
    pub fn new() -> Self {
        Line::default()
    }

    /// Record that `child` reports to `parent` (the edge a spawn makes).
    /// Attaching the same edge again changes nothing.
    pub fn attach(&mut self, parent: &str, child: &str) -> Result<(), LineError> {
        let id = |n: &str| registry::slug(n).ok_or_else(|| LineError::NotAMember(n.to_string()));
        let (p, c) = (id(parent)?, id(child)?);
        let role =
            |n: &str, id: &str| Role::of(id).ok_or_else(|| LineError::NotAMember(n.to_string()));
        let (pr, cr) = (role(parent, &p)?, role(child, &c)?);
        if pr.directs() != Some(cr) {
            return Err(LineError::WrongTier {
                parent: pr,
                child: cr,
            });
        }
        match self.parent.get(&c) {
            Some(has) if *has == p => return Ok(()),
            Some(has) => {
                return Err(LineError::SecondParent {
                    child: c,
                    has: has.clone(),
                });
            }
            None => {}
        }
        // Tiers strictly descend, so a cycle cannot arise; the walk keeps the
        // refusal true if the tier rule is ever relaxed.
        let mut up = Some(&p);
        while let Some(a) = up {
            if *a == c {
                return Err(LineError::Cycle(c));
            }
            up = self.parent.get(a);
        }
        self.parent.insert(c, p);
        Ok(())
    }

    /// The member `child` reports to; `None` for a chief and for an unknown name.
    pub fn parent_of(&self, child: &str) -> Option<&str> {
        self.parent.get(child).map(String::as_str)
    }

    /// Whether `parent` directs `child` directly.
    pub fn directs(&self, parent: &str, child: &str) -> bool {
        self.parent_of(child) == Some(parent)
    }

    /// Every `(parent, child)` edge, in child order.
    pub fn edges(&self) -> impl Iterator<Item = (&str, &str)> {
        self.parent.iter().map(|(c, p)| (p.as_str(), c.as_str()))
    }
}
