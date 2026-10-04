//! G-m: the decision desk. Every family returns a choice on every path with
//! the right provenance; the guards hold under adversarial answers; a
//! stalled decider costs at most 2 s per boundary; a `Recorded` replay
//! panics on a reworded question. G-k over the host run.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use rung_host::clock::MINUTE;
use rung_host::desk::admit::{Admit, AdmitCtx, AdmitInput, AdmitItem, Form, Waiting};
use rung_host::desk::consolidate::{CandidateView, Consolidate, ConsolidateInput};
use rung_host::desk::inject::{Cue, Inject, InjectCtx, InjectInput};
use rung_host::desk::pack::{Action, Pack, PackInput, Segment};
use rung_host::desk::tools::{Tools, ToolsCtx, ToolsInput};
use rung_host::desk::{By, DecisionDesk, DeskMode, HostQuestion, Scripted, SpendCap, Step, Why};
use rung_host::gates;
use rung_host::inbox::{ItemKind, Role};
use rung_host::kernel::TurnKind;
use rung_host::sim::{self, DeskSpec, SIM_START};
use rung_std::decide::{Ask, DEFAULT_MODEL, Decider, Recorded, Undecided};
use serde_json::{Value, json};

const NOW: i64 = SIM_START + 60 * MINUTE;

fn admit_case(committed: bool) -> (AdmitInput, AdmitCtx) {
    let w = |id: &str, kind, role, at, due, firm| Waiting {
        id: id.into(),
        kind,
        role,
        at,
        due,
        firm,
    };
    let waiting = vec![
        w("o1", ItemKind::Peer, Role::Owner, NOW - 5_000, None, false),
        w("p1", ItemKind::Peer, Role::Peer, NOW - 60_000, None, false),
        w(
            "c1",
            ItemKind::Calendar,
            Role::Host,
            NOW - 1_000,
            Some(NOW - 1_000),
            true,
        ),
        w(
            "e1",
            ItemKind::Expectation,
            Role::Host,
            NOW - 2_000,
            None,
            false,
        ),
        // Waited past the peer maximum deferral.
        w(
            "p2",
            ItemKind::Peer,
            Role::Peer,
            NOW - 31 * MINUTE,
            None,
            false,
        ),
    ];
    let items = waiting
        .iter()
        .filter(|x| x.role != Role::Owner)
        .map(|x| AdmitItem {
            id: x.id.clone(),
            kind: x.kind,
            role: x.role,
            age_s: (NOW - x.at) / 1000,
            due_in_s: x.due.map(|d| (d - NOW) / 1000),
            declared_urgency: None,
            gist: format!("item {}", x.id),
        })
        .collect();
    let input = AdmitInput {
        now: NOW,
        mode: if committed {
            json!({"committed": {"project": "p1"}})
        } else {
            json!({"free": {"session_turns": 2}})
        },
        items,
        interrupts_last_hour: 0,
        since_external_s: 5,
    };
    let ctx = AdmitCtx {
        now: NOW,
        waiting,
        at_break: false,
        committed,
    };
    (input, ctx)
}

fn inject_case(memory: &str) -> (InjectInput, InjectCtx) {
    (
        InjectInput {
            focus: vec!["owner asks about the calendar".into()],
            free_session_turns: 1,
            turns_since_recall: Some(4),
            recall_hits_last: 1,
            memory: memory.into(),
            expectations_due_1h: 1,
            calendar_within_2h: 0,
            note_age_turns: Some(9),
        },
        InjectCtx {
            kind: TurnKind::Responding,
            first_committed: false,
            turn: 30,
        },
    )
}

fn tools_case() -> (ToolsInput, ToolsCtx) {
    (
        ToolsInput {
            kind: TurnKind::Free,
            enabled: vec!["core".into(), "read".into()],
            ceiling: vec!["core".into(), "memory".into(), "read".into()],
            wants: vec![("web_read".into(), "to look something up".into())],
            uses_last_10: BTreeMap::new(),
            turns_since_switch: BTreeMap::from([("read".into(), 9), ("memory".into(), 9)]),
        },
        ToolsCtx {
            kind: TurnKind::Free,
            turn: 30,
            wanted_recently: BTreeSet::from(["web_read".to_string()]),
        },
    )
}

fn pack_case(fraction: f64, copy_flag: bool) -> PackInput {
    let budget = 10_000;
    let seg = |i: u64, r: bool| Segment {
        id: format!("s{i}"),
        first_turn: i * 3,
        last_turn: i * 3 + 2,
        tokens: 900,
        gist: format!("segment {i}"),
        referenced_by_open_commitment: r,
        referenced_by_open_expectation: false,
    };
    PackInput {
        epoch_tokens: (fraction * budget as f64) as usize,
        header_reserve: 300,
        budget,
        turns_in_epoch: 12,
        at_break: true,
        mode: "free".into(),
        cache_read_ratio_last10: 0.9,
        copy_flag,
        segments: vec![seg(1, true), seg(2, false), seg(3, false), seg(4, false)],
        last_turn: 14,
    }
}

