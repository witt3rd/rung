use rung_host::sim::{self, Scenario};

#[test]
#[ignore]
fn probe() {
    sim::test_timeout(1200);
    let dir = sim::temp_dir("probe");
    let mut sc = Scenario::new(&dir, 11);
    sc.max_turns = Some(10_000);
    sc.config.epoch_budget_tokens = 24_000;
    sc.memory = std::env::var("PROBE_MEM").is_ok();
    let t = std::time::Instant::now();
    let out = sim::run(sc);
    eprintln!("10000 turns: {:?}, lines {}", t.elapsed(), out.lines.len());
    let t = std::time::Instant::now();
    let g = rung_host::gates::g_l(&out.lines, &out.captured);
    eprintln!("{}  {:?}", g.summary(), g.failures);
    let g = rung_host::gates::g_j(&out.lines);
    eprintln!("{}  {:?}", g.summary(), g.failures);
    eprintln!("gates {:?}", t.elapsed());
}
