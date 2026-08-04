//! Panels — `⊨` with more than one judge (`panels`,
//! `panels-cannot-weaken-the-opponent`), and the shape that makes them possible
//! (`judgmental-is-kleisli-arrow`).
//!
//! These three propositions were carried as `deferred` on Q5 (fork-join
//! concurrency) and Q8 (the async driver). That deferral read more into them
//! than they say.
//!
//! **Q5 is about *concurrent* fork-join** — one argument split across N oracle
//! calls that run at the same time and are joined. Het says nothing about
//! simultaneity. `panels` says a panel is `⊨` with more than one judge and that
//! it is **not a separate construction**; a panel dispatched *in sequence* is
//! exactly that, and needs no primitive rung does not have. What Q5 buys is
//! latency, which is HetOpt's concern rather than Het's (`cut-at-valuation`).
//!
//! **Q8 is about the async driver.** A judgmental arrow is a Kleisli arrow
//! `A → 𝒫(B)`; the outside it consults may be a person, a file, or a model. A
//! *blocking* outside call works today — `rung-std`'s `LlmCall` ladder puts one
//! on the arrow. Async is a constraint on how the call is made, not a blocker
//! on whether the arrow is Kleisli.
//!
//! What these tests pin:
//!
//! - a panel is N ordinary `dispose` calls over one argument, each through its
//!   own licence minted against that argument, and the **combination rule is
//!   the theory's**;
//! - the combination cannot weaken the Opponent: an added oracle answer may
//!   take affirmation away and never grant it;
//! - `dispose` is `A → 𝒫(B)` and not `A → B`, demonstrably — one argument, two
//!   qualifying judges, two different and equally well-formed Dispositions.
//!
//! What they do **not** establish: that the combination rule is *right*. Which
//! rule a theory uses to combine (unanimity here, majority elsewhere) is the
//! theory's, exactly as its edits are (`edit-required-not-typed`). Nor do they
//! establish anything about running the seats concurrently; that is still Q5.

use rung_het::{
    Disposition, Pool, Principal, Proposal, Prov, Response, Role, Ruling, Steward, Verdict, dispose,
};

// ─────────────────────────────────────────────────────────────────────────
// The domain — a manuscript, its revisions, and the people around it
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Revision {
    Cut { lines: usize },
}

#[derive(Clone, Copy)]
struct Editor;
impl Role for Editor {
    const NAME: &'static str = "editor";
}

#[derive(Clone, Copy)]
struct Reader;
impl Role for Reader {
    const NAME: &'static str = "reader";
}

#[derive(Clone)]
struct Person {
    id: &'static str,
    prov: &'static [&'static str],
    roles: &'static [&'static str],
    stewards: &'static [&'static str],
}

impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name)
    }
    fn id(&self) -> &str {
        self.id
    }

    /// `authored` — the history this principal claims. `π(p)` is this
    /// **with `id()` added**, by the blanket `Provenanced` impl in `rung`:
    /// the provenance floor is not a value a principal gets to state.
    fn authored(&self) -> Prov {
        Prov::of(self.prov.iter().copied())
    }

    /// The oracle. The verdict is the outside's, not the caller's.
    fn rule(&self, _matter: &str) -> Response {
        Response::Rendered(Verdict::Conforming)
    }
}

impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

/// The author — steward of the manuscript, so it may propose; its provenance is
/// on the manuscript, so it may not judge what it proposed.
const AUTHOR: Person = Person {
    id: "author",
    prov: &["author"],
    roles: &["editor"],
    stewards: &["manuscript"],
};

