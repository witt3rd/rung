//! The driver, over a real ladder.
//!
//! `rung/tests/suspension.rs` proves the *channel* — that a judgmental forward
//! transition hands the argument back and that a resume edge takes a pen. It
//! holds exactly one suspension, in a local variable, for the length of one
//! test. That is the language guarantee and it stops there.
//!
//! This is the part that runs a composition: many suspended runs held at once,
//! evidence arriving for them in no particular order, and each released when —
//! and only when — the matter it awaits terminates.
//!
//! Most of what follows tests things the park **does not do**. That is the
//! right emphasis. A park that ordered its contents, capped their number,
//! bounded re-entry or timed anything out would still pass a round-trip test
//! while having quietly acquired a worth law, and a worth law inside mechanism
//! is the failure this whole design is arranged against
//! (`het-declares-no-worth-law`).

use rung::{
    Authorized, Judgment, Pool, Principal, Prov, Provenanced, Raised, Response, Role, Situated,
    Steward, Terminated, Verdict, ladder,
};
use rung_std::driver::Park;

// ── roles ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Referee;
impl Role for Referee {
    const NAME: &'static str = "referee";
}

/// The authorial role the resume edge requires — not the referee's, because the
/// principal that ruled is provenance-disjoint from the subject, which is
/// exactly what disqualifies it from writing to the subject.
#[derive(Clone, Copy)]
struct Keeper;
impl Role for Keeper {
    const NAME: &'static str = "keeper";
}

// ── principals ──────────────────────────────────────────────────────────────

struct Person {
    id: &'static str,
    prov: Prov,
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
    fn authored(&self) -> Prov {
        self.prov.clone()
    }
    fn rule(&self, _matter: &str) -> Response {
        Response::Rendered(Verdict::Conforming)
    }
}

impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        self.stewards.contains(&over)
    }
}

static KEEPER: Person = Person {
    id: "kit",
    prov: Prov::empty(),
    roles: &["keeper"],
    stewards: &["docket"],
};

fn referees() -> Pool<Person> {
    Pool::new(vec![Person {
        id: "rhea",
        prov: Prov::empty(),
        roles: &["referee"],
        stewards: &[],
    }])
}

fn keepers() -> Pool<Person> {
    Pool::new(vec![Person {
        id: KEEPER.id,
        prov: KEEPER.prov.clone(),
        roles: KEEPER.roles,
        stewards: KEEPER.stewards,
    }])
}

// ── the subject and the ladder ──────────────────────────────────────────────

/// An item under review. `raises` is what the referee cannot settle about it —
/// `None` means answerable now.
#[derive(Clone, PartialEq, Debug)]
struct Item {
    name: &'static str,
    raises: Option<&'static str>,
}

impl Provenanced for Item {
    fn provenance(&self) -> Prov {
        Prov::of([self.name])
    }
}

impl Situated for Item {
    fn container(&self) -> &str {
        "docket"
    }
}

#[derive(Clone, PartialEq)]
struct Outcome {
    judgment: Judgment,
}

impl Provenanced for Outcome {
    fn provenance(&self) -> Prov {
        self.judgment.provenance()
    }
}

ladder!(Review {
    Filed(Item)
        => #[judgmental(Referee)] Reviewed(Outcome)
        => { Closed }
    resume { revive: #[authorial(Keeper)] Suspended(Filed) => Filed }
} impl {
    reviewed = |filed, q| {
        match filed.payload.raises {
            Some(reference) => Err(Suspended {
                raised: ::rung::Raised::new(reference, "is_sound"),
                token: filed,
            }),
            None => Ok(Reviewed::new(Outcome { judgment: q.into_judgment() })),
        }
    },
    step = |_reviewed| { Ok(StepOutcome::Closed(Closed::new())) },
    revive = |s| { s.token },
});

/// Drive one item up to its first suspension. Panics if it does not suspend —
/// every caller here uses an item that raises.
fn suspend(item: Item) -> review::Suspended<review::Filed> {
    let licence = referees()
        .qualify_for::<Referee>(&item)
        .expect("rhea shares no provenance with any item here");
    match review::reviewed(review::Filed::new(item), licence) {
        Err(s) => s,
        Ok(_) => panic!("an item that raises a matter was reviewed anyway"),
    }
}

fn pen<'a>(keepers: &'a Pool<Person>) -> Authorized<'a, Keeper> {
    keepers
        .authorize::<Keeper, Person>(&KEEPER, "docket")
        .expect("kit stewards the docket")
}

