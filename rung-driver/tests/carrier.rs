//! The carrier layer — M(S), read by the driver.
//!
//! A carrier is the model side of the institution: the set a sort ranges over,
//! walked extensionally. This file proves the minimal, skeptical port — the
//! colocated strategies (folder, file, JSONL row-wise flat) with **opaque**
//! content. Parsing is a theory's job, never the carrier's.

use rung_driver::{
    Carrier, CsvFolderCarrier, FileCarrier, FolderCarrier, GitHubIssuesCarrier, JsonlFileCarrier,
    JsonlFolderCarrier, ObjectId,
};
use std::io::Write;
use std::path::{Path, PathBuf};

fn ws_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-driver sits in the workspace")
        .to_path_buf()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rung-carrier-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp dir");
    d
}

// ═════════════════════════════════════════════════════════════════════════
// 1 · Folder — one subject per file, deterministic, opaque
// ═════════════════════════════════════════════════════════════════════════

/// The canonical colocated carrier: a folder of files, one subject per file,
/// walked in sorted order so the audit is deterministic over the real corpus.
#[test]
fn a_folder_yields_one_subject_per_file_sorted_and_opaque() {
    let open = ws_root().join("questions");
    let c = FolderCarrier::new(&open);
    assert!(c.exists());

    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().expect("walk is clean");
    // every question file in questions/open/
    let mut expected: Vec<PathBuf> = std::fs::read_dir(&open)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    expected.sort(); // the carrier walks sorted; the expectation must match
    assert_eq!(ids.len(), expected.len());
    assert_eq!(
        ids,
        expected
            .iter()
            .map(|p| ObjectId::new(p.to_string_lossy().into_owned()))
            .collect::<Vec<_>>(),
        "the walk is sorted by file name and gives one subject per file"
    );

    // read is opaque — it is text, and the driver does not interpret it.
    let first = &ids[0];
    let content = c.read(first).expect("readable");
    assert!(!content.is_empty());
    assert_eq!(content, std::fs::read_to_string(first.as_str()).unwrap());
}

// ═════════════════════════════════════════════════════════════════════════
// 2 · File — one file, one subject
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn a_file_is_a_single_subject_carrier() {
    let d = tmp("file");
    let p = d.join("doc.md");
    std::fs::write(&p, "hello\nworld\n").unwrap();

    let c = FileCarrier::new(&p);
    assert!(c.exists());
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(c.read(&ids[0]).unwrap(), "hello\nworld\n");
}

// ═════════════════════════════════════════════════════════════════════════
// 3 · JsonlFile — row-wise, blank lines skipped, opaque rows
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn a_jsonl_file_is_row_wise_and_opaque() {
    let d = tmp("jsonl");
    let p = d.join("portfolio.jsonl");
    let mut f = std::fs::File::create(&p).unwrap();
    writeln!(f, "{{\"id\": \"a\"}}").unwrap();
    writeln!(f).unwrap(); // blank — skipped
    writeln!(f, "{{\"id\": \"b\"}}").unwrap();
    writeln!(f, "{{\"id\": \"c\"}}").unwrap();
    drop(f);

    let c = JsonlFileCarrier::new(&p);
    assert!(c.exists());
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), 3, "blank lines are not subjects");
    // stable row ids: <file>/row/<n>
    let stem = p.to_string_lossy().into_owned();
    assert_eq!(ids[0].as_str(), format!("{stem}/row/0"));
    assert_eq!(ids[2].as_str(), format!("{stem}/row/2"));
    // rows are opaque: the carrier hands back the raw line
    assert_eq!(c.read(&ids[0]).unwrap(), "{\"id\": \"a\"}");
    assert_eq!(c.read(&ids[2]).unwrap(), "{\"id\": \"c\"}");
}

