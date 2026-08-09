//! The first real relegation: the Questions instance discharges a work item
//! from its carrier, the catalog mediates which instance is the destination
//! (the ejection rationale routes to Issues), and the Issues instance admits it
//! — gated on *its own* membership — writing it onto the issues carrier.
//!
//! All file I/O is real, over temp-dir folder carriers on both sides. The
//! external GitHub carrier is the same `admit` path, with `add` creating an
//! issue; this test proves the shape on local folders so it runs offline with
//! no external side effects.

use std::path::{Path, PathBuf};

use rung_driver::{
    Carrier, CarrierConfig, CarrierKind, Catalog, CatalogEdit, CatalogEntry, FolderCarrier,
    Instance, ObjectId, admit, discharge,
};
use rung_het::{Admits, Applies};
use rung_std::issues::{Issue, Issues, Scheme, issues as issues_sentences};

const ISSUE_SCHEME: Scheme = Scheme {
    namespace: "rung-issues",
    root: "issues",
    id_prefix: "issue",
};

fn entry(name: &str, dir: &Path) -> CatalogEntry {
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

#[test]
fn questions_discharges_a_work_item_and_issues_admits_it() {
    let qdir = std::env::temp_dir().join(format!("rung-releg-q-{}", std::process::id()));
    let idir = std::env::temp_dir().join(format!("rung-releg-i-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&qdir);
    let _ = std::fs::remove_dir_all(&idir);
    std::fs::create_dir_all(&qdir).unwrap();
    std::fs::create_dir_all(&idir).unwrap();

    // The questions carrier holds a work-item: a Mode-B subject the authentic
    // cut says is "a decision / work item, not a determinate question."
    let question_body = r#"---
id: q-7
mode: B
---

Decide whether the router should pre-filter by rationale before intake.
"#;
    std::fs::write(qdir.join("q-7.md"), question_body).unwrap();
    let q_carrier = FolderCarrier::new(&qdir);
    let i_carrier = FolderCarrier::new(&idir);

    // The catalog mediates: rung-questions + rung-issues are admitted, and the
    // ejection rationale `not-well-posed-not-a-question` routes to Issues.
    let mut cat = Catalog::default();
    cat.apply(
        "catalog",
        &CatalogEdit::Admit(entry("rung-questions", &qdir)),
    )
    .unwrap();
    cat.apply("catalog", &CatalogEdit::Admit(entry("rung-issues", &idir)))
        .unwrap();
    cat.apply(
        "catalog",
        &CatalogEdit::Route {
            rationale: "not-well-posed-not-a-question".into(),
            target: "rung-issues".into(),
        },
    )
    .unwrap();

    // 1 · DISCHARGE — the Questions instance takes the work item out of its
    // carrier.
    discharge(
        &q_carrier,
        &ObjectId::new(qdir.join("q-7.md").to_string_lossy().into_owned()),
    )
    .expect("questions discharges the work item");

    // 2 · ROUTE — the catalog says the destination is the Issues instance.
    let route = cat
        .routes
        .iter()
        .find(|r| r.rationale == "not-well-posed-not-a-question")
        .unwrap();
    let target = cat
        .entry(&route.target)
        .expect("route targets an admitted instance");
    let issues_carrier = FolderCarrier::new(&target.base);

    // 3 · INTAKE — re-formed as a subject of the *destination* sort...
    let candidate = format!(
        "---
id: q-7
status: open
---

Work item: {question_body}
"
    );

    // ...the Issues world's own membership gate must admit it...
    let world = Issues::new(ISSUE_SCHEME, vec![]);
    assert!(
        world.content_is_admissible(&candidate),
        "the Issues theory admits its own kind"
    );

    // ...and admission writes it onto the issues carrier.
    let admitted = admit(&world, &issues_carrier, &ObjectId::new("q-7"), &candidate)
        .expect("issues admits the work item as a well-formed issue");
    assert!(admitted.as_str().ends_with("q-7.md"));

    // 4 · VERIFY — the question is gone from the questions carrier, the issue
    // is on the issues carrier, and it re-parses as an open issue.
    let q_walk: Vec<ObjectId> = q_carrier.iter().map(|r| r.unwrap()).collect();
    assert!(
        q_walk.is_empty(),
        "the work item left the questions carrier"
    );

    let i_walk: Vec<ObjectId> = i_carrier.iter().map(|r| r.unwrap()).collect();
    assert!(
        !i_walk.is_empty(),
        "the issues carrier holds the admitted item"
    );
    let text = i_carrier.read(i_walk.first().unwrap()).unwrap();
    let it = Issue::parse(ISSUE_SCHEME, &text, "", "q-7").unwrap();
    assert_eq!(it.id, "q-7");
    assert_eq!(it.status, "open");

    // The Issues world loads the admitted item from its own carrier.
    let reloaded = Issues::load(ISSUE_SCHEME, &idir);
    assert!(reloaded.by_id("q-7").is_some());

    // Sanity: the issues theory's sentences speak over the reloaded body.
    assert!(
        issues_sentences::ids_are_unique::holds(&reloaded)
            .verdict()
            .is_conforming()
    );

    let _ = std::fs::remove_dir_all(&qdir);
    let _ = std::fs::remove_dir_all(&idir);
}
