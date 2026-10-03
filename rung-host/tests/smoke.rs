use rung_host::sim::{self, Scenario};

#[test]
fn a_short_quiet_run() {
    sim::test_timeout(120);
    let dir = sim::temp_dir("smoke");
    let mut sc = Scenario::new(&dir, 7);
    sc.max_turns = Some(200);
    let out = sim::run(sc);
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for l in &out.lines {
        *kinds.entry(l.kind.clone()).or_default() += 1;
    }
    eprintln!("{kinds:#?}");
    eprintln!("why: {:?}", out.why);
    for l in out.lines.iter().filter(|l| l.kind == "turn.log").take(3) {
        eprintln!("{}", l.str("header"));
    }
}