fn consolidate_case() -> ConsolidateInput {
    ConsolidateInput {
        note_age_turns: Some(12),
        commits: 2,
        releases: 1,
        candidates: vec![
            CandidateView {
                id: "c10".into(),
                kind: "trace".into(),
                gist: "a trace".into(),
            },
            CandidateView {
                id: "c11".into(),
                kind: "settled".into(),
                gist: "e3 met".into(),
            },
        ],
        rollover_imminent: true,
    }
}

fn desk(step: Step, mode: DeskMode) -> DecisionDesk {
    DecisionDesk::new(Some(Arc::new(Scripted::always(step))), "scripted", mode)
}

/// One family through one desk: (choice, by, elapsed).
fn run<Q: HostQuestion>(
    d: &DecisionDesk,
    input: &Q::Input,
    ctx: &Q::Ctx,
) -> (Q::Choice, By, Duration, Value) {
    let t = Instant::now();
    let state = serde_json::to_value(input).unwrap();
    let asked = d.ask(state, Q::questions(input), 0.0);
    let dec = d.decide::<Q>(input, ctx, &asked, 1, 1);
    (
        dec.choice().clone(),
        dec.by().clone(),
        t.elapsed(),
        dec.line().clone(),
    )
}

fn undecided_variants() -> Vec<Undecided> {
    vec![
        Undecided::Unreachable("down".into()),
        Undecided::RateLimited,
        Undecided::Overloaded,
        Undecided::Unauthorized("no key".into()),
        Undecided::NoCredit,
        Undecided::Invalid("bad".into()),
        Undecided::TooLarge,
        Undecided::Malformed("short".into()),
        Undecided::Unavailable("stub".into()),
    ]
}

/// Every path for one family; `guard` checks the bound on every choice.
fn every_path<Q: HostQuestion>(
    input: &Q::Input,
    ctx: &Q::Ctx,
    guard: &dyn Fn(&Q::Choice),
) -> usize {
    let mut paths = 0;
    let mut check = |d: &DecisionDesk, want: &dyn Fn(&By) -> bool, label: &str| {
        let (c, by, took, line) = run::<Q>(d, input, ctx);
        assert!(want(&by), "{} {label}: by {by:?}", Q::ID);
        assert!(line["by"].is_object(), "{} {label}: no provenance", Q::ID);
        assert!(
            took <= Duration::from_millis(gates::G_M_BOUNDARY_COST_MS as u64),
            "{} {label}: {took:?}",
            Q::ID
        );
        guard(&c);
        paths += 1;
    };
    let uniform = Step::Uniform {
        p: 0.9,
        pick: "now".into(),
    };
    check(
        &desk(uniform.clone(), DeskMode::Decide),
        &|b| matches!(b, By::Jev { backend, .. } if backend == "scripted"),
        "answered",
    );
    for u in undecided_variants() {
        let label = u.to_string();
        let want = Why::Undecided(label.clone());
        check(
            &desk(Step::Undecided(u), DeskMode::Decide),
            &|b| *b == By::Rule(want.clone()),
            &label,
        );
    }
    check(
        &desk(
            Step::Delay(gates::G_M_DELAY_MS, Box::new(uniform.clone())),
            DeskMode::Decide,
        ),
        &|b| *b == By::Rule(Why::Timeout),
        "delay",
    );
    let mut capped = desk(uniform.clone(), DeskMode::Decide);
    capped.cap = SpendCap {
        per_day: 0.0,
        per_ask: 0.001,
    };
    check(&capped, &|b| *b == By::Rule(Why::Capped), "capped");
    check(
        &DecisionDesk::new(None, "none", DeskMode::Decide),
        &|b| *b == By::Rule(Why::NoDecider),
        "no decider",
    );
    check(
        &desk(uniform.clone(), DeskMode::RuleOnly),
        &|b| *b == By::Rule(Why::RuleOnly),
        "rule only",
    );
    check(
        &desk(Step::Answers(BTreeMap::new()), DeskMode::Decide),
        &|b| *b == By::Rule(Why::Incomplete),
        "incomplete",
    );
    let (_, by, _, line) = run::<Q>(&desk(uniform, DeskMode::Shadow), input, ctx);
    assert_eq!(by, By::Rule(Why::Shadow));
    assert!(
        line.get("jev_choice").is_some() && line.get("agree").is_some(),
        "{} shadow logs the decider",
        Q::ID
    );
    paths + 1
}

