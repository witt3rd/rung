//! Audit-only mode: see what's wrong, don't fix it (Q19 (1b)).
#[test]
fn audit_only_run_reports_without_rectifying() {
    use rung_driver::audit_run;
    use rung_std::questions::Questions;

    let world = Questions::load(
        rung_std::questions::Scheme {
            namespace: "rung-questions",
            root: "questions",
            id_prefix: "q",
        },
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".het/rung-questions/questions"),
    );
    // audit-run: the pinned drift is found...
    let findings = audit_run(&world);
    assert!(!findings.is_empty(), "the drift must be reported");
    assert!(
        findings
            .iter()
            .any(|f| f.sentence == "affects_mirrors_inbound")
    );
    // ...and nothing is rectified (the world is unmutated, no edit was enacted)
    assert_eq!(
        audit_run(&world).len(),
        findings.len(),
        "audit is pure and non-destructive"
    );
}
