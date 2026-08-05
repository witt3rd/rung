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
    assert_eq!(by_kind.values().sum::<usize>(), 108);
}

/// The triage across the whole corpus. Pinned so a later reading that moves a
/// proposition between kinds shows up here rather than in a diff nobody reads.
#[test]
fn the_corpus_triage_is_recorded() {
    let mut by_kind = std::collections::BTreeMap::new();
    for d in &all() {
        for p in d.props() {
            *by_kind.entry(p.kind.name()).or_insert(0usize) += 1;
        }
    }
    println!("\n  corpus triage: {by_kind:?}\n");
    assert_eq!(by_kind.get("decidable").copied(), Some(113));
    assert_eq!(by_kind.get("rationale").copied(), Some(151));
    assert_eq!(by_kind.get("signature").copied(), Some(64));
    assert_eq!(by_kind.get("judgmental").copied(), Some(49));
    assert_eq!(by_kind.get("owed").copied(), Some(3));
    assert_eq!(by_kind.values().sum::<usize>(), 380);
}

/// **Nothing is unclassified.** Every proposition carries a kind, because the
/// type has no variant for "not yet decided" — a migration that left some
/// unread would have had to say so in a field, and there is no field.
#[test]
fn every_proposition_in_the_corpus_carries_a_kind() {
    let n: usize = all().iter().map(|d| d.props().count()).sum();
    assert_eq!(n, 380);
}

/// **Every decidable proposition's proof resolves.**
///
/// Three forms count, and each is checked as what it is: a named test must name
/// a file that exists and an `fn` inside it; `(rustc)` stands for the compiler;
/// a checker must be a file that exists. A proof that resolves to nothing is
/// the promise-someone-keeps failure in the one place it could still occur.
///
/// Mutation: misspell any `proof:` and this reddens.
#[test]
fn every_decidable_proposition_names_a_proof_that_resolves() {
    use rung_doctrine::Kind;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    let mut checked = 0;
    let mut broken = Vec::new();
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        let Kind::Decidable { proof } = &p.kind else {
            continue;
        };
        checked += 1;
        if proof == "(rustc)" {
            continue; // the compiler is the proof; there is no file to open
        }
        // A proof may not be a doctest. rustdoc ignores the error code in a
        // `compile_fail,E0999` fence — and E0999 does not exist — so such a
        // test asserts exactly one thing, *this did not compile*, and cannot
        // tell the refusal it was written for from a typo. Ported from the
        // retired `_ledger.py`, which is the only reason the rule survived
        // deleting it (`no-guarantee-cites-a-compile-fail-doctest`).
        if proof.contains("/src/") {
            broken.push(format!(
                "  #{}: {proof} — inside a crate's src/, so it is a doctest",
                p.slug
            ));
            continue;
        }
        let (path, func) = match proof.split_once("::") {
            Some((path, func)) => (path, Some(func)),
            None => (proof.as_str(), None),
        };
        let full = root.join(path);
        let Ok(text) = std::fs::read_to_string(&full) else {
            broken.push(format!("  #{}: {proof} — no such file", p.slug));
            continue;
        };
        if let Some(func) = func
            && !text.contains(&format!("fn {func}"))
        {
            broken.push(format!("  #{}: {proof} — no such fn", p.slug));
        }
    }
    assert!(
        broken.is_empty(),
        "{} proof(s) do not resolve:\n{}",
        broken.len(),
        broken.join("\n")
    );
    println!("\n  {checked} decidable propositions, every proof resolves\n");
}

