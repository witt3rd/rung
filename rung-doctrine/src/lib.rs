//! rung's normative doctrine, encoded — the source the prose is rendered from.
//!
//! ## The inversion
//!
//! `docs/*-props.md` are, today, the source of truth: prose documents whose
//! integrity is checked after the fact by Python. This crate exists to invert
//! that. A doctrine is **declared**, type-checked, and the markdown is
//! **generated** from it.
//!
//! The difference is not tidiness. Under the current arrangement a dangling
//! reference is a check that fails; under this one it is a name that does not
//! resolve. A stale number is not possible, because there is no number to
//! write — numbers are derived from tree position at render time and appear
//! nowhere in the source. The rules `_props.py check` enforces do not get
//! reimplemented here; most of them stop being expressible.
//!
//! ## Nothing is left behind
//!
//! An encoding that took only the *sentences* — the claims a machine can
//! evaluate — and left signature and rationale in prose would leave two sources
//! of truth, which is the thing this is for. So every proposition travels.
//! [`Kind`] is what differs between them: not who is encoded, but **what the
//! encoding demands of each**.
//!
//! | kind | supplies | may not have |
//! |---|---|---|
//! | [`Kind::Decidable`] | a body evaluated over a model | a role — nothing is dispatched |
//! | [`Kind::Judgmental`] | a competence role | a body — it would launder a machine check |
//! | [`Kind::Signature`] | what it introduces — a sort, an operation | a gate; it declares rather than claims |
//! | [`Kind::Rationale`] | prose | a gate; it is an argument, not a claim |
//!
//! ## Faithfulness, and how it is measured
//!
//! A document holds more than propositions: a preamble, section headings, an
//! appendix. Those are carried as [`Element::Verbatim`], reproduced exactly and
//! claiming nothing.
//!
//! That escape hatch is also the honest measure of how far the encoding has
//! got. [`Doctrine::coverage`] reports what fraction of the document is
//! structured as against carried — and a migration that quietly widened the
//! verbatim blocks until it round-tripped would show up there as a number
//! going the wrong way.

pub mod rung_ct;

use std::collections::BTreeMap;
use std::fmt::Write as _;

// ════════════════════════════════════════════════════════════════════════════
// 1 · The model
// ════════════════════════════════════════════════════════════════════════════

/// What a proposition is, and therefore what it must supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A claim a machine settles. Carries a body evaluated over a model.
    Decidable,
    /// A claim needing an outside. Carries the competence role required.
    Judgmental { role: &'static str },
    /// Declares part of the signature — a sort, an operation. Not a claim
    /// about a model, so it has no gate.
    Signature,
    /// An argument. Not a claim that could be satisfied, so it has no gate.
    Rationale,
}

impl Kind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Decidable => "decidable",
            Self::Judgmental { .. } => "judgmental",
            Self::Signature => "signature",
            Self::Rationale => "rationale",
        }
    }

    /// Whether this kind is a claim that could be true or false. Signature and
    /// rationale are not, which is why neither carries a gate.
    pub fn is_a_claim(&self) -> bool {
        matches!(self, Self::Decidable | Self::Judgmental { .. })
    }
}

/// One proposition.
///
/// It carries no number. The number is derived from `parent` plus position at
/// render time, so there is nothing here for a renumbering to invalidate and
/// nothing a careless edit could make stale.
#[derive(Clone, Debug)]
pub struct Prop {
    /// Stable identity. Every reference names this and never a number.
    pub slug: &'static str,
    /// The proposition this is a remark on. `None` makes it a root.
    pub parent: Option<&'static str>,
    pub kind: Kind,
    /// A root may number its children flat under a letter (`G1`, `J2`) rather
    /// than by concatenation.
    pub numbering: Option<char>,
    /// The prose, with `{#slug}` where a reference goes. The reference's text
    /// is generated, so a reference cannot display a number that has moved.
    pub prose: &'static str,
}

