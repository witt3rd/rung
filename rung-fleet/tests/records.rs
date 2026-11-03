//! The `crew.*` schema: every kind round-trips through a real record line, a
//! reader skips what it does not know, a malformed known kind is refused, a
//! host reads a record holding crew lines, and no other crate writes a kind.

use rung_fleet::brief::{Autonomy, Brief};
use rung_fleet::records::{CrewRecord, GROUP, RecordError};
use rung_host::record::Line;
use rung_testkit::TempDir;

fn brief() -> Brief {
    Brief::new("a plan", "reviewed", "a report", Autonomy::Decide).unwrap()
}

fn every_kind() -> Vec<CrewRecord> {
    let c = || "crew-1".to_string();
    vec![
        CrewRecord::Spawned {
            crew: c(),
            brief: brief(),
            assignee: "worker".into(),
        },
        CrewRecord::Assigned { crew: c() },
        CrewRecord::Active { crew: c() },
        CrewRecord::Held {
            crew: c(),
            key: "k1".into(),
        },
        CrewRecord::Resolved {
            crew: c(),
            key: "k1".into(),
        },
        CrewRecord::Stalled {
            crew: c(),
            stalls: 2,
        },
        CrewRecord::Recovered { crew: c() },
        CrewRecord::Done {
            crew: c(),
            summary: "finished".into(),
            validator: "steward".into(),
        },
        CrewRecord::Retired { crew: c() },
        CrewRecord::Receipt {
            crew: c(),
            correlation: "c-1".into(),
            seq: 812,
        },
    ]
}

#[test]
fn every_crew_kind_round_trips_through_a_record_line() {
    let all = every_kind();
    let kinds: Vec<&str> = all.iter().map(CrewRecord::kind).collect();
    assert_eq!(
        kinds,
        CrewRecord::KINDS,
        "the sample covers every kind, in order"
    );
    for (i, r) in all.iter().enumerate() {
        assert!(r.kind().starts_with(&format!("{GROUP}.")));
        let line = r.to_line(i as u64 + 1, 1_000);
        // Through the text a record holds, not just the in-memory line.
        let parsed = Line::parse(&line.text()).expect("a record line");
        assert_eq!(
            parsed.seq,
            i as u64 + 1,
            "the body does not shadow the line's seq"
        );
        assert_eq!(CrewRecord::from_line(&parsed).unwrap().as_ref(), Some(r));
    }
}

#[test]
fn a_reader_skips_what_it_does_not_know() {
    let future = Line::parse(r#"{"seq":1,"at":1,"kind":"crew.teleported","crew":"c"}"#).unwrap();
    assert_eq!(CrewRecord::from_line(&future), Ok(None));
    let other = Line::parse(r#"{"seq":2,"at":1,"kind":"host.start"}"#).unwrap();
    assert_eq!(CrewRecord::from_line(&other), Ok(None));
}

#[test]
fn a_known_kind_with_a_bad_field_is_refused() {
    let no_key = Line::parse(r#"{"seq":1,"at":1,"kind":"crew.held","crew":"c"}"#).unwrap();
    assert!(matches!(
        CrewRecord::from_line(&no_key),
        Err(RecordError::Field { field: "key", .. })
    ));
    let bad = Line::parse(
        r#"{"seq":1,"at":1,"kind":"crew.spawned","crew":"c","assignee":"w","objective":"o","done_when":"d","delivery":"x","autonomy":"anything"}"#,
    )
    .unwrap();
    assert!(matches!(
        CrewRecord::from_line(&bad),
        Err(RecordError::BadAutonomy(_))
    ));
    let empty = Line::parse(
        r#"{"seq":1,"at":1,"kind":"crew.spawned","crew":"c","assignee":"w","objective":" ","done_when":"d","delivery":"x","autonomy":"ask"}"#,
    )
    .unwrap();
    assert!(matches!(
        CrewRecord::from_line(&empty),
        Err(RecordError::BadBrief(_))
    ));
}

#[test]
fn a_host_reads_a_record_that_holds_crew_lines() {
    let root = TempDir::new("fleet-crew-kinds");
    let dir = root.path().join("record");
    std::fs::create_dir_all(&dir).unwrap();
    let text: String = every_kind()
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}\n", r.to_line(i as u64 + 1, 1_000).text()))
        .collect();
    std::fs::write(dir.join("seg-000000.ndjson"), text).unwrap();
    let mut seen = Vec::new();
    let scanned = rung_host::read::scan(root.path(), |l| {
        seen.push(CrewRecord::from_line(l).unwrap().unwrap())
    })
    .expect("the host's reader accepts crew lines");
    assert_eq!(scanned.total, 10);
    assert!(scanned.damaged.is_none());
    assert_eq!(seen, every_kind());
}

/// No crate but rung-fleet (and the documents that name the kinds) holds a
/// `"crew.<kind>"` string literal in source.
#[test]
fn no_other_crate_writes_a_crew_kind() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut hits = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !matches!(
                    name.as_str(),
                    "target" | ".git" | "node_modules" | "rung-fleet" | "rung-doctrine"
                ) {
                    stack.push(p);
                }
            } else if name.ends_with(".rs") {
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                if text.contains("\"crew.") {
                    hits.push(p.display().to_string());
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "crew kinds written outside rung-fleet: {hits:?}"
    );
}