#[test]
fn every_family_decides_on_every_path_and_its_guards_hold() {
    sim::test_timeout(1800);
    let mut paths = 0;
    let (ai, ac) = admit_case(true);
    paths += every_path::<Admit>(&ai, &ac, &|c| {
        assert_eq!(c.forms["o1"], Form::Now, "the owner is never deferred");
        assert_eq!(c.forms["c1"], Form::Now, "a firm due item is shown");
        assert_eq!(c.forms["p2"], Form::Now, "past its maximum deferral");
    });
    let (ii, ic) = inject_case("off");
    paths += every_path::<Inject>(&ii, &ic, &|c| {
        assert!(!c.recall && c.cue == Cue::None, "no memory, no recall")
    });
    let (ti, tc) = tools_case();
    paths += every_path::<Tools>(&ti, &tc, &|c| {
        assert!(c.enabled.contains(&"core".to_string()));
        assert!(
            c.enabled.iter().all(|g| ti.ceiling.contains(g)),
            "outside the ceiling: {:?}",
            c.enabled
        );
    });
    let low = pack_case(0.2, false);
    paths += every_path::<Pack>(&low, &(), &|c| {
        assert_eq!(c.action, Action::Append, "below the floor")
    });
    let high = pack_case(0.9, false);
    paths += every_path::<Pack>(&high, &(), &|c| {
        assert_eq!(c.action, Action::Rollover, "past the ceiling");
        let kept: usize = high
            .segments
            .iter()
            .filter(|s| c.keep.contains(&s.id))
            .map(|s| s.tokens)
            .sum();
        assert!(kept as f64 <= 0.15 * high.budget as f64, "kept {kept}");
    });
    let copy = pack_case(0.2, true);
    paths += every_path::<Pack>(&copy, &(), &|c| {
        assert_eq!(
            (c.action, c.cause.as_str()),
            (Action::Rollover, "copy_loop")
        );
    });
    let ci = consolidate_case();
    paths += every_path::<Consolidate>(&ci, &(), &|c| {
        assert!(
            c.retain
                .iter()
                .all(|id| ci.candidates.iter().any(|x| &x.id == id))
        );
    });
    eprintln!("GATE G-m/paths PASS {{\"family_paths\":{paths}}}");
}

#[test]
fn adversarial_answers_stay_inside_the_guards() {
    // Interrupt nothing, enable everything, roll over below the floor.
    let (ai, ac) = admit_case(true);
    let d = desk(
        Step::Uniform {
            p: 0.0,
            pick: "at_break".into(),
        },
        DeskMode::Decide,
    );
    let (c, _, _, _) = run::<Admit>(&d, &ai, &ac);
    assert_eq!((c.forms["o1"], c.forms["c1"]), (Form::Now, Form::Now));
    let mut answers = BTreeMap::new();
    answers.insert(
        "enable_web_read".to_string(),
        rung_std::decide::Answer::Noul { p: 1.0 },
    );
    answers.insert(
        "enable_memory".to_string(),
        rung_std::decide::Answer::Noul { p: 1.0 },
    );
    answers.insert(
        "enable_read".to_string(),
        rung_std::decide::Answer::Noul { p: 1.0 },
    );
    let (ti, tc) = tools_case();
    let (c, by, _, _) = run::<Tools>(&desk(Step::Answers(answers), DeskMode::Decide), &ti, &tc);
    assert!(matches!(by, By::Jev { .. }));
    assert!(!c.enabled.contains(&"web_read".to_string()));
    let low = pack_case(0.3, false);
    let (c, _, _, _) = run::<Pack>(
        &desk(
            Step::Uniform {
                p: 1.0,
                pick: "rollover_now".into(),
            },
            DeskMode::Decide,
        ),
        &low,
        &(),
    );
    assert_eq!(c.action, Action::Append);
}

// ─── Recorded fixtures ───────────────────────────────────────────────────────

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/decide")
}

