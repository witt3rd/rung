//! **Enact-to-carrier**: an enacted edit is persisted back to the question's
//! file — the writer half that makes the "real cycle" real. If enactment only
//! mutated the in-memory model, the driver could never repair the collection it
//! audits. These tests prove `persist` writes through the mechanism and leaves
//! the prose body untouched.

use rung_het::Applies;
use rung_std::questions::{Filing, QuestionEdit, Questions, Scheme};

const RUNG: Scheme = Scheme {
    namespace: "rung-questions",
    root: "questions",
    id_prefix: "q",
};

fn ws_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn copy_docket_to(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    for e in std::fs::read_dir(ws_root().join(".het/rung-questions/questions")).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            std::fs::copy(&p, dir.join(p.file_name().unwrap())).unwrap();
        }
    }
}

#[test]
fn persist_writes_the_enacted_edit_to_the_file_and_keeps_the_body() {
    let dir = std::path::PathBuf::from("/tmp/rung_wb_test_").join(std::process::id().to_string());
    let _ = std::fs::remove_dir_all(&dir);
    copy_docket_to(&dir);
    let mut world = Questions::load(RUNG, &dir);
    assert!(
        world.persist("q18").is_ok(),
        "persist requires a loaded carrier"
    );

    // the author enacts a REPAIR (Rewrite) in the model…
    let edit = QuestionEdit::Rewrite {
        answerable: "a single determinate, unique, stable, authentic fact".into(),
    };
    world.apply("q18", &edit).unwrap();
    // …and the driver persists it to the file
    world.persist("q18").ok();

    // verify the FILE reflects the edit (the observer reads the post-state back)
    let on_disk = std::fs::read_to_string(dir.join("q18-het-state-sidecar-convention.md")).unwrap();
    assert!(
        on_disk.contains("a single determinate, unique, stable, authentic fact"),
        "the enacted edit must be on disk"
    );
    assert!(
        on_disk.contains("## Two axis of flexibility"),
        "the prose body must survive the writeback"
    );

    // round-trip: reread the file into the model; the re-file/re-image reflects
    // the persisted edit
    let reread = Questions::load(RUNG, &dir);
    let q = reread.by_id("q18").unwrap();
    assert_eq!(
        q.answerable.as_deref(),
        Some("a single determinate, unique, stable, authentic fact")
    );
    assert_eq!(q.filing, Filing::WellPosed, "a Rewrite keeps Mode A");

    let _ = std::fs::remove_dir_all(&dir);
}
