//! A doctrine as a **governed subject** — sentences that audit it, and edits
//! that may be proposed against it.
//!
//! This is what lets the audit-rectify pass be pointed at a normative document.
//! `lib.rs` can already say whether a doctrine resolves; here that becomes a
//! *theory* — sentences with gates, evaluated over the doctrine as a model —
//! and an edit vocabulary the pass may apply.
//!
//! ## Why the sentences are the same checks, and why that matters
//!
//! `Doctrine::check` and these sentences compute the same things. That is not
//! duplication: `check` is the renderer's own precondition, and these are
//! *claims about a model that a principal can be handed*. The difference is
//! what can be done with them — a `check` result is a value a caller may
//! ignore, a `Settled` sentence is something a pass audits on and proposes
//! against.
//!
//! ## The gap this does not close
//!
//! The interesting propositions about a doctrine are judgmental — *is this a
//! sentence or is it rationale*, *does the mechanism establish the claim* — and
//! nothing here can settle them. They are declared, and settling them needs an
//! outside that this repository mostly does not have (see the triage note).

use crate::{Doctrine, Kind, Prop, references};
use rung::{Prov, Provenanced, Role, Situated, theory};
use rung_het::{Applies, EnactError};

// ── roles ───────────────────────────────────────────────────────────────────

/// Rules on whether a proposition is a *claim* at all, as against signature or
/// rationale. The classification the triage proposes and cannot settle.
#[derive(Clone, Copy)]
pub struct Editor;
impl Role for Editor {
    const NAME: &'static str = "editor";
}

/// Rules on a categorical identification — whether the mathematics is right.
/// The role the 23 judgmental propositions of `rung-ct-props.md` name.
#[derive(Clone, Copy)]
pub struct CategoryTheorist;
impl Role for CategoryTheorist {
    const NAME: &'static str = "category-theorist";
}

/// **Authorial.** Amending a normative document is authorship and requires
/// standing over it — never competence to judge it.
#[derive(Clone, Copy)]
pub struct Maintainer;
impl Role for Maintainer {
    const NAME: &'static str = "maintainer";
}

// ── the model ───────────────────────────────────────────────────────────────

impl Provenanced for Doctrine {
    /// A doctrine's provenance is the slugs it holds. Per-proposition rather
    /// than per-document, so that authoring one proposition disqualifies a
    /// judge from that one and not from all of them.
    fn provenance(&self) -> Prov {
        Prov::of(self.props().map(|p| p.slug.clone()))
    }
}

impl Situated for Doctrine {
    fn container(&self) -> &str {
        &self.file
    }
}

impl Provenanced for Prop {
    fn provenance(&self) -> Prov {
        Prov::of([self.slug.clone()])
    }
}

impl Doctrine {
    /// Slugs declared more than once.
    pub fn duplicate_slugs(&self) -> Vec<String> {
        let mut seen: Vec<&str> = Vec::new();
        let mut dup: Vec<String> = Vec::new();
        for p in self.props() {
            if seen.contains(&p.slug.as_str()) && !dup.contains(&p.slug) {
                dup.push(p.slug.clone());
            }
            seen.push(&p.slug);
        }
        dup.sort();
        dup
    }

    /// `(slug, missing parent)`.
    pub fn dangling_parents(&self) -> Vec<(String, String)> {
        let known: Vec<&str> = self.props().map(|p| p.slug.as_str()).collect();
        let mut out: Vec<(String, String)> = self
            .props()
            .filter_map(|p| p.parent.as_ref().map(|q| (p, q)))
            .filter(|(_, q)| !known.contains(&q.as_str()))
            .map(|(p, q)| (p.slug.clone(), q.clone()))
            .collect();
        out.sort();
        out
    }