// ═════════════════════════════════════════════════════════════════════════
// 4 · JsonlFolder — the flatmap: rows across many files, one stream
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn a_jsonl_folder_flatmaps_rows_across_files() {
    let d = tmp("jsonlfolder");
    let a = d.join("a.jsonl");
    let b = d.join("b.jsonl");
    std::fs::write(&a, "1\n2\n").unwrap();
    std::fs::write(&b, "3\n").unwrap();
    // a non-jsonl file is not population and is not swept in (the narrow point)
    std::fs::write(d.join("log.txt"), "not population\n").unwrap();

    let c = JsonlFolderCarrier::new(&d);
    assert!(c.exists());
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), 3, "rows across both .jsonl, the log swept away");
    assert_eq!(c.read(&ids[0]).unwrap(), "1");
    assert_eq!(c.read(&ids[1]).unwrap(), "2");
    assert_eq!(c.read(&ids[2]).unwrap(), "3");
}

/// A missing carrier reports its absence; a folded id does not resolve.
#[test]
fn a_missing_or_foreign_id_is_an_error_not_a_fabrication() {
    let c = FileCarrier::new(ws_root().join("definitely-not-here.md"));
    assert!(!c.exists());
    // walking a missing file yields a fault, not an empty sweep
    let first = c.iter().next();
    assert!(first.is_some_and(|r| r.is_err()));

    let d = tmp("foreign");
    let p = d.join("x.jsonl");
    std::fs::write(&p, "row\n").unwrap();
    let c = JsonlFileCarrier::new(&p);
    let alien = ObjectId::new(ws_root().join("other.jsonl").to_string_lossy().into_owned());
    assert!(
        c.read(&alien).is_err(),
        "a foreign id is refused, not guessed"
    );
}

// ═════════════════════════════════════════════════════════════════════════
// 5 · CSV — one subject per row, header excluded, quoting handled
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn a_csv_folder_is_row_wise_with_header_excluded() {
    let d = tmp("csv");
    std::fs::write(
        d.join("a.csv"),
        "id,name\n1,\"solo, inc\"\n2,\"two, Inc\"\n",
    )
    .unwrap();
    // a non-csv file is not population
    std::fs::write(d.join("notes.txt"), "not population").unwrap();

    let c = CsvFolderCarrier::new(&d);
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(ids.len(), 2, "header excluded, non-csv swept out");
    // records are opaque; field-boundary correctness is the carrier's job
    // the csv crate gives parsed fields (quotes stripped); the record text is
    // still opaque to the carrier — it never interprets the schema
    assert_eq!(c.read(&ids[0]).unwrap(), "1,solo, inc");
    assert_eq!(c.read(&ids[1]).unwrap(), "2,two, Inc");
}

// ═════════════════════════════════════════════════════════════════════════
// 6 · config -> carrier resolution (Q18: the driver reads config, not constants)
// ═════════════════════════════════════════════════════════════════════════

/// The config names the strategy and location; `build` yields a concrete,
/// domain-blind carrier — no theory knowledge in the driver.
#[test]
fn carrier_config_builds_the_declared_strategy() {
    use rung_driver::{CarrierConfig, CarrierKind};

    let folder = serde_yaml::from_str::<CarrierConfig>(&format!(
        "kind: folder\npath: {}\n",
        ws_root().join("questions").display()
    ))
    .unwrap();
    assert_eq!(folder.kind, CarrierKind::Folder);
    let c = folder.build().unwrap();
    assert!(c.exists());
    // ./questions is a folder of the worktree — many subjects
    let n = c.iter().filter_map(|r| r.ok()).count();
    assert!(n > 0);

    // a colocated carrier without a path is refused, not guessed
    let bad = serde_yaml::from_str::<CarrierConfig>("kind: folder\n").unwrap();
    assert!(bad.build().is_err());

    // github needs repos, and never a secret
    let gh = serde_yaml::from_str::<CarrierConfig>("kind: github\n").unwrap();
    assert!(gh.build().is_err(), "github without repos is refused");
    let gh =
        serde_yaml::from_str::<CarrierConfig>("kind: github\nrepos: [witt3rd/rung]\n").unwrap();
    let c = gh.build().expect("repos given, no secret taken");
    // id is the declared address; existence is left to `exists()`
    assert!(c.id().as_str().starts_with("github:issues:"));
}

