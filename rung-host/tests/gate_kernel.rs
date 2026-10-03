//! G-c (the free-time kernel) and its refusals; G-k over the run.

mod common;

use common::*;
use rung_host::clock::{DAY, HOUR};
use rung_host::gates;
use rung_host::kernel::KernelState;
use rung_host::record::Line;
use rung_host::registers::Registers;
use rung_host::render;
use rung_host::sim::{self, Rng, SIM_START};
use rung_host::state::State;

#[test]
fn two_thousand_turns_follow_the_kernel() {
    sim::test_timeout(600);
    let mut sc = scenario("gate-c", 23);
    sc.max_turns = Some(gates::G_C_TURNS);
    sc.world = busy_world(23, DAY);
    let out = sim::run(sc);
    assert_gate(&gates::g_c(&out.lines));
    assert_gate(&gates::g_k(&out.lines));
    // The agent's free-time material, rebuilt at the end, under random scores.
    let st = State::replay(&out.lines);
    material_order_holds(&st.registers, &st.kernel);
}

fn material_order_holds(reg: &Registers, kernel: &KernelState) {
    let now = SIM_START + 8 * HOUR;
    let base = render::material(reg, kernel, now);
    let mut rng = Rng::new(99);
    let mut same = 0;
    for _ in 0..gates::G_C_SCORE_PERMUTATIONS {
        let mut r = reg.clone();
        for t in r.todo.values_mut() {
            t.priority = Some(rng.f64() * 100.0);
        }
        for p in r.projects.values_mut() {
            p.priority = Some(rng.f64() * 100.0);
        }
        for q in r.questions.values_mut() {
            q.priority = Some(rng.f64() * 100.0);
        }
        if render::material(&r, kernel, now) == base {
            same += 1;
        }
    }
    eprintln!(
        "GATE G-c/no-ranking {} of {} permutations identical",
        same,
        gates::G_C_SCORE_PERMUTATIONS
    );
    assert_eq!(same, gates::G_C_SCORE_PERMUTATIONS);
}

#[test]
fn material_is_unranked_on_a_rich_register() {
    // A register with many entries, so ordering could matter.
    let mut lines = Vec::new();
    let mut seq = 0;
    let mut push = |kind: &str, body: serde_json::Value, at: i64| {
        seq += 1;
        let serde_json::Value::Object(m) = body else {
            panic!()
        };
        lines.push(Line {
            seq,
            at,
            kind: kind.into(),
            body: m,
        });
    };
    for k in 0..12 {
        push(
            "todo.added",
            serde_json::json!({"id": format!("t{k}"), "text": format!("todo {k}")}),
            SIM_START + k * 1000,
        );
        push(
            "project.added",
            serde_json::json!({"id": format!("p{k}"), "title": format!("project {k}"), "why": "w", "status": "active"}),
            SIM_START + k * 1000,
        );
        push(
            "question.added",
            serde_json::json!({"id": format!("q{k}"), "text": format!("question {k}")}),
            SIM_START + k * 1000,
        );
    }
    let st = State::replay(&lines);
    material_order_holds(&st.registers, &st.kernel);
}

#[test]
fn no_path_outside_the_kernel_writes_its_lines() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
