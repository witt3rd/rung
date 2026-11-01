//! A brief is the typed contract a task is assigned with
//! (`a-brief-is-a-typed-contract`).
//!
//! Every field is required and none has a default: the machinery refuses to
//! guess one. A struct literal that leaves a field out does not compile, and
//! an empty text field is refused by [`Brief::new`].

/// How much the worker may decide on its own. There is no `Default`: autonomy
/// is an explicit grant, never inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Autonomy {
    /// Do the work; ask before anything not named in the brief.
    Ask,
    /// Decide within the brief; report what was decided.
    Decide,
}

/// Why a brief was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BriefError {
    /// The named field was empty (or only whitespace).
    Empty(&'static str),
}

impl std::fmt::Display for BriefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BriefError::Empty(field) => write!(f, "brief field `{field}` is empty"),
        }
    }
}

impl std::error::Error for BriefError {}

/// What to build or learn, what counts as complete, how it is delivered, and
/// how much autonomy the worker has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brief {
    pub objective: String,
    pub done_when: String,
    pub delivery: String,
    pub autonomy: Autonomy,
}

impl Brief {
    /// Build a brief, refusing an empty text field.
    pub fn new(
        objective: impl Into<String>,
        done_when: impl Into<String>,
        delivery: impl Into<String>,
        autonomy: Autonomy,
    ) -> Result<Self, BriefError> {
        let brief = Brief {
            objective: objective.into(),
            done_when: done_when.into(),
            delivery: delivery.into(),
            autonomy,
        };
        brief.validate()?;
        Ok(brief)
    }

    /// Check every text field is non-empty (a literal can bypass `new`).
    pub fn validate(&self) -> Result<(), BriefError> {
        for (name, text) in [
            ("objective", &self.objective),
            ("done_when", &self.done_when),
            ("delivery", &self.delivery),
        ] {
            if text.trim().is_empty() {
                return Err(BriefError::Empty(name));
            }
        }
        Ok(())
    }
}