fn item(name: &'static str, raises: &'static str) -> Item {
    Item {
        name,
        raises: Some(raises),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 1 · The round trip
// ════════════════════════════════════════════════════════════════════════════

/// Park a suspended run, hold it, release it on evidence, resume it through the
/// ladder's own authorial edge, and drive it to a terminal.
///
/// The whole point of the park is the *hold* in the middle: between the
/// dispatch that could not be settled and the evidence that settles it, the run
/// is somewhere that is not a local variable in one stack frame.
#[test]
fn a_parked_run_is_released_by_its_evidence_and_resumes_to_a_terminal() {
    let mut park: Park<review::Suspended<review::Filed>> = Park::new();
    let keepers = keepers();

    let suspended = suspend(item("axiom", "q-14"));
    let raised = suspended.raised.clone();
    park.park(suspended);
    assert_eq!(park.depth(), 1);

    let evidence = Terminated::of(&raised, "resolved");
    let mut released = park.claim(&evidence);
    assert_eq!(released.len(), 1);
    assert!(park.is_empty(), "a released run is no longer parked");

    // The park hands back the suspension; the *ladder* resumes it, with a pen.
    let filed = review::revive(released.remove(0), evidence, pen(&keepers));
    assert_eq!(filed.payload.name, "axiom");

    // And the revived run is live: it drives on. Answerable this time.
    let answerable = Item {
        name: "axiom",
        raises: None,
    };
    let licence = referees().qualify_for::<Referee>(&answerable).unwrap();
    let reviewed = review::reviewed(review::Filed::new(answerable), licence)
        .expect("nothing is raised the second time");
    assert!(matches!(
        review::step(reviewed),
        Ok(review::StepOutcome::Closed(_))
    ));
}

// ════════════════════════════════════════════════════════════════════════════
// 2 · What the park must not do
// ════════════════════════════════════════════════════════════════════════════

/// **No ordering.** Evidence for the *last* run parked releases that run while
/// the earlier ones stay put. A park that were a queue would release the head;
/// a park that were a stack would release the top and only then the rest.
///
/// This is `ordering-is-hetopts` at the mechanism level: which run moves is
/// decided by which question was answered, and the park has no say.
///
/// Mutation: make `claim` return the first parked run regardless of what the
/// evidence answers, and this reddens on the identity of the released run.
#[test]
fn evidence_releases_the_run_it_answers_and_not_the_one_parked_first() {
    let mut park = Park::new();
    park.park(suspend(item("first", "q-1")));
    park.park(suspend(item("second", "q-2")));
    park.park(suspend(item("third", "q-3")));

    // Answer the middle one. Not the head, not the top.
    let released = park.claim(&Terminated::of(&Raised::new("q-2", "is_sound"), "resolved"));

    assert_eq!(released.len(), 1);
    assert_eq!(released[0].token.payload.name, "second");
    assert_eq!(park.depth(), 2, "the unanswered runs stay parked");

    let still: Vec<&str> = park.awaiting().map(|r| r.reference()).collect();
    assert_eq!(still, ["q-1", "q-3"]);
}

/// **No preference among runs awaiting the same matter.** Two runs blocked on
/// one question is ordinary — that is what a shared premise looks like. One
/// terminal answers both, and `claim` returns both.
///
/// Returning one and keeping the other would make the park choose which run
/// proceeds first, which is precisely the judgment it is not entitled to make.
///
/// Mutation: `break` out of `claim`'s loop after the first match and this
/// reddens.
#[test]
fn one_terminal_releases_every_run_that_awaits_it() {
    let mut park = Park::new();
    park.park(suspend(item("alpha", "q-11")));
    park.park(suspend(item("beta", "q-11")));
    park.park(suspend(item("gamma", "q-12")));

    let released = park.claim(&Terminated::of(
        &Raised::new("q-11", "is_sound"),
        "resolved",
    ));

    let mut names: Vec<&str> = released.iter().map(|s| s.token.payload.name).collect();
    names.sort_unstable();
    assert_eq!(names, ["alpha", "beta"]);
    assert_eq!(park.depth(), 1, "gamma awaits a different matter");
}

/// **No bound on re-entry** (`no-bound-on-reentry`,
/// `guarded-reentry-is-eviction`). The same run suspends and resumes round
/// after round, with a payload that does not change, and the park never begins
/// to object.
///
/// Ten rounds is not a meaningful number; the meaningful claim is that nothing
/// in the park counts. A park that counted would be the eviction rule Het
/// forbids, relocated into mechanism where nobody would look for it.
#[test]
fn the_same_run_parks_and_resumes_without_bound() {
    let mut park = Park::new();
    let keepers = keepers();
    let subject = item("recurring", "q-4");

    let mut filed = review::Filed::new(subject.clone());
    for round in 0..10 {
        let licence = referees().qualify_for::<Referee>(&filed.payload).unwrap();
        let suspended = match review::reviewed(filed, licence) {
            Err(s) => s,
            Ok(_) => panic!("round {round}: expected a suspension"),
        };
        let raised = suspended.raised.clone();
        park.park(suspended);

        let mut released = park.claim(&Terminated::of(&raised, "resolved"));
        assert_eq!(released.len(), 1, "round {round}");
        filed = review::revive(
            released.remove(0),
            Terminated::of(&raised, "resolved"),
            pen(&keepers),
        );
        assert_eq!(filed.payload, subject, "round {round}");
    }
    assert!(park.is_empty());
}

/// **No cap on depth.** Nesting is normal — answering one question routinely
/// raises another, which is what Q11 raising Q12 was — so a park holds many
/// suspended runs at once and must not have an opinion about how many.
///
/// A fixed-size park would have to evict, and choosing which run to evict is a
/// worth judgment about which one matters least.
#[test]
fn depth_is_unbounded_and_every_parked_run_is_visible() {
    let mut park = Park::new();
    for i in 0..64 {
        let reference: &'static str = Box::leak(format!("q-{i}").into_boxed_str());
        park.park(suspend(item("nested", reference)));
    }
    assert_eq!(park.depth(), 64);
    assert_eq!(park.awaiting().count(), 64);

    // Each is individually reachable by its own evidence. No head, no top.
    let released = park.claim(&Terminated::of(
        &Raised::new("q-63", "is_sound"),
        "resolved",
    ));
    assert_eq!(released.len(), 1);
    assert_eq!(park.depth(), 63);
}

/// **A block that does not lift stays visible.** Composition-note item 7: the
/// inner run may never terminate, and the composition should make that visible
/// rather than resolve it. There is no timeout, no expiry, no reaping.
///
/// `awaiting` is the whole of the reporting surface, and it reports rather than
/// acts.
#[test]
fn a_matter_that_never_terminates_leaves_its_run_parked_and_named() {
    let mut park = Park::new();
    park.park(suspend(item("stuck", "q-13")));

    // Evidence arrives for everything except what this run awaits.
    for other in ["q-1", "q-4", "q-11", "q-12"] {
        assert!(
            park.claim(&Terminated::of(&Raised::new(other, "is_sound"), "resolved"))
                .is_empty(),
            "evidence for {other} released a run awaiting q-13"
        );
    }

    assert_eq!(
        park.depth(),
        1,
        "the block did not lift, and did not vanish"
    );
    let waiting: Vec<(&str, &str)> = park
        .awaiting()
        .map(|r| (r.reference(), r.matter()))
        .collect();
    assert_eq!(waiting, [("q-13", "is_sound")]);
}

// ════════════════════════════════════════════════════════════════════════════
// 3 · The reference stays the theory's
// ════════════════════════════════════════════════════════════════════════════

/// **The park never interprets the reference** (`pool-is-opaque`,
/// `raised-reference-is-opaque`). Matching is `Terminated::answers`, the
/// theory's own predicate, so anything the theory calls a reference works —
/// including strings no scheme would produce.
///
/// Mutation: replace `evidence.answers(..)` in `claim` with any comparison the
/// park performs itself — a prefix match, a case-insensitive one, a parse —
/// and one of these pairs stops behaving.
#[test]
fn any_reference_the_theory_names_is_matched_and_none_is_parsed() {
    let opaque = ["¶ anything at all §", "", "q-1/../q-2", "Q-1", "  q-1  "];

    for reference in opaque {
        let mut park = Park::new();
        park.park(suspend(item(
            "opaque",
            Box::leak(reference.to_string().into_boxed_str()),
        )));

        // Its own evidence releases it, whatever it looks like.
        let released = park.claim(&Terminated::of(&Raised::new(reference, "is_sound"), "done"));
        assert_eq!(released.len(), 1, "reference {reference:?} was not matched");
    }

    // And no near-miss does. `Q-1` is not `q-1`; a park that lowercased would
    // release a run on evidence for a different matter entirely.
    let mut park = Park::new();
    park.park(suspend(item("cased", "q-1")));
    assert!(
        park.claim(&Terminated::of(&Raised::new("Q-1", "is_sound"), "done"))
            .is_empty(),
        "the park treated two distinct references as one"
    );
    assert_eq!(park.depth(), 1);
}

/// **Evidence for nothing parked is not an error.** A park that rejected it
/// would be claiming to know which references exist, and that roster is the
/// theory's. It simply releases nothing.
#[test]
fn evidence_for_an_unparked_matter_releases_nothing_and_disturbs_nothing() {
    let mut park = Park::new();
    park.park(suspend(item("held", "q-4")));

    let released = park.claim(&Terminated::of(
        &Raised::new("q-99", "is_sound"),
        "dissolved",
    ));
    assert!(released.is_empty());
    assert_eq!(park.depth(), 1);
    assert_eq!(
        park.awaiting().map(|r| r.reference()).collect::<Vec<_>>(),
        ["q-4"]
    );
}

/// **The terminal is carried, not judged.** Het requires that a terminal was
/// *reached*, never which one (`nothing-further-required`). A dissolved
/// question releases its run exactly as a resolved one does; what to make of
/// the difference is the resuming theory's business.
///
/// Mutation: have `claim` skip evidence whose terminal is not `"resolved"` and
/// this reddens — which is the point, because that mutation is a plausible
/// convenience.
#[test]
fn every_terminal_releases_alike_and_the_park_reads_none_of_them() {
    for terminal in ["resolved", "dissolved", "abandoned", "withdrawn", ""] {
        let mut park = Park::new();
        park.park(suspend(item("any", "q-7")));
        let released = park.claim(&Terminated::of(&Raised::new("q-7", "is_sound"), terminal));
        assert_eq!(
            released.len(),
            1,
            "a run was held back on terminal {terminal:?}"
        );
    }
}
