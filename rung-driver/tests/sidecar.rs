//! Q18 — the `.het/` state sidecar, actualized for rung's own bootstrap.
//!
//! One carrier's audit-rectify state lives in one place. The `.het/rung-questions/
//! config.yaml` is an [`Instance`]: the theory, the carrier (a folder over the
//! flat docket), the population, and the `state/` home. The generic driver
//! reads it and knows what to point at.

use rung_driver::Instance;

fn ws_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn the_het_sidecar_drives_the_q18_config() {
    let root = ws_root();
    let base = root.join(".het").join("rung-questions");

    // the config.yaml is the per-carrier state sidecar declaration
    let text = std::fs::read_to_string(base.join("config.yaml")).unwrap();
    let inst = Instance::from_yaml(&text).unwrap();
    assert_eq!(inst.theory, "rung-question");
    assert_eq!(inst.population.as_deref(), Some("../population.yaml"));
    assert_eq!(inst.state.as_deref(), Some("./state"));

    // carrier resolves against the config's own directory -> the flat docket
    let carrier = inst.build_carrier_at(&base).unwrap();
    let n = carrier.iter().filter_map(|r| r.ok()).count();
    assert!(
        n >= 19,
        "the flat docket is enumerated through the sidecar carrier"
    );

    // population + state resolve against the same base
    let pop = inst.population_path_at(&base).unwrap();
    assert!(
        pop.is_file(),
        "the shared population resolves: {}",
        pop.display()
    );
    let state = inst.state_dir_at(&base).unwrap();
    assert!(
        state.starts_with(&base),
        "state is inside the instance: {}",
        state.display()
    );
}