    /// `(slug, unresolved reference)` **within this doctrine**.
    ///
    /// A reference into a document not yet encoded is not a fault here — it is
    /// resolved by the `Resolver` at render time, and reporting it as dangling
    /// would make every cross-document link an error until the last migration
    /// lands.
    pub fn internal_dangling_references(&self) -> Vec<(String, String)> {
        let known: Vec<&str> = self.props().map(|p| p.slug.as_str()).collect();
        let mut out = Vec::new();
        for p in self.props() {
            for r in references(&p.prose) {
                if !known.contains(&r.as_str()) && self.claims_to_own(&r) {
                    out.push((p.slug.clone(), r));
                }
            }
        }
        out.sort();
        out
    }

    /// Whether a slug looks like one this doctrine should hold. Conservative:
    /// only slugs nothing else could own, which today means none — every
    /// unresolved reference is presumed external until the other documents are
    /// encoded and the `Resolver` can say otherwise.
    fn claims_to_own(&self, _slug: &str) -> bool {
        false
    }

    /// Propositions marked `Decidable` with no proof named.
    ///
    /// The marker's whole content is that a body exists; an empty name is the
    /// promise-someone-keeps failure, in the one place it could still occur.
    pub fn decidable_without_a_proof(&self) -> Vec<String> {
        self.props()
            .filter(|p| matches!(&p.kind, Kind::Decidable { proof } if proof.is_empty()))
            .map(|p| p.slug.clone())
            .collect()
    }

    /// Propositions owed a proof, with what is owed.
    ///
    /// The work queue, as a list. An audit that reports this is telling an
    /// author what to write.
    pub fn owed(&self) -> Vec<(String, String)> {
        self.props()
            .filter_map(|p| match &p.kind {
                Kind::Owed { why } => Some((p.slug.clone(), why.clone())),
                _ => None,
            })
            .collect()
    }

    /// Propositions marked `Owed` that do not say what is owed.
    pub fn owed_without_a_reason(&self) -> Vec<String> {
        self.props()
            .filter(|p| matches!(&p.kind, Kind::Owed { why } if why.is_empty()))
            .map(|p| p.slug.clone())
            .collect()
    }

    /// Propositions marked `Judgmental` with no role named.
    pub fn judgmental_without_a_role(&self) -> Vec<String> {
        self.props()
            .filter(|p| matches!(&p.kind, Kind::Judgmental { role } if role.is_empty()))
            .map(|p| p.slug.clone())
            .collect()
    }
}

// ── the sentences ───────────────────────────────────────────────────────────

theory!(doctrine for Doctrine {
    // Identity is the slug, so two propositions may not share one.
    decidable slugs_are_unique = |d: &Doctrine| d.duplicate_slugs().is_empty();

    // Every parent resolves, so every number derives.
    decidable every_parent_resolves = |d: &Doctrine| d.dangling_parents().is_empty();

    // Every internal reference resolves.
    decidable every_reference_resolves = |d: &Doctrine|
        d.internal_dangling_references().is_empty();

    // `tower-floor`'s clauses, over this theory's own declarations: a gate that
    // needs a filler has one. The macro refuses the alternative when a sentence
    // is *written*; these are propositions carrying a marker as data, where it
    // could still go missing.
    decidable every_decidable_names_a_proof = |d: &Doctrine|
        d.decidable_without_a_proof().is_empty();

    decidable every_judgmental_names_a_role = |d: &Doctrine|
        d.judgmental_without_a_role().is_empty();

    // An owed proof says what is owed. "Nothing establishes this" is not a
    // work item; "the conditional gate is unimplemented" is.
    decidable every_owed_proof_says_what_is_owed = |d: &Doctrine|
        d.owed_without_a_reason().is_empty();

    // Whether the sentence / signature / rationale partition is adequate at
    // all. A claim about the classification, so applying it cannot settle it.
    judgmental partition_is_adequate: Editor;
});

theory!(proposition for Prop {
    // A slug is kebab-case, so a reference can name it.
    decidable slug_is_kebab_case = |p: &Prop| {
        let s = &p.slug;
        !s.is_empty()
            && !s.starts_with('-')
            && !s.ends_with('-')
            && !s.contains("--")
            && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };

    // Signature and rationale carry no gate, because neither is a claim.
    decidable only_claims_carry_a_gate = |p: &Prop| match &p.kind {
        Kind::Signature | Kind::Rationale => !p.kind.is_a_claim(),
        Kind::Decidable { .. } | Kind::Judgmental { .. } | Kind::Owed { .. } => p.kind.is_a_claim(),
    };

    // Whether this is a claim about a model at all. Not computable: it is a
    // reading of the prose, and the whole point of the partition is that only a
    // reader can make it.
    judgmental is_a_claim_not_rationale: Editor;

    // Whether the categorical identification it asserts is right. The 23.
    judgmental the_mathematics_holds: CategoryTheorist;
});

