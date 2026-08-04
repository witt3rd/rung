//! The doctrine as a governed subject — audited by its own sentences, edited
//! through a typed vocabulary, and **verified against what an editor actually
//! did to the source**.
//!
//! The last of those is the point. An agent with `file-editing` will be the
//! thing that enacts a proposal, and the question that usually sinks
//! agent-edits-code is *how do you know it did what it said*. Here the typed
//! edit is the specification and the round trip is the check: apply the edit to
//! the doctrine value, render, and compare with what came back. An editor that
//! touched anything else is caught, whatever it was.

use rung::Verdict;
use rung_doctrine::governed::{DoctrineEdit, doctrine as doctrine_theory, proposition};
use rung_doctrine::{Doctrine, Element, Kind, Prop, Resolver, rung_ct};
use rung_het::Applies;

fn holds(s: &rung::Settled) -> bool {
    matches!(s.verdict(), Verdict::Conforming)
}

fn prop(slug: &str, parent: Option<&str>, prose: &str) -> Element {
    Element::Prop(Prop {
        slug: slug.into(),
        parent: parent.map(Into::into),
        kind: Kind::Rationale,
        numbering: None,
        prose: format!("{prose}\n\n"),
    })
}

fn small() -> Doctrine {
    Doctrine {
        file: "synthetic.md".into(),
        elements: vec![
            prop("zeta-root", None, "The root."),
            prop("zeta-first", Some("zeta-root"), "A remark on {#zeta-root}."),
            prop("zeta-second", Some("zeta-root"), "Another remark."),
        ],
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 1 · The sentences, over the real doctrine
// ════════════════════════════════════════════════════════════════════════════

/// Every decidable sentence holds over `rung-ct-props.md` as encoded.
#[test]
fn the_real_doctrine_satisfies_every_decidable_sentence() {
    let d = rung_ct::doctrine();
    let mut broken = Vec::new();
    for s in [
        doctrine_theory::slugs_are_unique::holds(&d),
        doctrine_theory::every_parent_resolves::holds(&d),
        doctrine_theory::every_reference_resolves::holds(&d),
        doctrine_theory::every_decidable_names_a_sentence::holds(&d),
        doctrine_theory::every_judgmental_names_a_role::holds(&d),
    ] {
        assert!(
            !s.consulted_outside(),
            "a decidable sentence called outside"
        );
        if let Verdict::NonConforming { reason } = s.verdict() {
            broken.push(format!("  {:<32} {reason}", s.sentence()));
        }
    }
    for p in d.props() {
        for s in [
            proposition::slug_is_kebab_case::holds(p),
            proposition::only_claims_carry_a_gate::holds(p),
        ] {
            if let Verdict::NonConforming { reason } = s.verdict() {
                broken.push(format!("  {:<32} #{} {reason}", s.sentence(), p.slug));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "the doctrine violates:\n{}",
        broken.join("\n")
    );
}

/// Each sentence can fail. A corpus built to break exactly one.
#[test]
fn every_sentence_can_fail() {
    let mut d = small();
    d.elements.push(prop("zeta-root", None, "A duplicate."));
    assert!(!holds(&doctrine_theory::slugs_are_unique::holds(&d)));

    let mut d = small();
    d.elements
        .push(prop("zeta-lost", Some("nobody"), "Orphan."));
    assert!(!holds(&doctrine_theory::every_parent_resolves::holds(&d)));

    let mut d = small();
    if let Element::Prop(p) = &mut d.elements[0] {
        p.kind = Kind::Decidable {
            sentence: String::new(),
        };
    }
    assert!(!holds(
        &doctrine_theory::every_decidable_names_a_sentence::holds(&d)
    ));

    let mut d = small();
    if let Element::Prop(p) = &mut d.elements[0] {
        p.kind = Kind::Judgmental {
            role: String::new(),
        };
    }
    assert!(!holds(
        &doctrine_theory::every_judgmental_names_a_role::holds(&d)
    ));
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · Edits, and the write-guard
// ════════════════════════════════════════════════════════════════════════════

/// Reparenting renumbers **by construction**. Nothing stores a number, so the
/// edit touches one field and the document reads differently.
#[test]
fn reparenting_renumbers_with_no_number_to_update() {
    let mut d = small();
    assert_eq!(d.numbers()["zeta-second"], "1.2");

    d.apply(
        "zeta-second",
        &DoctrineEdit::Reparent {
            under: Some("zeta-first".into()),
        },
    )
    .expect("zeta-first exists");

    assert_eq!(d.numbers()["zeta-second"], "1.11");
}

/// The triage, as an edit — and it may not produce a marker with nothing
/// behind it.
#[test]
fn reclassifying_refuses_a_gate_with_no_filler() {
    let mut d = small();

    assert!(
        d.apply(
            "zeta-root",
            &DoctrineEdit::Reclassify {
                to: Kind::Decidable {
                    sentence: String::new()
                }
            }
        )
        .is_err()
    );
    assert!(
        d.apply(
            "zeta-root",
            &DoctrineEdit::Reclassify {
                to: Kind::Judgmental {
                    role: String::new()
                }
            }
        )
        .is_err()
    );

    // Named, it goes through.
    d.apply(
        "zeta-root",
        &DoctrineEdit::Reclassify {
            to: Kind::Judgmental {
                role: "editor".into(),
            },
        },
    )
    .expect("a named role is a filler");
    assert!(d.by_slug("zeta-root").unwrap().kind.is_a_claim());
}

/// Retiring is refused where it would leave the document violating its own
/// sentences — cited, or with children.
#[test]
fn retiring_is_refused_when_it_would_break_the_document() {
    let mut d = small();
    assert!(
        d.apply("zeta-root", &DoctrineEdit::Retire).is_err(),
        "cited by zeta-first, and parent of two"
    );

    d.apply("zeta-second", &DoctrineEdit::Retire)
        .expect("uncited, childless");
    assert!(d.by_slug("zeta-second").is_none());
    assert!(holds(&doctrine_theory::every_parent_resolves::holds(&d)));
}

#[test]
fn a_proposition_cannot_become_its_own_parent() {
    let mut d = small();
    assert!(
        d.apply(
            "zeta-first",
            &DoctrineEdit::Reparent {
                under: Some("zeta-first".into())
            }
        )
        .is_err()
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · Verifying what an editor did
// ════════════════════════════════════════════════════════════════════════════

/// **The check that makes an agent-authored edit safe.**
///
/// A typed `DoctrineEdit` is the specification. Apply it to the value to get
/// the expected doctrine; whatever the editor produced is re-read and compared.
/// If the editor did anything besides the edit — improved neighbouring prose,
/// dropped a reference, renumbered by hand — the two differ.
///
/// Here the "editor" is a closure standing in for an agent. What matters is
/// that the comparison does not trust it.
#[test]
fn an_editor_that_does_more_than_the_edit_is_caught() {
    let before = small();
    let edit = DoctrineEdit::AmendProse {
        to: "The root, restated.\n\n".into(),
    };

    let mut expected = before.clone();
    expected.apply("zeta-root", &edit).expect("applies");

    // A faithful editor.
    let mut faithful = before.clone();
    if let Element::Prop(p) = &mut faithful.elements[0] {
        p.prose = "The root, restated.\n\n".into();
    }
    assert_eq!(
        render(&faithful),
        render(&expected),
        "faithful edit accepted"
    );

    // An editor that also "tidied" a neighbour.
    let mut helpful = before.clone();
    if let Element::Prop(p) = &mut helpful.elements[0] {
        p.prose = "The root, restated.\n\n".into();
    }
    if let Element::Prop(p) = &mut helpful.elements[2] {
        p.prose = "Another remark, improved.\n\n".into();
    }
    assert_ne!(
        render(&helpful),
        render(&expected),
        "an unrequested change slipped through"
    );

    // An editor that quietly dropped a reference.
    let mut lossy = before.clone();
    if let Element::Prop(p) = &mut lossy.elements[0] {
        p.prose = "The root, restated.\n\n".into();
    }
    if let Element::Prop(p) = &mut lossy.elements[1] {
        p.prose = "A remark on the root.\n\n".into();
    }
    assert_ne!(
        render(&lossy),
        render(&expected),
        "a lost reference slipped through"
    );
}

fn render(d: &Doctrine) -> String {
    d.render(&Resolver::new().with_doctrine(d))
        .expect("the synthetic doctrine resolves")
}

/// The same check over the **real** doctrine: apply an edit to the value,
/// render, and confirm the rendering differs from the document on disk in
/// exactly the way the edit says — one proposition's prose.
#[test]
fn an_edit_to_the_real_doctrine_shows_up_where_it_should_and_nowhere_else() {
    let before = rung_ct::doctrine();
    let mut after = before.clone();
    after
        .apply(
            "rungs-are-objects",
            &DoctrineEdit::AmendProse {
                to: "A **rung is an object**, and nothing else.\n\n".into(),
            },
        )
        .expect("the proposition exists");

    let numbers_before = before.numbers();
    let numbers_after = after.numbers();
    assert_eq!(
        numbers_before, numbers_after,
        "amending prose moved a number"
    );

    let changed: Vec<&String> = before
        .props()
        .zip(after.props())
        .filter(|(a, b)| a.prose != b.prose)
        .map(|(a, _)| &a.slug)
        .collect();
    assert_eq!(
        changed,
        ["rungs-are-objects"],
        "exactly one proposition changed"
    );
}

/// Amending prose does not renumber; reparenting does. Stated together because
/// the pair is what makes "numbers are derived" observable rather than asserted.
#[test]
fn only_structural_edits_renumber() {
    let d = rung_ct::doctrine();
    let before = d.numbers();

    let mut amended = d.clone();
    amended
        .apply(
            "the-law",
            &DoctrineEdit::AmendProse {
                to: "Changed.\n\n".into(),
            },
        )
        .unwrap();
    assert_eq!(amended.numbers(), before);

    let mut moved = d.clone();
    moved
        .apply(
            "the-law",
            &DoctrineEdit::Reparent {
                under: Some("rungs-are-objects".into()),
            },
        )
        .unwrap();
    let after = moved.numbers();
    let shifted = before
        .iter()
        .filter(|(k, v)| after.get(*k) != Some(*v))
        .count();
    assert!(shifted > 1, "one reparent moved {shifted} numbers");
}
