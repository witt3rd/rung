//! Authority tokens: only an owner item in the principal's record starts one;
//! a one-time grant is minted once; a standing grant never widens; and every
//! verb that needs the principal's word refuses to compile without a token.

mod common;

use rung_fleet::authority::{AuthorityError, Granted, act};
use rung_testkit::TempDir;

fn principal(dir: &std::path::Path, items: &[(&str, &str, &str)]) {
    common::record_with(dir, items);
}

#[test]
fn only_an_owner_item_is_a_grant() {
    let root = TempDir::new("fleet-authority-owner");
    let s = root.path().join("chief");
    let text = Granted::text("stop", None);
    principal(
        &s,
        &[
            ("peer-says", "peer", &text),
            ("agent-says", "agent", &text),
            ("owner-says", "owner", &text),
            ("owner-chatter", "owner", "please stop"),
        ],
    );
    for id in ["peer-says", "agent-says", "nobody"] {
        assert!(
            matches!(
                Granted::from_record(&s, id),
                Err(AuthorityError::NoGrant(_))
            ),
            "{id}"
        );
    }
    assert!(matches!(
        Granted::from_record(&s, "owner-chatter"),
        Err(AuthorityError::NotAGrant(_))
    ));
    assert!(Granted::from_record(&s, "owner-says").is_ok());
}

#[test]
fn a_one_time_grant_makes_one_token_for_its_own_act() {
    let root = TempDir::new("fleet-authority-once");
    let r = root.path();
    let s = r.join("chief");
    principal(&s, &[("g1", "owner", &Granted::text("stop", None))]);
    let g = Granted::from_record(&s, "g1").unwrap();
    let ledger = r.join("ledger");

    assert!(matches!(
        g.once::<act::Merge>(&ledger),
        Err(AuthorityError::WrongAct { .. })
    ));
    assert!(matches!(
        g.standing::<act::SteerMember>("p", "r"),
        Err(AuthorityError::WrongAct { .. })
    ));
    let _token = g.once::<act::StopMember>(&ledger).unwrap();
    // Read again by another process or a retry: the ledger remembers.
    let again = Granted::from_record(&s, "g1").unwrap();
    assert_eq!(
        again.once::<act::StopMember>(&ledger).err(),
        Some(AuthorityError::Used("g1".into()))
    );
}

#[test]
fn a_standing_grant_never_widens() {
    let root = TempDir::new("fleet-authority-standing");
    let r = root.path();
    let s = r.join("chief");
    principal(
        &s,
        &[(
            "g2",
            "owner",
            &Granted::text("steer", Some(("proj-a", "req-1"))),
        )],
    );
    let g = Granted::from_record(&s, "g2").unwrap();
    let t = g.standing::<act::SteerMember>("proj-a", "req-1").unwrap();
    assert_eq!((t.project(), t.request()), ("proj-a", "req-1"));
    let _copy = t.clone(); // standing: may be reused
    for (p, q) in [("proj-b", "req-1"), ("proj-a", "req-2")] {
        assert!(matches!(
            g.standing::<act::SteerMember>(p, q),
            Err(AuthorityError::OutOfScope { .. })
        ));
    }
    assert!(matches!(
        g.once::<act::SteerMember>(&r.join("ledger")),
        Err(AuthorityError::WrongScope(_))
    ));
}

#[test]
fn a_malformed_grant_is_refused() {
    let root = TempDir::new("fleet-authority-bad");
    let s = root.path().join("chief");
    principal(
        &s,
        &[
            ("a", "owner", "[fleet:grant] stop"),
            ("b", "owner", "[fleet:grant] stop sometimes"),
            ("c", "owner", "[fleet:grant] steer standing proj"),
            ("d", "owner", "[fleet:grant] stop once and more"),
        ],
    );
    for id in ["a", "b", "c", "d"] {
        assert!(
            matches!(
                Granted::from_record(&s, id),
                Err(AuthorityError::NotAGrant(_))
            ),
            "{id}"
        );
    }
}

#[test]
fn the_refusals_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/stop_without_authority.rs");
    t.compile_fail("tests/ui/stop_with_standing.rs");
    t.compile_fail("tests/ui/standing_irreversible.rs");
    t.compile_fail("tests/ui/token_used_twice.rs");
    t.compile_fail("tests/ui/token_forged.rs");
    t.compile_fail("tests/ui/token_for_another_act.rs");
}
