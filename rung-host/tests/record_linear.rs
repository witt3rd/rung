//! Following the record door's `next` cursor costs work in proportion to the
//! record, not to pages times record; and the pages join with no gap and no
//! repeat, across segment boundaries and across a restart of the process.

use std::path::Path;
use std::time::Instant;

use rung_host::read::{RecordQuery, forget_record_indexes, record_open};
use rung_host::redact::Redactor;
use rung_testkit::TempDir;
use serde_json::{Value, json};

/// A registry and a state directory whose record is `n` lines spread over
/// segments of `per_seg` lines.
fn world(tag: &str, n: u64, per_seg: u64) -> (TempDir, std::path::PathBuf) {
    let t = TempDir::new(tag);
    let state = t.path().join("state");
    let reg = t.path().join("reg");
    std::fs::create_dir_all(state.join("record")).unwrap();
    std::fs::create_dir_all(&reg).unwrap();
    std::fs::write(
        reg.join("inst.json"),
        json!({
            "schema": 1, "id": "inst", "name": "inst", "state_dir": state,
            "config": null, "workspace": state, "address": null,
            "pid": 1, "started": 1, "version": "0",
        })
        .to_string(),
    )
    .unwrap();
    append(&state, 1, n, per_seg);
    (t, reg)
}

/// Append lines `from..=to`, a new segment every `per_seg` lines.
fn append(state: &Path, from: u64, to: u64, per_seg: u64) {
    use std::io::Write;
    for seq in from..=to {
        let seg = (seq - 1) / per_seg;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(state.join(format!("record/seg-{seg:08}.ndjson")))
            .unwrap();
        let l = json!({"seq": seq, "at": seq, "kind": "note", "text": format!("line {seq}")});
        writeln!(f, "{l}").unwrap();
    }
}

fn page(reg: &Path, q: &RecordQuery) -> Value {
    let mut out = Vec::new();
    record_open(reg, "inst", q, &Redactor::default())
        .unwrap()
        .write_json(&mut out)
        .unwrap();
    serde_json::from_slice(&out).unwrap()
}

/// Follow `next` to the end; the seqs read, in order.
fn follow(reg: &Path, limit: u64, mut on_page: impl FnMut(u64)) -> Vec<u64> {
    let mut seqs = Vec::new();
    let mut offset = 0;
    loop {
        let p = page(
            reg,
            &RecordQuery {
                offset,
                limit,
                ..RecordQuery::default()
            },
        );
        seqs.extend(
            p["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l["seq"].as_u64().unwrap()),
        );
        on_page(seqs.len() as u64);
        match p["next"].as_u64() {
            Some(n) => offset = n,
            None => return seqs,
        }
    }
}

fn timed(tag: &str, n: u64) -> f64 {
    let (_t, reg) = world(tag, n, 3000);
    forget_record_indexes();
    let t0 = Instant::now();
    let got = follow(&reg, 50, |_| {});
    let secs = t0.elapsed().as_secs_f64();
    assert_eq!(got.len() as u64, n);
    secs
}

#[test]
fn following_next_costs_work_in_proportion_to_the_record() {
    // Linear: 4x the record is about 4x the time. The old door scanned the whole
    // record for every page, so 4x the record was about 16x the time.
    let best = |n| {
        (0..3)
            .map(|i| timed(&format!("lin-{n}-{i}"), n))
            .fold(f64::MAX, f64::min)
    };
    let (small, big) = (best(5_000), best(20_000));
    let ratio = big / small;
    assert!(
        ratio < 8.0,
        "4x the record took {ratio:.1}x the time ({small:.3}s -> {big:.3}s); linear is about 4x"
    );
}

#[test]
fn pages_join_with_no_gap_and_no_repeat_across_segments_and_a_restart() {
    let n = 3_500;
    let (t, reg) = world("exact", n, 700);
    let state = t.path().join("state");
    let mut restarted = false;
    let got = follow(&reg, 333, |read| {
        if read >= 1_500 && !restarted {
            restarted = true;
            forget_record_indexes(); // the process restarts mid-read
        }
    });
    assert!(restarted);
    assert_eq!(got, (1..=n).collect::<Vec<_>>());

    // the same through the cursor, with lines appended between pages
    let mut seen = Vec::new();
    let mut after = 0;
    let mut total = n;
    loop {
        let p = page(
            &reg,
            &RecordQuery {
                after: Some(after),
                limit: 400,
                ..RecordQuery::default()
            },
        );
        seen.extend(
            p["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l["seq"].as_u64().unwrap()),
        );
        assert_eq!(p["total"].as_u64().unwrap(), total);
        let Some(c) = p["cursor"].as_u64() else {
            panic!()
        };
        after = c;
        if p["next"].is_null() {
            if total >= n + 900 {
                break;
            }
            append(&state, total + 1, total + 450, 700);
            total += 450;
        }
    }
    assert_eq!(seen, (1..=total).collect::<Vec<_>>());

    // newest first reads the same lines, backwards
    let d = page(
        &reg,
        &RecordQuery {
            offset: 10,
            limit: 5,
            desc: true,
            ..RecordQuery::default()
        },
    );
    let ds: Vec<u64> = d["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(ds, (0..5).map(|i| total - 10 - i).collect::<Vec<_>>());
}

#[test]
fn damage_is_reported_and_stays_where_it_was_found() {
    let (t, reg) = world("damage", 2_100, 700);
    let state = t.path().join("state");
    let q = RecordQuery {
        offset: 2_000,
        limit: 10,
        ..RecordQuery::default()
    };
    assert!(page(&reg, &q)["damaged"].is_null());
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(state.join("record/seg-00000002.ndjson"))
            .unwrap();
        writeln!(f, "not json").unwrap();
    }
    append(&state, 2_101, 2_200, 700);
    let p = page(&reg, &q);
    assert_eq!(p["total"], 2_100);
    assert_eq!(p["damaged"]["after_seq"], 2_100);
}