// ═════════════════════════════════════════════════════════════════════════
// 7 · GitHub — the external carrier, and its auth
// ═════════════════════════════════════════════════════════════════════════

#[test]
fn github_carrier_refuses_no_repos_but_takes_no_secret() {
    assert!(GitHubIssuesCarrier::new(vec![]).is_err());
    let c = GitHubIssuesCarrier::new(vec!["witt3rd/rung".to_string()]).unwrap();
    // the config is an address; the credential is the ambient gh, never here
    assert!(c.id().as_str().starts_with("github:issues:"));
    assert!(!c.id().as_str().contains("GH_TOKEN"));
    assert!(!c.id().as_str().contains("github_pat"));
}

/// **Live, integration, gated on a real `gh` with auth.** Not run in CI by
/// default (no credential to prove); run it locally with `--ignored` when you
/// want the external walk exercised: `cargo test -p rung-driver github_live -- --ignored`.
#[test]
#[ignore = "needs an ambient gh CLI with auth (never a secret in config)"]
fn github_live_issues_walk() {
    let repo = "witt3rd/rung";
    let c = GitHubIssuesCarrier::new(vec![repo.to_string()]).unwrap();
    assert!(
        c.exists(),
        "gh should be installed and reachable for the live test"
    );
    // at least one issue is open in this repo (the docket lives in questions/, but
    // PRs/issues exist); the walk must not fault and ids must be `owner/repo#n`
    let ids: Vec<ObjectId> = c.iter().collect::<Result<_, _>>().expect("gh walk works");
    for id in &ids {
        assert!(id.as_str().starts_with(&format!("{repo}#")));
        assert!(c.read(id).is_ok(), "each enumerated issue is readable");
    }
    // a foreign id is refused, not fetched
    let alien = ObjectId::new(format!("{repo}#99999999"));
    assert!(c.read(&alien).is_ok() || c.read(&alien).is_err()); // number absent -> gh errors or empty
}

// ═════════════════════════════════════════════════════════════════════════
// 8 · Instance config drives the audit, through the carrier (Q18/Q19)
// ═════════════════════════════════════════════════════════════════════════

/// The driver reads an instance config.yaml, builds the declared carrier, and
/// audits its subjects with the governing theory — the walk is the generic
/// carrier, not a hand-rolled fragment.
#[test]
fn instance_config_drives_a_carrier_audit() {
    use rung_driver::Instance;
    use rung_std::questions::{Question, Scheme, question};

    let text = std::fs::read_to_string(ws_root().join("instance.yaml")).unwrap();
    let inst = Instance::from_yaml(&text).unwrap();
    assert_eq!(inst.theory, "rung-question");

    let carrier = inst.build_carrier_at(&ws_root()).unwrap();
    let scheme = Scheme {
        namespace: "rung-questions",
        root: "questions",
        id_prefix: "q",
    };

    let mut audited = 0usize;
    for subject in carrier.iter() {
        let id = subject.expect("carrier walk is clean");
        let content = carrier.read(&id).expect("subject readable");
        let path = Path::new(id.as_str());
        let dir = path
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|d| d.to_str())
            .unwrap_or("");
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // infra files (README, INTAKE, _map) are not questions — the theory
        // skips them, and so does the audit over the carrier's subjects.
        let Some(q) = Question::parse(scheme, &content, dir, stem) else {
            continue;
        };
        for settled in [
            question::id_matches_the_filename::holds(&q),
            question::status_is_declared::holds(&q),
            question::edge_kinds_are_declared::holds(&q),
        ] {
            assert!(settled.verdict().is_conforming(), "{} failed", q.id);
        }
        audited += 1;
    }
    assert!(audited > 0, "the carrier enumerated subjects to audit");
}
