//! Authority tokens (`authority-is-explicit-and-typed`,
//! `irreversible-action-needs-authority`).
//!
//! A transition that needs the principal's word takes a token that only the
//! principal's channel can produce. The channel is the host's own record: the
//! principal speaks through an **owner** item, and the host records the role of
//! every item it admits, so a peer (a steward, a crew member, a report) cannot
//! write one. [`Granted::from_record`] reads such an item; it is the only way
//! to start a token, and the token's fields are private.
//!
//! An act is a marker type ([`act`]). A [`Once`] token is for one act, is not
//! `Clone`, and is consumed by value; minting one is recorded in a ledger
//! directory, so the same grant cannot be minted twice, even by another
//! process. A [`Standing`] token is bound to a project and to the original
//! request and may be cloned, but only for the [`Routine`] acts: the
//! irreversible acts (stop, merge, discard, destructive and security-sensitive
//! work) accept a [`Once`] token and nothing else, so a standing grant cannot
//! be passed where a one-time one is asked, and no verb that needs authority
//! compiles without a token.

use std::fmt;
use std::marker::PhantomData;
use std::path::Path;

use rung_host::read;

mod sealed {
    pub trait Act {}
    pub trait Token {}
}

/// An act the principal may authorize. Sealed: the set is the fleet's.
pub trait Act: sealed::Act {
    /// The word a grant names it by.
    const WORD: &'static str;
}

/// An act a standing grant may cover: it can be undone or repeated safely.
pub trait Routine: Act {}

/// The acts, as marker types.
pub mod act {
    use super::{Act, Routine, sealed};

    macro_rules! acts {
        ($($(#[$m:meta])* $name:ident = $word:literal $(, routine $r:tt)?;)*) => {$(
            $(#[$m])*
            #[derive(Debug, Clone, Copy)]
            pub struct $name;
            impl sealed::Act for $name {}
            impl Act for $name { const WORD: &'static str = $word; }
            $(acts!(@routine $name $r);)?
        )*};
        (@routine $name:ident _) => { impl Routine for $name {} };
    }

    acts! {
        /// Bring a member into being.
        SpawnMember = "spawn", routine _;
        /// Give a member a task.
        AssignTask = "assign", routine _;
        /// Direct a member.
        SteerMember = "steer", routine _;
        /// Halt a member.
        StopMember = "stop";
        /// Merge work into a shared line.
        Merge = "merge";
        /// Discard work.
        Discard = "discard";
        /// Anything destructive or irreversible not named above.
        Destructive = "destructive";
        /// Anything security-sensitive.
        SecuritySensitive = "security-sensitive";
    }
}

/// Why a token could not be made.
#[derive(Debug, PartialEq, Eq)]
pub enum AuthorityError {
    /// The record could not be read.
    Record(String),
    /// No owner item has this id.
    NoGrant(String),
    /// The item is not a grant, or is malformed.
    NotAGrant(String),
    /// The grant names another act.
    WrongAct {
        granted: String,
        asked: &'static str,
    },
    /// The grant is one-time and cannot make a standing token, or the reverse.
    WrongScope(&'static str),
    /// A standing grant for another project or request.
    OutOfScope { granted: String, asked: String },
    /// The one-time grant was already used.
    Used(String),
    /// The ledger could not be written.
    Ledger(String),
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthorityError::Record(e) => write!(f, "record read failed: {e}"),
            AuthorityError::NoGrant(id) => write!(f, "no owner item {id:?} in the record"),
            AuthorityError::NotAGrant(why) => write!(f, "not a grant: {why}"),
            AuthorityError::WrongAct { granted, asked } => {
                write!(f, "the grant is for {granted:?}, not {asked:?}")
            }
            AuthorityError::WrongScope(w) => write!(f, "{w}"),
            AuthorityError::OutOfScope { granted, asked } => {
                write!(f, "the grant covers {granted:?}, not {asked:?}")
            }
            AuthorityError::Used(id) => write!(f, "grant {id:?} was already used"),
            AuthorityError::Ledger(e) => write!(f, "ledger write failed: {e}"),
        }
    }
}

impl std::error::Error for AuthorityError {}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Scope {
    Once,
    Standing { project: String, request: String },
}

/// What the principal granted, read from an owner item. Not yet a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Granted {
    id: String,
    act: String,
    scope: Scope,
}

impl Granted {
    /// The text of a grant, for the principal's channel to send:
    /// `[fleet:grant] <act> once` or `[fleet:grant] <act> standing <project> <request>`.
    pub fn text(act: &str, scope: Option<(&str, &str)>) -> String {
        match scope {
            None => format!("[fleet:grant] {act} once"),
            Some((p, r)) => format!("[fleet:grant] {act} standing {p} {r}"),
        }
    }

    /// Read the owner item `id` from the record in `state_dir`. An item of
    /// any other role is never a grant, whatever its text says.
    pub fn from_record(state_dir: &Path, id: &str) -> Result<Granted, AuthorityError> {
        let mut text = None;
        read::scan(state_dir, |l| {
            if text.is_none()
                && l.kind == "stimulus.accepted"
                && l.get("item")["id"].as_str() == Some(id)
                && l.get("item")["role"] == "owner"
            {
                text = l.get("item")["text"].as_str().map(str::to_string);
            }
        })
        .map_err(|e| AuthorityError::Record(format!("{e:?}")))?;
        let text = text.ok_or_else(|| AuthorityError::NoGrant(id.to_string()))?;
        Self::parse(id, &text)
    }