// ── edits ───────────────────────────────────────────────────────────────────

/// What may be done to a doctrine in remedy. The theory's vocabulary, not
/// Het's (`edit-required-not-typed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DoctrineEdit {
    /// Change a proposition's prose, leaving its slug and place alone.
    AmendProse { to: String },
    /// Move it under a different parent. **Renumbers by construction** — there
    /// is no number to update.
    Reparent { under: Option<String> },
    /// Change what kind of thing it is. The triage, as an edit.
    Reclassify { to: Kind },
    /// Remove it. Nothing may still cite it.
    Retire,
}

impl Applies<DoctrineEdit> for Doctrine {
    fn territory(&self) -> &'static str {
        // Leaked deliberately: `Applies::territory` is `&'static str`, and a
        // doctrine's file name is owned. One leak per doctrine, at first use.
        Box::leak(self.file.clone().into_boxed_str())
    }

    fn apply(&mut self, object: &str, edit: &DoctrineEdit) -> Result<(), EnactError> {
        let idx = self
            .elements
            .iter()
            .position(|e| matches!(e, crate::Element::Prop(p) if p.slug == object))
            .ok_or_else(|| EnactError::ObjectNotFound {
                object: object.to_string(),
            })?;

        // The write-guard (`target-runs-its-own-models`): an authorization to
        // edit is not a licence to leave the document in a state its own
        // sentences refuse. Each arm checks what it could break.
        match edit {
            DoctrineEdit::AmendProse { to } => {
                if let crate::Element::Prop(p) = &mut self.elements[idx] {
                    p.prose = to.clone();
                }
            }
            DoctrineEdit::Reparent { under } => {
                if let Some(u) = under {
                    if self.by_slug(u).is_none() {
                        return Err(EnactError::TargetRefused {
                            target: self.file.clone(),
                            reason: format!("#{u} is not a proposition of this doctrine"),
                        });
                    }
                    if u == object {
                        return Err(EnactError::TargetRefused {
                            target: self.file.clone(),
                            reason: format!("#{object} cannot be its own parent"),
                        });
                    }
                }
                if let crate::Element::Prop(p) = &mut self.elements[idx] {
                    p.parent = under.clone();
                }
            }
            DoctrineEdit::Reclassify { to } => {
                if matches!(to, Kind::Decidable { proof } if proof.is_empty()) {
                    return Err(EnactError::TargetRefused {
                        target: self.file.clone(),
                        reason: "a decidable proposition must name the proof that establishes it"
                            .into(),
                    });
                }
                if matches!(to, Kind::Judgmental { role } if role.is_empty()) {
                    return Err(EnactError::TargetRefused {
                        target: self.file.clone(),
                        reason: "a judgmental proposition must name the role that settles it"
                            .into(),
                    });
                }
                if let crate::Element::Prop(p) = &mut self.elements[idx] {
                    p.kind = to.clone();
                }
            }
            DoctrineEdit::Retire => {
                if self
                    .props()
                    .any(|p| references(&p.prose).iter().any(|r| r == object))
                {
                    return Err(EnactError::TargetRefused {
                        target: self.file.clone(),
                        reason: format!(
                            "#{object} is still cited; retiring it would dangle a reference"
                        ),
                    });
                }
                if self.props().any(|p| p.parent.as_deref() == Some(object)) {
                    return Err(EnactError::TargetRefused {
                        target: self.file.clone(),
                        reason: format!(
                            "#{object} still has children; retiring it would orphan them"
                        ),
                    });
                }
                self.elements.remove(idx);
            }
        }
        Ok(())
    }
}
