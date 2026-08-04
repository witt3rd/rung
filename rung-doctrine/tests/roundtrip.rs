//! **The bet.** Render the encoded doctrine and compare it, byte for byte, to
//! the document checked into `docs/`.
//!
//! Everything downstream rests on this one test. If the encoding is lossless
//! the markdown can stop being the source of truth and become a build artifact;
//! if it is not, nothing else is safe to attempt. It is deliberately the
//! harshest available check — not "the propositions survive", not "the
//! references resolve", but *the file is the same file*.
//!
//! It also fails cheaply, which is the point of doing it first.

use rung_doctrine::{Doctrine, Element, Resolver, rung, rung_ct, rung_het};
use std::path::PathBuf;

fn docs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-doctrine sits in the workspace")
        .join("docs")
}

/// Numbers for slugs that live in documents not yet encoded.
///
/// `rung-ct-props.md` refers into `rung-het-props.md` and `rung-props.md`,
/// which are still prose. Their numbers are read out of them — the same
/// derivation, applied to a document that has not migrated yet. As each is
/// encoded this shrinks, and when the last one lands it is empty.
fn all() -> Vec<Doctrine> {
    vec![rung::doctrine(), rung_het::doctrine(), rung_ct::doctrine()]
}

/// **Every document is encoded, so the resolver needs nothing external.**
///
/// While a document was still prose its numbers had to be read off the page —
/// the very thing being abolished, kept alive as scaffolding. That scaffolding
/// is gone: the three doctrines resolve each other.
fn resolver() -> Resolver {
    let mut r = Resolver::new();
    for d in &all() {
        r = r.with_doctrine(d);
    }
    r
}

/// Read `(slug, number)` straight off a prose document. Migration scaffolding:
/// it reads the number a document *displays* rather than deriving it, which is
/// exactly the thing being abolished — and is why this function disappears when
/// the last document is encoded.
fn numbers_of(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("<a id=\"") else {
            continue;
        };
        let Some((slug, _)) = rest.split_once('"') else {
            continue;
        };
        let Some(next) = lines.get(i + 1) else {
            continue;
        };
        let Some(rest) = next.strip_prefix("**") else {
            continue;
        };
        let Some((num, _)) = rest.split_once("**") else {
            continue;
        };
        out.push((slug.to_string(), num.to_string()));
    }
    out
}

// ════════════════════════════════════════════════════════════════════════════

/// The whole bet, over every governing document.
#[test]
fn every_encoded_doctrine_renders_its_document_byte_for_byte() {
    let r = resolver();
    for d in &all() {
        one_document(d, &r);
    }
}

