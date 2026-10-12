//! Following the record door's `next` cursor costs work in proportion to the
//! record, not to pages times record; and the pages join with no gap and no
//! repeat, across segment boundaries and across a restart of the process.

use std::path::Path;

use rung_host::read::{RecordQuery, forget_record_index, record_lines_read, record_open};
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

#[test]
fn following_next_reads_lines_in_proportion_to_the_record() {
    // Work is lines read. The first page reads the record once; every later page
    // reads what it returns plus at most one seek stride, whatever the record's size.
    let (n, limit, stride) = (20_000u64, 50u64, 1024u64);
    let (t, reg) = world("work", n, 3000);
    forget_record_index(&t.path().join("state"));
    let mut pages = 0u64;
    let mut worst = 0u64;
    let mut offset = 0;
    let start = record_lines_read();
    let mut last = start;
    loop {
        let p = page(
            &reg,
            &RecordQuery {
                offset,
                limit,
                ..RecordQuery::default()
            },
        );
        let now = record_lines_read();
        if pages > 0 {
            worst = worst.max(now - last);
        }
        last = now;
        pages += 1;
        match p["next"].as_u64() {
            Some(n) => offset = n,
            None => break,
        }
    }
    assert_eq!(pages, n / limit);
    assert!(worst <= limit + stride, "a page read {worst} lines");
    assert!(last - start <= n + pages * (limit + stride));
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
            forget_record_index(&state); // the process restarts mid-read
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

#[test]
fn a_record_replaced_under_a_live_index_is_read_afresh() {
    let (t, reg) = world("replace", 2_000, 5_000);
    let state = t.path().join("state");
    let q = RecordQuery {
        offset: 1_900,
        limit: 10,
        ..RecordQuery::default()
    };
    assert_eq!(page(&reg, &q)["total"], 2_000);

    let seg = state.join("record/seg-00000000.ndjson");
    let tmp = state.join("record/replacement");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).unwrap();
        for seq in 1..=1_500u64 {
            let l = json!({"seq": seq, "at": seq, "kind": "note", "text": format!("replaced {seq} {}", "x".repeat(60))});
            writeln!(f, "{l}").unwrap();
        }
    }
    std::fs::rename(&tmp, &seg).unwrap();

    let p = page(&reg, &q);
    assert_eq!(p["total"], 1_500);
    assert!(p["damaged"].is_null(), "{}", p["damaged"]);
    let seqs: Vec<u64> = p["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["seq"].as_u64().unwrap())
        .collect();
    assert!(seqs.is_empty());

    let q = RecordQuery {
        offset: 1_400,
        limit: 10,
        ..RecordQuery::default()
    };
    let p = page(&reg, &q);
    assert_eq!(p["total"], 1_500);
    assert!(p["damaged"].is_null());
    assert!(
        p["lines"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("replaced 1401")
    );
    assert_eq!(p["next"], 1_410);
}