/// **How much of the decidable fragment has actually been proven.**
///
/// Naming a proof is clause one. Clause two is that the proof has been *seen to
/// fail* — a test that cannot fail is not a proof
/// (`a-refusal-test-that-cannot-fail`), and this repository knows it well
/// enough to have a proposition about it.
///
/// The mutation that demonstrates a failure is recorded in the conformance
/// ledger's mechanism prose. This counts how many, and reports rather than
/// asserts, because the number's job is to be visible: a decidable fragment
/// where most proofs have never been falsified is a green board making a
/// promise it has not kept.
#[test]
fn the_proven_fraction_of_the_decidable_fragment_is_reported() {
    use rung_doctrine::Kind;
    let ledger = std::fs::read_to_string(docs().join("conformance.md")).expect("the ledger");

    let mut decidable = 0;
    let mut demonstrated = 0;
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        if !matches!(p.kind, Kind::Decidable { .. }) {
            continue;
        }
        decidable += 1;
        let row = ledger
            .lines()
            .find(|l| l.contains(&format!("`{}`", p.slug)))
            .unwrap_or("");
        let lower = row.to_lowercase();
        if ["mutation", "mutate", "deleting", "removing", "break the"]
            .iter()
            .any(|m| lower.contains(m))
        {
            demonstrated += 1;
        }
    }

    println!(
        "\n  decidable: {decidable}\n  \
         with a demonstrated failure: {demonstrated} ({}%)\n  \
         naming a proof nobody has watched fail: {}\n",
        demonstrated * 100 / decidable.max(1),
        decidable - demonstrated
    );
    assert!(decidable > 0);
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
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        if let Kind::Judgmental { role } = &p.kind {
            assert_eq!(role, "category-theorist", "#{}", p.slug);
            n += 1;
        }
    }
    assert_eq!(n, 49);
}