fn one_document(d: &Doctrine, r: &Resolver) {
    let faults = d.check(r);
    assert!(
        faults.is_empty(),
        "the encoding does not resolve:\n{}",
        faults
            .iter()
            .map(|e| format!("  {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let rendered = d.render(r).expect("checked above");
    let on_disk = std::fs::read_to_string(docs().join(&d.file)).expect("the document");

    if rendered != on_disk {
        // A byte diff over 731 lines is unreadable; report the first divergence
        // with enough context to act on.
        let a: Vec<&str> = rendered.lines().collect();
        let b: Vec<&str> = on_disk.lines().collect();
        let at = a
            .iter()
            .zip(b.iter())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        panic!(
            "rendered output differs from docs/{}\n\
             first divergence at line {}\n\
             rendered: {:?}\n\
             on disk:  {:?}\n\
             ({} lines rendered, {} on disk)",
            d.file,
            at + 1,
            a.get(at),
            b.get(at),
            a.len(),
            b.len()
        );
    }
}

/// Numbers are **derived**, and the derivation agrees with what the document
/// displays for every proposition in it.
///
/// The round trip above would catch a wrong number too, but this says which
/// one, which is the difference between a failing test and a usable one.
#[test]
fn every_derived_number_matches_the_document() {
    let mut wrong = Vec::new();
    let mut total = 0;
    for d in &all() {
        let derived = d.numbers();
        let on_disk: std::collections::BTreeMap<String, String> =
            numbers_of(&std::fs::read_to_string(docs().join(&d.file)).expect("the document"))
                .into_iter()
                .collect();
        total += derived.len();
        for (slug, num) in &derived {
            match on_disk.get(slug.as_str()) {
                Some(shown) if shown == num => {}
                Some(shown) => {
                    wrong.push(format!("  #{slug}: derives {num}, document shows {shown}"))
                }
                None => wrong.push(format!(
                    "  #{slug}: derived {num}, absent from the document"
                )),
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} number(s) disagree:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    println!("\n  {total} numbers, all derived, across three documents\n");
}

/// No proposition stores a number, and no prose contains a rendered link.
///
/// This is what makes renumbering impossible to get wrong rather than merely
/// checked: there is no number in the source to go stale, and no link text to
/// disagree with its target. A migration that left `[1.31](#...)` in a body
/// would round-trip perfectly and reintroduce exactly the problem being
/// removed — so it is asserted separately.
#[test]
fn the_source_holds_no_number_and_no_rendered_link() {
    let mut offenders = Vec::new();
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        let p = &p;
        if p.prose.contains("](#") || p.prose.contains("-props.md#") {
            offenders.push(format!("  #{}: prose holds a rendered link", p.slug));
        }
    }
    assert!(
        offenders.is_empty(),
        "{} proposition(s) hold a link that should be a reference:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// How much of the document is structured, as against carried verbatim.
///
/// Reported rather than asserted at a threshold: the number's job is to be
/// *visible* while the migration proceeds. A round trip achieved by widening
/// the verbatim blocks would show up here as this going down, which a green
/// tick would not reveal.
#[test]
fn coverage_is_reported() {
    let mut props = 0;
    println!();
    for d in &all() {
        let c = d.coverage();
        props += c.props;
        println!(
            "  {:<22} {:>3} propositions, {:.1}% structured",
            d.file,
            c.props,
            c.fraction() * 100.0
        );
    }
    println!("  {props} propositions encoded in total\n");
    assert_eq!(props, 380, "the corpus did not survive migration");
}

/// Where the triage has got to.
///
/// Reported and pinned. The counts are not a target — they are a record of a
/// reading, and if a later reading moves a proposition between kinds this test
/// is where that becomes visible rather than something a reader must diff for.
#[test]
fn the_triage_is_recorded() {
    let d = rung_ct::doctrine();
    let mut by_kind = std::collections::BTreeMap::new();
    for p in d.props() {
        *by_kind.entry(p.kind.name()).or_insert(0usize) += 1;
    }
    println!("\n  triage: {by_kind:?}\n");
    assert_eq!(by_kind.get("signature").copied(), Some(41));
    assert_eq!(by_kind.get("rationale").copied(), Some(41));
    assert_eq!(by_kind.get("judgmental").copied(), Some(23));
    assert_eq!(by_kind.get("decidable").copied(), Some(3));
    assert_eq!(by_kind.values().sum::<usize>(), 108);
}

/// **A decidable proposition names a sentence that exists.**
///
/// Without this the `Decidable` marker is a promise someone keeps — precisely
/// the failure mode the encoding exists to remove, reintroduced one level up.
/// The names are checked against the sentences the theories actually declare,
/// which is a fact about compiled code.
///
/// Mutation: misspell any `sentence:` and this reddens.
#[test]
fn every_decidable_proposition_names_a_declared_sentence() {
    use rung_doctrine::Kind;
    use rung_std::questions::{propagation, questions};

    let declared: Vec<&str> = questions::SENTENCES
        .iter()
        .chain(propagation::SENTENCES.iter())
        .map(|(name, _)| *name)
        .collect();

    let mut checked = 0;
    for p in rung_ct::doctrine().props() {
        if let Kind::Decidable { sentence } = &p.kind {
            assert!(
                declared.contains(&sentence.as_str()),
                "#{} names sentence `{sentence}`, which no theory declares. \
                 Declared: {declared:?}",
                p.slug
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 3, "the decidable fragment changed size");
}

/// **A judgmental proposition names a role**, and every one here names the same
/// one — because what makes these judgmental is identical in each case: they
/// assert a *mathematical identification that could be wrong*, and only a
/// mathematician can say.
///
/// The precedent is lived rather than assumed. Q7 (transitions are Prisms),
/// Q9 (the dependency structure is an opfibration) and Q10 (opfibrations
/// compose) were each settled by outside expert review, and the reviews are in
/// `docs/questions/resolved/_evidence/`. These 23 are the claims of that kind.
#[test]
fn every_judgmental_proposition_names_the_role_that_could_settle_it() {
    use rung_doctrine::Kind;
    let mut n = 0;
    for p in rung_ct::doctrine().props() {
        if let Kind::Judgmental { role } = &p.kind {
            assert_eq!(role, "category-theorist", "#{}", p.slug);
            n += 1;
        }
    }
    assert_eq!(n, 23);
}

/// Signature and rationale carry no gate, and that is structural: neither is a
/// claim that could be satisfied, so there is nothing for a principal to settle.
#[test]
fn only_claims_carry_a_gate() {
    use rung_doctrine::Kind;
    for p in rung_ct::doctrine().props() {
        match &p.kind {
            Kind::Signature | Kind::Rationale => assert!(!p.kind.is_a_claim(), "#{}", p.slug),
            Kind::Decidable { .. } | Kind::Judgmental { .. } => {
                assert!(p.kind.is_a_claim(), "#{}", p.slug)
            }
        }
    }
}

/// The verbatim escape hatch carries what a document has beyond propositions —
/// title, preamble, section headings, the appendix — and nothing else.
#[test]
fn verbatim_blocks_carry_only_non_propositional_matter() {
    let d = rung_ct::doctrine();
    for e in &d.elements {
        if let Element::Verbatim(t) = e {
            assert!(
                !t.contains("\n**1."),
                "a verbatim block contains what looks like a numbered proposition"
            );
        }
    }
}

/// **The whole corpus is encoded, and the scaffolding is gone.**
///
/// While any document was still prose, cross-document references had to be
/// resolved by reading numbers off the page — the very practice being
/// abolished, kept alive to bridge the migration. With all three encoded the
/// resolver is built from doctrines alone, and this asserts that no external
/// entry is needed to render any of them.
///
/// Mutation: drop one doctrine from `all()` and the other two stop resolving,
/// because their cross-references have nowhere to land.
#[test]
fn no_document_depends_on_a_number_read_off_a_page() {
    let r = resolver();
    let mut unresolved = Vec::new();
    for d in &all() {
        for e in d.check(&r) {
            unresolved.push(format!("  {e}"));
        }
    }
    assert!(
        unresolved.is_empty(),
        "the corpus does not resolve from its own encodings:\n{}",
        unresolved.join("\n")
    );

    // And every governing document in docs/ is one of the three.
    let mut on_disk: Vec<String> = std::fs::read_dir(docs())
        .expect("docs/")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with("-props.md"))
        .collect();
    on_disk.sort();
    let mut encoded: Vec<String> = all().iter().map(|d| d.file.clone()).collect();
    encoded.sort();
    assert_eq!(
        on_disk, encoded,
        "a governing document exists that nothing generates"
    );
}