    fn parse(id: &str, text: &str) -> Result<Granted, AuthorityError> {
        let bad = |w: &str| AuthorityError::NotAGrant(w.to_string());
        let rest = text
            .strip_prefix("[fleet:grant] ")
            .ok_or_else(|| bad("no `[fleet:grant]` prefix"))?;
        let mut w = rest.split_whitespace();
        let act = w.next().ok_or_else(|| bad("no act"))?.to_string();
        let scope = match w.next() {
            Some("once") => Scope::Once,
            Some("standing") => Scope::Standing {
                project: w.next().ok_or_else(|| bad("no project"))?.to_string(),
                request: w.next().ok_or_else(|| bad("no request"))?.to_string(),
            },
            _ => return Err(bad("scope is `once` or `standing`")),
        };
        if w.next().is_some() {
            return Err(bad("trailing words"));
        }
        Ok(Granted {
            id: id.to_string(),
            act,
            scope,
        })
    }

    fn check_act<A: Act>(&self) -> Result<(), AuthorityError> {
        if self.act == A::WORD {
            Ok(())
        } else {
            Err(AuthorityError::WrongAct {
                granted: self.act.clone(),
                asked: A::WORD,
            })
        }
    }

    /// Make the one-time token. `ledger` is a directory the fleet keeps for
    /// this; the grant's id is created in it, and a second mint of the same id
    /// is refused.
    pub fn once<A: Act>(&self, ledger: &Path) -> Result<Once<A>, AuthorityError> {
        self.check_act::<A>()?;
        if self.scope != Scope::Once {
            return Err(AuthorityError::WrongScope(
                "a standing grant does not make a one-time token",
            ));
        }
        std::fs::create_dir_all(ledger).map_err(|e| AuthorityError::Ledger(e.to_string()))?;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(ledger.join(&self.id))
        {
            Ok(_) => Ok(Once {
                id: self.id.clone(),
                _act: PhantomData,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(AuthorityError::Used(self.id.clone()))
            }
            Err(e) => Err(AuthorityError::Ledger(e.to_string())),
        }
    }

    /// Make the standing token for `project` and `request`, which must be the
    /// ones the principal named: a grant never widens.
    pub fn standing<A: Routine>(
        &self,
        project: &str,
        request: &str,
    ) -> Result<Standing<A>, AuthorityError> {
        self.check_act::<A>()?;
        match &self.scope {
            Scope::Standing {
                project: p,
                request: r,
            } if p == project && r == request => Ok(Standing {
                id: self.id.clone(),
                project: p.clone(),
                request: r.clone(),
                _act: PhantomData,
            }),
            Scope::Standing {
                project: p,
                request: r,
            } => Err(AuthorityError::OutOfScope {
                granted: format!("{p}/{r}"),
                asked: format!("{project}/{request}"),
            }),
            Scope::Once => Err(AuthorityError::WrongScope(
                "a one-time grant does not make a standing token",
            )),
        }
    }
}

/// Authority for one act, once. Not `Clone`: it is spent by being passed.
#[must_use = "an authority token is for one act; pass it to the verb that needs it"]
#[derive(Debug)]
pub struct Once<A: Act> {
    id: String,
    _act: PhantomData<fn() -> A>,
}

/// Authority for a routine act within one project and one request.
#[must_use = "an authority token is for an act; pass it to the verb that needs it"]
#[derive(Debug, Clone)]
pub struct Standing<A: Routine> {
    id: String,
    project: String,
    request: String,
    _act: PhantomData<fn() -> A>,
}

impl<A: Routine> Standing<A> {
    pub fn project(&self) -> &str {
        &self.project
    }
    pub fn request(&self) -> &str {
        &self.request
    }
}

/// A token that authorizes act `A`. The irreversible acts have one impl,
/// [`Once`]; the routine acts have two.
pub trait Grant<A: Act>: sealed::Token {
    /// The id of the owner item this authority came from, for the record of
    /// what was done under it.
    fn grant_id(&self) -> &str;
}

impl<A: Act> sealed::Token for Once<A> {}
impl<A: Act> Grant<A> for Once<A> {
    fn grant_id(&self) -> &str {
        &self.id
    }
}
impl<A: Routine> sealed::Token for Standing<A> {}
impl<A: Routine> Grant<A> for Standing<A> {
    fn grant_id(&self) -> &str {
        &self.id
    }
}

/// Either token for a routine act, for a caller that holds whichever the
/// principal granted. Irreversible acts have no such type.
#[must_use = "an authority token is for an act; pass it to the verb that needs it"]
#[derive(Debug)]
pub enum Any<A: Routine> {
    Once(Once<A>),
    Standing(Standing<A>),
}

impl<A: Routine> sealed::Token for Any<A> {}
impl<A: Routine> Grant<A> for Any<A> {
    fn grant_id(&self) -> &str {
        match self {
            Any::Once(t) => t.grant_id(),
            Any::Standing(t) => t.grant_id(),
        }
    }
}

impl Granted {
    /// The token for a routine act, whichever kind this grant is: standing
    /// when `scope` (project, request) is given, one-time otherwise.
    pub fn routine<A: Routine>(
        &self,
        ledger: &Path,
        scope: Option<(&str, &str)>,
    ) -> Result<Any<A>, AuthorityError> {
        match scope {
            Some((p, r)) => self.standing(p, r).map(Any::Standing),
            None => self.once(ledger).map(Any::Once),
        }
    }
}