/// A piece of a document, in order.
#[derive(Clone, Debug)]
pub enum Element {
    /// Reproduced exactly, claiming nothing: a preamble, a heading, an
    /// appendix. Counted by [`Doctrine::coverage`].
    Verbatim(&'static str),
    /// A proposition, rendered with its derived number.
    Prop(Prop),
}

/// One normative document, encoded.
#[derive(Clone, Debug)]
pub struct Doctrine {
    /// The file this renders to, and the name references from other documents
    /// use to reach it.
    pub file: &'static str,
    pub elements: Vec<Element>,
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · Derived numbering
// ════════════════════════════════════════════════════════════════════════════

impl Doctrine {
    pub fn props(&self) -> impl Iterator<Item = &Prop> {
        self.elements.iter().filter_map(|e| match e {
            Element::Prop(p) => Some(p),
            Element::Verbatim(_) => None,
        })
    }

    pub fn by_slug(&self, slug: &str) -> Option<&Prop> {
        self.props().find(|p| p.slug == slug)
    }

    /// Every proposition's number, derived from the tree.
    ///
    /// Roots take document order. A child concatenates: `1` → `1.1` → `1.11`,
    /// with the separator only at the first level. A root declaring a letter
    /// numbers its children flat under it, so a labelled subtree reads
    /// `G1..Gn` and the past-9 ambiguity of concatenation does not arise.
    pub fn numbers(&self) -> BTreeMap<&'static str, String> {
        let props: Vec<&Prop> = self.props().collect();
        let known: Vec<&str> = props.iter().map(|p| p.slug).collect();
        let mut kids: BTreeMap<&str, Vec<&Prop>> = BTreeMap::new();
        let mut roots: Vec<&Prop> = Vec::new();
        for p in &props {
            match p.parent {
                None => roots.push(p),
                Some(q) if known.contains(&q) => kids.entry(q).or_default().push(p),
                Some(_) => {}
            }
        }
        let mut out = BTreeMap::new();
        for (i, root) in roots.iter().enumerate() {
            walk(root, &(i + 1).to_string(), 0, &kids, &mut out);
        }
        out
    }
}

fn walk(
    node: &Prop,
    num: &str,
    depth: usize,
    kids: &BTreeMap<&str, Vec<&Prop>>,
    out: &mut BTreeMap<&'static str, String>,
) {
    out.insert(node.slug, num.to_string());
    let Some(children) = kids.get(node.slug) else {
        return;
    };
    if let Some(letter) = node.numbering {
        for (i, kid) in children.iter().enumerate() {
            walk(kid, &format!("{letter}{}", i + 1), 0, kids, out);
        }
        return;
    }
    for (i, kid) in children.iter().enumerate() {
        let sep = if depth == 0 { "." } else { "" };
        walk(kid, &format!("{num}{sep}{}", i + 1), depth + 1, kids, out);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · Resolving references
// ════════════════════════════════════════════════════════════════════════════

/// Where a slug lives and what number it currently has.
///
/// A reference's *text* is generated from this, which is what makes a stale
/// cross-reference impossible rather than merely detectable. While the other
/// documents are still prose, a carrier supplies their numbers by parsing them;
/// once they are encoded too, the resolver is built from the doctrines.
#[derive(Clone, Debug, Default)]
pub struct Resolver {
    entries: BTreeMap<String, (String, String)>,
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add every proposition of a doctrine.
    pub fn with_doctrine(mut self, d: &Doctrine) -> Self {
        for (slug, num) in d.numbers() {
            self.entries
                .insert(slug.to_string(), (d.file.to_string(), num));
        }
        self
    }

    /// Add a slug that lives in a document not yet encoded.
    pub fn with_external(&mut self, slug: &str, file: &str, number: &str) {
        self.entries
            .insert(slug.to_string(), (file.to_string(), number.to_string()));
    }

    pub fn get(&self, slug: &str) -> Option<(&str, &str)> {
        self.entries
            .get(slug)
            .map(|(f, n)| (f.as_str(), n.as_str()))
    }
}

/// What went wrong rendering.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// A `{#slug}` naming nothing the resolver knows.
    UnresolvedReference { in_prop: String, slug: String },
    /// A proposition naming a parent the doctrine does not hold.
    DanglingParent { slug: String, parent: String },
    /// Two propositions with one slug.
    DuplicateSlug { slug: String },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedReference { in_prop, slug } => write!(
                f,
                "#{in_prop} refers to {{#{slug}}}, which no document declares"
            ),
            Self::DanglingParent { slug, parent } => {
                write!(f, "#{slug} names a parent #{parent} that is not declared")
            }
            Self::DuplicateSlug { slug } => write!(f, "#{slug} is declared twice"),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 4 · Rendering
// ════════════════════════════════════════════════════════════════════════════

impl Doctrine {
    /// Faults a render would hit, all of them, rather than the first.
    pub fn check(&self, r: &Resolver) -> Vec<RenderError> {
        let mut errs = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for p in self.props() {
            if seen.contains(&p.slug) {
                errs.push(RenderError::DuplicateSlug {
                    slug: p.slug.to_string(),
                });
            }
            seen.push(p.slug);
        }
        for p in self.props() {
            if let Some(parent) = p.parent
                && self.by_slug(parent).is_none()
            {
                errs.push(RenderError::DanglingParent {
                    slug: p.slug.to_string(),
                    parent: parent.to_string(),
                });
            }
            for slug in references(p.prose) {
                if r.get(&slug).is_none() {
                    errs.push(RenderError::UnresolvedReference {
                        in_prop: p.slug.to_string(),
                        slug,
                    });
                }
            }
        }
        errs
    }

    /// The markdown, as the document should read.
    pub fn render(&self, r: &Resolver) -> Result<String, Vec<RenderError>> {
        let errs = self.check(r);
        if !errs.is_empty() {
            return Err(errs);
        }
        let numbers = self.numbers();
        let mut out = String::new();
        for element in &self.elements {
            match element {
                Element::Verbatim(text) => out.push_str(text),
                Element::Prop(p) => {
                    let attrs = match (p.parent, p.numbering) {
                        (Some(q), Some(l)) => {
                            format!(" data-parent=\"{q}\" data-numbering=\"{l}\"")
                        }
                        (Some(q), None) => format!(" data-parent=\"{q}\""),
                        (None, Some(l)) => format!(" data-numbering=\"{l}\""),
                        (None, None) => String::new(),
                    };
                    let num = numbers.get(p.slug).map_or("?", String::as_str);
                    let _ = write!(out, "<a id=\"{}\"{attrs}></a>\n**{num}** ", p.slug);
                    out.push_str(&expand(p.prose, self.file, r));
                }
            }
        }
        Ok(out)
    }

    /// How much of the document is structured, as against carried verbatim.
    ///
    /// The number to watch during a migration: a round trip achieved by
    /// widening the verbatim blocks is a round trip that proves nothing, and it
    /// shows here.
    pub fn coverage(&self) -> Coverage {
        let mut structured = 0;
        let mut verbatim = 0;
        for e in &self.elements {
            match e {
                Element::Prop(p) => structured += p.prose.len(),
                Element::Verbatim(t) => verbatim += t.len(),
            }
        }
        Coverage {
            structured_bytes: structured,
            verbatim_bytes: verbatim,
            props: self.props().count(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Coverage {
    pub structured_bytes: usize,
    pub verbatim_bytes: usize,
    pub props: usize,
}

impl Coverage {
    pub fn fraction(&self) -> f64 {
        let total = self.structured_bytes + self.verbatim_bytes;
        if total == 0 {
            return 0.0;
        }
        self.structured_bytes as f64 / total as f64
    }
}

/// The slugs a body refers to.
pub fn references(prose: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = prose;
    while let Some(i) = rest.find("{#") {
        rest = &rest[i + 2..];
        let Some(j) = rest.find('}') else { break };
        out.push(rest[..j].to_string());
        rest = &rest[j + 1..];
    }
    out
}

/// Replace every `{#slug}` with the link a reader sees. Same-document
/// references omit the filename, matching how the documents are written.
fn expand(prose: &str, file: &str, r: &Resolver) -> String {
    let mut out = String::new();
    let mut rest = prose;
    while let Some(i) = rest.find("{#") {
        out.push_str(&rest[..i]);
        rest = &rest[i + 2..];
        let Some(j) = rest.find('}') else {
            out.push_str("{#");
            break;
        };
        let slug = &rest[..j];
        if let Some((f, num)) = r.get(slug) {
            let target = if f == file { "" } else { f };
            let _ = write!(out, "[{num}]({target}#{slug})");
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}