fn reader(id: &'static str, prov: &'static [&'static str]) -> Person {
    Person {
        id,
        prov,
        roles: &["reader"],
        stewards: &[],
    }
}

/// Three readers. Every one of them qualifies against a proposal the author
/// wrote; none of them wrote it.
fn readers() -> Vec<Person> {
    vec![
        reader("first-reader", &["first-reader"]),
        reader("second-reader", &["second-reader"]),
        reader("third-reader", &["third-reader"]),
    ]
}

/// One seat on the panel: a pool of exactly one principal.
///
/// The theory owns its principals, so it may partition them, and each seat
/// mints its own licence against the very same argument. This is the whole of
/// the "new machinery" a panel needs — none.
fn seat(who: &Person) -> Pool<Person> {
    Pool::new(vec![who.clone()])
}

/// Run one argument past N judges — the theory's own combination, not the
/// library's.
///
/// Note what this is not: it is not in `rung-het`. Het says a panel is `⊨` with
/// an enlarged oracle-move set and that it is **not a separate construction**.
/// A `panel()` primitive in the library would make it one, and would legislate
/// a combination rule Het does not have.
fn convene(
    seats: &[Person],
    proposal: &Proposal<Revision>,
    each: impl Fn(&Person) -> Disposition,
) -> Vec<Ruling<Revision>> {
    seats
        .iter()
        .map(|who| {
            let q = seat(who)
                .qualify_for::<Reader>(proposal)
                .expect("every seat is disjoint from the author's proposal");
            dispose(proposal, q, each(who))
                .expect("the licence was minted against this very argument")
        })
        .collect()
}

/// This theory's combination rule: unanimity.
fn affirms_unanimously(rulings: &[Ruling<Revision>]) -> bool {
    !rulings.is_empty() && rulings.iter().all(Ruling::is_affirming)
}

// ─────────────────────────────────────────────────────────────────────────
// `panels` — the pass with more than one judge, and no new construction
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn a_panel_is_the_pass_with_more_than_one_judge() {
    let pool = Pool::new(vec![AUTHOR]);
    let pen = pool
        .authorize::<Editor, _>(&AUTHOR, "manuscript")
        .expect("the author holds standing over the manuscript");
    let proposal = Proposal::remedy(&pen, "ch-3", Revision::Cut { lines: 40 });

    let rulings = convene(&readers(), &proposal, |_| Disposition::Accept);

    assert_eq!(rulings.len(), 3, "one ruling per seat, and nothing else");
    assert_eq!(
        rulings.iter().map(Ruling::judge).collect::<Vec<_>>(),
        vec!["first-reader", "second-reader", "third-reader"],
        "each ruling names the principal that produced it — a panel is N \
         ordinary dispositions, not one anonymous aggregate"
    );
    assert!(
        rulings.iter().all(|r| r.object() == "ch-3"),
        "every seat ruled on the same argument"
    );
    assert!(affirms_unanimously(&rulings));
}

// ─────────────────────────────────────────────────────────────────────────
// `panels-cannot-weaken-the-opponent`
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn a_panel_cannot_weaken_the_opponent() {
    // A Proponent winning strategy in the original game remains winning in the
    // composite, and the extra oracle answers can only strengthen the Opponent.
    // The observable form: the panel affirms only where every seat affirmed, so
    // adding a seat may take a win away and can never grant one.
    let pool = Pool::new(vec![AUTHOR]);
    let pen = pool
        .authorize::<Editor, _>(&AUTHOR, "manuscript")
        .expect("the author holds standing over the manuscript");
    let proposal = Proposal::remedy(&pen, "ch-3", Revision::Cut { lines: 40 });

    let all = readers();

    // The original game — one judge, who accepts. The Proponent wins.
    let solo = convene(&all[..1], &proposal, |_| Disposition::Accept);
    assert!(
        affirms_unanimously(&solo),
        "against the first reader alone the cut is accepted"
    );

    // The composite — the same move, the same first answer, and two more. One
    // of them refuses, and the Proponent's win does not survive.
    let composite = convene(&all, &proposal, |who| {
        if who.id == "third-reader" {
            Disposition::RejectRemedy {
                reason: "cutting 40 lines loses the argument of the chapter".into(),
            }
        } else {
            Disposition::Accept
        }
    });

    assert!(
        !affirms_unanimously(&composite),
        "panels-cannot-weaken-the-opponent: an added oracle answer may take \
         affirmation away, never grant it"
    );

    // What makes it a strengthening rather than a change of game: the seat that
    // played in the original game answered identically here. The composite adds
    // answers; it does not revise them.
    assert_eq!(
        solo[0].disposition().name(),
        composite[0].disposition().name()
    );
    assert_eq!(solo[0].judge(), composite[0].judge());

    // And the refusal reaches the author, so the loss is a move rather than a
    // silence (`reason-is-not-an-edit`).
    assert_eq!(
        composite
            .iter()
            .find(|r| r.judge() == "third-reader")
            .and_then(Ruling::reason),
        Some("cutting 40 lines loses the argument of the chapter")
    );
}

// ─────────────────────────────────────────────────────────────────────────
// `judgmental-is-kleisli-arrow` — `A → 𝒫(B)`, not `A → B`
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn a_judgmental_arrow_returns_a_set_and_not_a_value() {
    // The claim is about *shape*, and the panel is what exhibits it: one
    // argument, two qualifying judges, two well-formed and different outcomes.
    // Were `dispose` an `A → B`, the second call could not disagree.
    let pool = Pool::new(vec![AUTHOR]);
    let pen = pool
        .authorize::<Editor, _>(&AUTHOR, "manuscript")
        .expect("the author holds standing over the manuscript");
    let proposal = Proposal::remedy(&pen, "ch-3", Revision::Cut { lines: 40 });

    let all = readers();
    let accepted = convene(&all[..1], &proposal, |_| Disposition::Accept);
    let refused = convene(&all[1..2], &proposal, |_| Disposition::RejectRemedy {
        reason: "not this chapter".into(),
    });

    assert_ne!(
        accepted[0].disposition().name(),
        refused[0].disposition().name(),
        "judgmental-is-kleisli-arrow: one argument admits more than one \
         outcome, each drawn from the outside. That is A → P(B)"
    );

    // Both are honest: each was produced through a licence minted against this
    // very proposal. The non-determinism is the outside, not a defect —
    // `no-preference-among-judges` forbids Het from ranking the two.
    assert!(accepted[0].is_affirming());
    assert!(!refused[0].is_affirming());
    assert!(
        !refused[0].is_terminal(),
        "reject-remedy re-enters the loop"
    );
}
