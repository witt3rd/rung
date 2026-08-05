//! The conformance record, rendered from the doctrine.
//!
//! ## Why this is here and not in a checker
//!
//! `docs/conformance.md` used to be generated from a curated Python table that
//! held, per proposition, a verdict and the test that established it. The
//! doctrine now holds both — [`Kind`] says what would settle a proposition and
//! `Kind::Decidable { proof }` says what does — so the same fact lived in two
//! places.
//!
//! It drifted within a day: six propositions gained proofs in the doctrine
//! while the Python table kept the old citations, and nothing compared them.
//! That is the failure this repository exists to notice, so the join is
//! computed from one place and the record is a **view**.
//!
//! ## What is not derivable
//!
//! [`Prop::mechanism`] — *why* a proof is the right proof for a claim. A
//! machine can check that a test exists and that it runs; it cannot check that
//! it establishes the proposition it is cited for. That is
//! `establishes_what_it_cites`, judgmental and unsettled, and the prose is a
//! human's answer to it standing in until a judge gives one.

use crate::{Doctrine, Element, Kind, Prop, Resolver, expand_refs};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Every proof any proposition names.
pub fn claimed(doctrines: &[Doctrine]) -> BTreeSet<String> {
    doctrines
        .iter()
        .flat_map(|d| d.props())
        .filter_map(|p| match &p.kind {
            Kind::Decidable { proof } => Some(proof.clone()),
            _ => None,
        })
        .collect()
}

/// Tests that prove no recorded proposition.
///
/// **The other direction of the gap, and the more interesting one.**
/// [`Kind::Owed`] is a proposition with no proof — it tells an author to write
/// a test. This is a proof with no proposition, and it tells an author
/// something harder: either the citation was never recorded, or **the suite is
/// checking a claim nobody wrote down.**
///
/// The second case is why this is worth counting. A test that guards a real
/// property the documents never state is a guarantee the project makes and
/// cannot account for — and it will be deleted one day by someone who reads it
/// as incidental, because nothing says otherwise.
///
/// Not filtered by crate. Some of these are tests of the tooling and will never
/// cite a proposition, and excluding them by name would be exactly the quiet
/// narrowing that makes a queue look shorter than it is. The reader can see the
/// file each one sits in.
pub fn unclaimed(doctrines: &[Doctrine], tests: &[String]) -> Vec<String> {
    let claimed = claimed(doctrines);
    tests
        .iter()
        .filter(|t| !claimed.contains(*t))
        .cloned()
        .collect()
}

/// What a kind means for conformance, for the legend.
fn meaning(kind: &str) -> &'static str {
    match kind {
        "decidable" => "a proof exists that fails when the proposition is violated",
        "judgmental" => "only a principal can settle it — no test decides this",
        "owed" => "decidable in principle; nothing establishes it **yet**",
        "signature" => "declares vocabulary; not a claim that could be satisfied",
        _ => "an argument, or a recorded limit; not a claim",
    }
}

/// What discharges a proposition of this kind, for the row.
fn proof_of(p: &Prop) -> String {
    match &p.kind {
        Kind::Decidable { proof } => format!("`{proof}`"),
        Kind::Judgmental { role, ruling: None } => format!("*awaits a* `{role}`"),
        Kind::Judgmental {
            ruling: Some(r), ..
        } => format!("`judgments/{r}`"),
        Kind::Owed { why } => format!("**owed** — {why}"),
        _ => "—".to_string(),
    }
}