/// Signature and rationale carry no gate, and that is structural: neither is a
/// claim that could be satisfied, so there is nothing for a principal to settle.
#[test]
fn only_claims_carry_a_gate() {
    use rung_doctrine::Kind;
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        match &p.kind {
            Kind::Signature | Kind::Rationale => assert!(!p.kind.is_a_claim(), "#{}", p.slug),
            Kind::Decidable { .. } | Kind::Judgmental { .. } | Kind::Owed { .. } => {
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

/// **The work queue.**
///
/// A proposition decidable in principle with nothing establishing it is
/// `Owed`, not `Judgmental`. The distinction is what keeps a judge from being
/// handed work no judge can do: `one-gate-unimplemented` is not waiting on a
/// mathematician, it is waiting on `#[conditional]`.
///
/// This prints the queue. An audit that reports it is telling an author what
/// to write — which is the only form in which the doctrine drives the
/// implementation rather than describing it.
#[test]
fn the_owed_proofs_are_the_work_queue() {
    let mut queue = Vec::new();
    for d in &all() {
        for (slug, why) in d.owed() {
            queue.push(format!("  #{slug}\n      {why}"));
        }
    }
    println!("\n  owed ({}):\n{}\n", queue.len(), queue.join("\n"));
    assert!(
        !queue.is_empty(),
        "an empty queue would mean every decidable proposition is proven"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// The conformance record
// ════════════════════════════════════════════════════════════════════════════

/// **The record is a view, and views cannot drift.**
///
/// `docs/conformance.md` used to be generated from a curated table that held,
/// per proposition, a verdict and the test establishing it — the same fact the
/// doctrine holds as `kind` and `proof`. It drifted within a day: six
/// propositions gained proofs in the doctrine while the table kept the old
/// citations, and nothing compared them.
///
/// Now it is rendered from the doctrine, so there is nothing to compare.
#[test]
fn the_conformance_record_is_rendered_from_the_doctrine() {
    let r = resolver();
    let rendered = rung_doctrine::conformance::render(&all(), &r);
    let on_disk = std::fs::read_to_string(docs().join("conformance.md")).expect("the record");
    assert_eq!(
        rendered, on_disk,
        "docs/conformance.md differs from what the doctrine renders — \
         run `cargo run -p rung-doctrine --bin render`"
    );
}

/// Every proposition appears exactly once, so the record cannot quietly omit
/// one. An unlisted proposition would read as a corpus that is smaller and
/// better-covered than it is.
#[test]
fn the_record_lists_every_proposition_once() {
    let r = resolver();
    let rendered = rung_doctrine::conformance::render(&all(), &r);
    for d in &all() {
        for p in d.props() {
            let key = format!("| `{}` |", p.slug);
            assert_eq!(
                rendered.matches(&key).count(),
                1,
                "#{} appears {} times in the record",
                p.slug,
                rendered.matches(&key).count()
            );
        }
    }
}

/// The mechanism prose survived the migration out of Python.
///
/// It is the one part of a conformance record a machine cannot derive — *why*
/// a proof is the right proof — so losing it in a refactor would be losing the
/// only curated content the record had.
#[test]
fn the_curated_mechanism_prose_survived() {
    let with_prose = all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
        .filter(|p| !p.mechanism.is_empty())
        .count();
    println!("\n  {with_prose} propositions carry curated mechanism prose\n");
    assert!(
        with_prose >= 115,
        "the Python table held 115; only {with_prose} survived"
    );
}

/// Ported from the retired `_ledger.py`: mechanism prose must cite by slug and
/// every citation must resolve.
///
/// A bare decimal in mechanism prose is the failure this whole scheme exists to
/// remove — a number written by hand, which the next renumbering makes a lie.
/// The prose says `{#slug}` and the number is generated, exactly as in the
/// documents themselves.
#[test]
fn mechanism_prose_cites_by_slug_and_every_citation_resolves() {
    let r = resolver();
    let mut broken = Vec::new();
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        if p.mechanism.is_empty() {
            continue;
        }
        for slug in rung_doctrine::references(&p.mechanism) {
            if r.get(&slug).is_none() {
                broken.push(format!(
                    "  #{}: cites {{#{slug}}}, which resolves to nothing",
                    p.slug
                ));
            }
        }
        // A bare `1.23` outside a code span or a path.
        for (i, _) in p.mechanism.match_indices(char::is_numeric) {
            let rest = &p.mechanism[i..];
            let token: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if token.contains('.')
                && token.matches('.').count() == 1
                && !token.ends_with('.')
                && p.mechanism[..i]
                    .chars()
                    .next_back()
                    .is_none_or(|c| c == ' ')
            {
                broken.push(format!(
                    "  #{}: bare number {token} — write it as {{#slug}}, which survives renumbering",
                    p.slug
                ));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "{} mechanism fault(s):\n{}",
        broken.len(),
        broken.join("\n")
    );
}

/// **A count written by hand in a document must match the doctrine.**
///
/// `docs/triage.md` once carried a per-document table I wrote out rather than
/// derived. It was wrong by two, both versions summed to 70, and nothing
/// caught it — in the note explaining why numbers should not be written by
/// hand.
///
/// The table is gone; `conformance.md` is generated and carries the per-document
/// counts. What remains by hand is the README's corpus table, because a reader
/// arriving at the repository should meet the shape without following a link.
/// This is what keeps it true.
///
/// Mutation: change any number in the README's kind table and this reddens.
#[test]
fn hand_written_counts_in_prose_match_the_doctrine() {
    use rung_doctrine::Kind;
    let mut by_kind = std::collections::BTreeMap::new();
    for p in all()
        .iter()
        .flat_map(|d| d.props().cloned().collect::<Vec<_>>())
    {
        *by_kind.entry(p.kind.name()).or_insert(0usize) += 1;
    }

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let readme = std::fs::read_to_string(root.join("README.md")).expect("README.md");

    let mut checked = 0;
    for (kind, count) in &by_kind {
        // Rows read `| **decidable** | ... | 113 |`.
        let Some(line) = readme
            .lines()
            .find(|l| l.starts_with(&format!("| **{kind}**")))
        else {
            continue;
        };
        let shown: usize = line
            .rsplit('|')
            .nth(1)
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or_else(|| panic!("README row for `{kind}` has no count: {line}"));
        assert_eq!(
            shown, *count,
            "README says {shown} {kind} propositions; the doctrine has {count}"
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        by_kind.len(),
        "the README's kind table does not cover every kind"
    );

    // And no stale per-document table crept back into the triage note.
    let triage = std::fs::read_to_string(root.join("docs/triage.md")).expect("triage.md");
    assert!(
        !triage.contains("| `rung-props.md` |"),
        "docs/triage.md carries a hand-written per-document table again; \
         conformance.md is generated and has one"
    );
    let _ = Kind::Rationale;
}
