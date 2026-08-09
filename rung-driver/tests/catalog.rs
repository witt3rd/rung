//! The catalog theory — a second-order theory over [`Instance`]s.
//!
//! Its edits admit/evict instances and route ejection rationales to them; its
//! decidable sentences audit the whole collection (unique names, present
//! carriers, routes that never dangle).

use std::path::PathBuf;

use rung_driver::{
    CarrierConfig, CarrierKind, Catalog, CatalogEdit, CatalogEntry, Instance,
    catalog::the_catalog::every_carrier_is_present,
    catalog::the_catalog::instances_are_named_uniquely,
    catalog::the_catalog::routes_target_admitted_instances,
};
use rung_het::{Applies, EnactError, Verify};

fn entry(name: &str, dir: &std::path::Path) -> CatalogEntry {
    CatalogEntry {
        name: name.to_string(),
        instance: Instance {
            theory: "rung-question".to_string(),
            carrier: CarrierConfig {
                kind: CarrierKind::Folder,
                path: Some(dir.to_string_lossy().into_owned()),
                repos: vec![],
            },
            population: None,
            state: None,
        },
        base: PathBuf::from(dir),
    }
}

fn mkdir(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("rung-catalog-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn admit_evict_and_route_are_higher_order_edits() {
    let qdir = mkdir("questions");
    let idir = mkdir("issues");

    let mut cat = Catalog::default();
    assert!(
        instances_are_named_uniquely::holds(&cat)
            .verdict()
            .is_conforming()
    );
    assert!(
        every_carrier_is_present::holds(&cat)
            .verdict()
            .is_conforming()
    );

    // Admit an instance; the after-state verifies and satisfies the sentences.
    cat.apply(
        "catalog",
        &CatalogEdit::Admit(entry("rung-questions", &qdir)),
    )
    .unwrap();
    cat.apply("catalog", &CatalogEdit::Admit(entry("rung-issues", &idir)))
        .unwrap();
    assert!(cat.confirms(
        &CatalogEdit::Admit(entry("rung-questions", &qdir)),
        "catalog"
    ));
    assert!(
        instances_are_named_uniquely::holds(&cat)
            .verdict()
            .is_conforming()
    );
    assert!(
        every_carrier_is_present::holds(&cat)
            .verdict()
            .is_conforming()
    );

    // Double-admit is refused (the pool will not hold two of the same name).
    let err = cat.apply(
        "catalog",
        &CatalogEdit::Admit(entry("rung-questions", &qdir)),
    );
    match err {
        Err(EnactError::TargetRefused { .. }) => {}
        other => panic!("expected TargetRefused, got {other:?}"),
    }

    // Route an ejection rationale to an admitted instance.
    cat.apply(
        "catalog",
        &CatalogEdit::Route {
            rationale: "not-well-posed-not-a-question".into(),
            target: "rung-issues".into(),
        },
    )
    .unwrap();
    assert!(cat.confirms(
        &CatalogEdit::Route {
            rationale: "not-well-posed-not-a-question".into(),
            target: "rung-issues".into(),
        },
        "catalog"
    ));
    assert!(
        routes_target_admitted_instances::holds(&cat)
            .verdict()
            .is_conforming()
    );

    // A route to a never-admitted instance is refused the moment it is made —
    // the routing set cannot target a ghost.
    let err = cat.apply(
        "catalog",
        &CatalogEdit::Route {
            rationale: "whatever".into(),
            target: "rung-backlog".into(),
        },
    );
    match err {
        Err(EnactError::TargetRefused { .. }) => {}
        other => panic!("expected TargetRefused, got {other:?}"),
    }

    // Evict an instance; routes pointing at it are dropped so the routing set
    // never dangles.
    cat.apply("catalog", &CatalogEdit::Evict("rung-issues".into()))
        .unwrap();
    assert!(cat.confirms(&CatalogEdit::Evict("rung-issues".into()), "catalog"));
    assert!(
        cat.routes.iter().all(|r| r.target != "rung-issues"),
        "routes to an evicted instance are dropped"
    );
    assert!(
        routes_target_admitted_instances::holds(&cat)
            .verdict()
            .is_conforming()
    );

    // Evicting a name that is not admitted is a miss.
    match cat.apply("catalog", &CatalogEdit::Evict("rung-issues".into())) {
        Err(EnactError::ObjectNotFound { .. }) => {}
        other => panic!("expected ObjectNotFound, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&qdir);
    let _ = std::fs::remove_dir_all(&idir);
}

#[test]
fn every_carrier_is_present_is_lived_not_declared() {
    let present = mkdir("present");
    let ghost = mkdir("ghost");
    // Remove the ghost dir so its carrier does not exist.
    std::fs::remove_dir(&ghost).unwrap();

    let mut cat = Catalog::default();
    cat.apply("catalog", &CatalogEdit::Admit(entry("present", &present)))
        .unwrap();
    cat.apply("catalog", &CatalogEdit::Admit(entry("ghost", &ghost)))
        .unwrap();

    // A catalog names the ghost instance, but its carrier is not reachable —
    // the lived-instance discipline as data, one level up.
    assert!(
        !every_carrier_is_present::holds(&cat)
            .verdict()
            .is_conforming()
    );

    let _ = std::fs::remove_dir_all(&present);
}