/// Render the conformance record for a set of doctrines.
///
/// Sections follow the documents' own headings, which are carried in
/// [`Element::Verbatim`] blocks — so the record's shape tracks the documents
/// without a second table saying where each proposition sits.
pub fn render(doctrines: &[Doctrine], r: &Resolver, tests: &[String]) -> String {
    let mut out = String::from(
        "# Conformance\n\n\
         **Status: not normative, and generated.** The three `*-props.md` documents govern.\n\
         This is a **view of the doctrine** in [`rung-doctrine/`](../rung-doctrine/), written by\n\
         `cargo run -p rung-doctrine --bin render`. Editing it here does nothing; the next\n\
         render restores it, and CI fails if the two disagree.\n\n\
         Rows are keyed on a proposition's **slug**, never its number, so the record survives\n\
         every renumbering — and the numbers shown are themselves derived at render time.\n\n\
         A proposition's **kind** is what would settle it, which is also who it is dispatched\n\
         to. The two middle kinds route to structurally exclusive principals: judgment\n\
         requires provenance-disjointness, authorship requires standing.\n\n\
         | kind | meaning |\n|---|---|\n",
    );
    for k in ["decidable", "judgmental", "owed", "signature", "rationale"] {
        let _ = writeln!(out, "| `{k}` | {} |", meaning(k));
    }
    out.push_str(
        "\n**The mechanism column is the part no machine derives.** It says *why* a proof is\n\
         the right proof, which is a reading — `establishes_what_it_cites`, judgmental and\n\
         unsettled. Where it is blank, nobody has written one down.\n",
    );

    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();

    for d in doctrines {
        let numbers = d.numbers();
        let mut per_doc: BTreeMap<&str, usize> = BTreeMap::new();
        for p in d.props() {
            *per_doc.entry(p.kind.name()).or_default() += 1;
            *totals.entry(p.kind.name()).or_default() += 1;
        }
        let n: usize = per_doc.values().sum();

        let _ = write!(out, "\n---\n\n## `{}`\n\n**Counts.** ", d.file);
        let mut parts: Vec<String> = per_doc
            .iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect::<Vec<_>>();
        parts.push(format!("{n} total"));
        let _ = writeln!(out, "{}.\n", parts.join(" · "));

        // Walk elements in order so section headings land where the document
        // puts them, rather than in a hand-kept parallel list.
        let mut open = false;
        for e in &d.elements {
            match e {
                Element::Verbatim(text) => {
                    if let Some(h) = text.lines().find(|l| l.starts_with("## ")) {
                        if open {
                            out.push('\n');
                        }
                        let title = h.trim_start_matches("## ");
                        let title = title.split_once(" · ").map_or(title, |(_, t)| t);
                        let _ = write!(
                            out,
                            "### {title}\n\n| prop | slug | kind | mechanism | discharged by |\n\
                             |---|---|---|---|---|\n"
                        );
                        open = true;
                    }
                }
                Element::Prop(p) => {
                    if !open {
                        out.push_str(
                            "| prop | slug | kind | mechanism | discharged by |\n|---|---|---|---|---|\n",
                        );
                        open = true;
                    }
                    let num = numbers.get(&p.slug).map_or("?", String::as_str);
                    let mech = if p.mechanism.is_empty() {
                        "—".to_string()
                    } else {
                        expand_refs(&p.mechanism, "", r).replace('\n', " ")
                    };
                    let _ = writeln!(
                        out,
                        "| [{num}]({}#{}) | `{}` | `{}` | {mech} | {} |",
                        d.file,
                        p.slug,
                        p.slug,
                        p.kind.name(),
                        proof_of(p)
                    );
                }
            }
        }
    }

    let total: usize = totals.values().sum();
    let _ = write!(
        out,
        "\n---\n\n## The whole corpus\n\n| kind | count |\n|---|---:|\n"
    );
    for (k, v) in &totals {
        let _ = writeln!(out, "| `{k}` | {v} |");
    }
    let _ = writeln!(out, "| **total** | **{total}** |");

    out.push_str(
        "\n**What this table does not say.** Naming a proof is one thing; having watched it\n\
         fail is another. A test that cannot fail is not a proof, and the mutation that\n\
         demonstrates one is recorded in prose rather than counted here. Nor does anything\n\
         check that a cited proof is *apt* for the proposition citing it.\n",
    );

    // ── the other direction ────────────────────────────────────────────────
    let loose = unclaimed(doctrines, tests);
    let owed: usize = doctrines
        .iter()
        .flat_map(|d| d.props())
        .filter(|p| matches!(p.kind, Kind::Owed { .. }))
        .count();

    let _ = write!(
        out,
        "\n---\n\n## The gap, both ways\n\n\
         The join is not onto in either direction, and each direction is a queue.\n\n\
         | direction | meaning | tells an author to | count |\n|---|---|---|---:|\n\
         | **owed** | a proposition with no proof | write the test — or build the thing it would run against | {owed} |\n\
         | **unclaimed** | a proof with no proposition | record the citation, **or write the proposition it proves** | {} |\n\n\
         The second is the sharper one. A test guarding a real property the documents never\n\
         state is a guarantee this project makes and cannot account for — and one day someone\n\
         reads it as incidental and deletes it, because nothing says otherwise.\n\n\
         Not filtered by crate: some of these test the tooling and will never cite a\n\
         proposition. Excluding them by name would be the quiet narrowing that makes a queue\n\
         look shorter than it is.\n\n",
        loose.len()
    );

    let mut by_file: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for t in &loose {
        let (file, func) = t.split_once("::").unwrap_or((t.as_str(), ""));
        by_file.entry(file).or_default().push(func);
    }
    for (file, funcs) in &by_file {
        let _ = writeln!(out, "**`{file}`** — {} unclaimed\n", funcs.len());
        for f in funcs {
            let _ = writeln!(out, "- `{f}`");
        }
        out.push('\n');
    }
    out
}