/// The scenarios the design names, each one boundary or rollover ask.
fn recorded_asks() -> Vec<(&'static str, Ask)> {
    let boundary = |committed: bool| {
        let (ai, _) = admit_case(committed);
        let (ii, _) = inject_case("baseline");
        let (ti, _) = tools_case();
        let mut q = Admit::questions(&ai);
        q.extend(Inject::questions(&ii));
        q.extend(Tools::questions(&ti));
        Ask {
            state: json!({"admit": ai, "inject": ii, "tools": ti}),
            questions: q,
        }
    };
    let mut burst = boundary(false);
    if let Value::Object(m) = &mut burst.state {
        m.insert("note".into(), json!("a burst of eight peer items"));
    }
    let rollover = {
        let p = pack_case(0.65, false);
        let c = consolidate_case();
        let mut q = Pack::questions(&p);
        q.extend(Consolidate::questions(&c));
        Ask {
            state: json!({"pack": p, "consolidate": c}),
            questions: q,
        }
    };
    vec![
        ("admit/peer-during-commit", boundary(true)),
        ("admit/calendar-due-in-free", boundary(false)),
        ("admit/burst-of-8", burst),
        ("pack/rollover-at-break", rollover),
    ]
}

/// A synthetic System One response: every question answered, marked as
/// authored (no live Jev was asked in this slice).
fn synthetic_response(ask: &Ask) -> Value {
    let d = Scripted::always(Step::Seeded(17)).decide(ask).unwrap();
    let mut answers = serde_json::Map::new();
    for (id, a) in &d.answers {
        let v = match a {
            rung_std::decide::Answer::Noul { p } => json!({"type": "noul", "noul": p}),
            rung_std::decide::Answer::Choice {
                choice,
                probabilities,
                confidence,
            } => {
                json!({"type": "choice", "choice": choice, "probabilities": probabilities, "confidence": confidence})
            }
        };
        answers.insert(id.clone(), v);
    }
    json!({"model": "synthetic (authored, not recorded)", "answers": answers, "usage": {"input_tokens": ask.estimated_tokens(), "cost": 0.0}})
}

#[test]
fn recorded_fixtures_replay_and_a_reworded_question_panics() {
    let write = std::env::var("RUNG_HOST_WRITE_FIXTURES").is_ok();
    for (name, ask) in recorded_asks() {
        let path = fixtures().join(format!("{name}.json"));
        if write {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let f = json!({"request": ask.body(DEFAULT_MODEL), "response": synthetic_response(&ask),
                           "model": "synthetic", "recorded_at": "synthetic"});
            std::fs::write(&path, serde_json::to_string_pretty(&f).unwrap() + "\n").unwrap();
        }
        let d = DecisionDesk::new(
            Some(Arc::new(Recorded::replay(&path))),
            "recorded",
            DeskMode::Decide,
        );
        let asked = d.ask(ask.state.clone(), ask.questions.clone(), 0.0);
        assert!(asked.result.is_ok(), "{name}: {:?}", asked.result.err());
        // Reword one question: the replay must refuse loudly.
        let mut reworded = ask.clone();
        let (id, q) = reworded.questions.iter_mut().next().unwrap();
        let (rung_std::decide::Question::Noul { instructions, .. }
        | rung_std::decide::Question::Choice { instructions, .. }) = q;
        instructions.push_str(" (reworded)");
        let id = id.clone();
        let rec = Recorded::replay(&path);
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rec.decide(&reworded)));
        let msg = err.expect_err("a reworded question must panic");
        let text = msg.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(
            text.contains("records a different request"),
            "{name} ({id}): {text}"
        );
    }
    eprintln!(
        "GATE G-m/recorded PASS {{\"fixtures\":{}}}",
        recorded_asks().len()
    );
}

// ─── The host run ────────────────────────────────────────────────────────────

#[test]
fn a_host_on_an_adversarial_decider_keeps_its_guards() {
    sim::test_timeout(1800);
    let mut sc = scenario("gate-m", 31);
    sc.max_turns = Some(800);
    sc.world = busy_world(31, 6 * 3_600_000);
    sc.config.epoch_budget_tokens = 12_000;
    sc.config.ceiling = ["core", "memory", "read", "workspace_write"]
        .into_iter()
        .map(String::from)
        .collect();
    let policy = Scripted::policy(|_ask: &Ask, n: u64| {
        let adversarial = if n.is_multiple_of(2) {
            Step::Uniform {
                p: 1.0,
                pick: "rollover_now".into(),
            }
        } else {
            Step::Uniform {
                p: 0.0,
                pick: "at_break".into(),
            }
        };
        match n % 300 {
            0 => Step::Delay(gates::G_M_DELAY_MS, Box::new(adversarial)),
            7 => Step::Undecided(Undecided::RateLimited),
            _ => adversarial,
        }
    });
    sc.desk = DeskSpec::Decider {
        decider: Arc::new(policy),
        backend: "scripted".into(),
        mode: DeskMode::Decide,
    };
    let out = sim::run(sc);
    let ceiling = ["core", "memory", "read", "workspace_write"];
    assert_gate(&gates::g_m(&out.lines, &ceiling));
    assert_gate(&gates::g_k(&out.lines));
}
