//! `reporting-line-is-a-tree`: whatever sequence of edges is attempted, the
//! parent relation is single-valued and acyclic, a chief has no parent, and
//! nothing below a steward directs anyone.

use std::collections::BTreeSet;

use rung_fleet::tree::{Line, LineError, Role};

/// A small deterministic generator, so a failure replays from its seed.
struct Rng(u64);
impl Rng {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

const NAMES: [&str; 12] = [
    "chief-main",
    "chief-second",
    "steward-alpha",
    "steward-beta",
    "steward-gamma",
    "crew-1",
    "crew-2",
    "crew-3",
    "crew-4",
    "worker-x",
    "Chief Third",
    "crew-1",
];

fn check(line: &Line) {
    let mut children = BTreeSet::new();
    for (p, c) in line.edges() {
        assert!(children.insert(c), "{c} has two parents");
        let (pr, cr) = (Role::of(p).unwrap(), Role::of(c).unwrap());
        assert_eq!(pr.directs(), Some(cr), "{p} -> {c} skips or inverts a tier");
        assert_ne!(cr, Role::Chief, "a chief has a parent");
        // Walk to the root: it ends within three members.
        let (mut at, mut depth) = (c, 1);
        while let Some(up) = line.parent_of(at) {
            at = up;
            depth += 1;
            assert!(depth <= 3, "deeper than three, or a cycle at {c}");
        }
        // A steward not yet attached to a chief is a root; a crew never is.
        assert_ne!(
            Role::of(at),
            Some(Role::Crew),
            "{c} is rooted at a crew member"
        );
    }
}

#[test]
fn whatever_sequence_of_edges_ran_the_line_is_a_tree_of_depth_three() {
    for seed in 0..200 {
        let mut rng = Rng(seed);
        let mut line = Line::new();
        for _ in 0..60 {
            let (p, c) = (NAMES[rng.next(NAMES.len())], NAMES[rng.next(NAMES.len())]);
            let _ = line.attach(p, c);
            check(&line);
        }
    }
}

#[test]
fn each_refusal_is_named() {
    let mut l = Line::new();
    l.attach("chief-main", "steward-alpha").unwrap();
    l.attach("steward-alpha", "crew-1").unwrap();
    l.attach("steward-alpha", "crew-1").unwrap(); // the same edge: nothing changes
    assert_eq!(l.edges().count(), 2);
    assert!(l.directs("steward-alpha", "crew-1"));
    assert_eq!(l.parent_of("steward-alpha"), Some("chief-main"));
    assert_eq!(l.parent_of("chief-main"), None);

    assert_eq!(
        l.attach("chief-second", "steward-alpha"),
        Err(LineError::SecondParent {
            child: "steward-alpha".into(),
            has: "chief-main".into()
        })
    );
    assert!(matches!(
        l.attach("crew-1", "crew-2"),
        Err(LineError::WrongTier { .. })
    ));
    assert!(matches!(
        l.attach("steward-alpha", "chief-main"),
        Err(LineError::WrongTier { .. })
    ));
    assert!(matches!(
        l.attach("chief-main", "crew-3"),
        Err(LineError::WrongTier { .. })
    ));
    assert!(matches!(
        l.attach("chief-main", "worker-x"),
        Err(LineError::NotAMember(_))
    ));
    assert_eq!(l.edges().count(), 2, "a refusal changes nothing");
}

#[test]
fn a_spawn_makes_the_edge_the_line_takes() {
    use rung_fleet::brief::{Autonomy, Brief};
    use rung_fleet::lifecycle::Spawn;
    let root = rung_testkit::TempDir::new("fleet-line-spawn");
    let brief = Brief::new("o", "d", "r", Autonomy::Ask).unwrap();
    let what = Spawn::new(brief, "crew-1").unwrap();
    let s = rung_fleet::spawn::spawn(root.path(), "crew-1", "steward-alpha", "t", &what, "mock")
        .unwrap();
    let (parent, child) = s.edge();
    let mut l = Line::new();
    l.attach(parent, child).unwrap();
    assert!(l.directs("steward-alpha", "crew-1"));
}
