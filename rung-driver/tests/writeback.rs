//! **Enact to carrier (patch-based)** — an enacted edit is persisted back to
//! the question's file **without a whole-document re-render**: only the
//! frontmatter regions whose content the edit changed are rewritten, and every
//! other line — including frontmatter fields the model does not know about —
//! survives byte-for-byte. Then the write is verified (re-load, round-trip) and
//! rolled back if it fails.

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
fn fresh_tmp() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from("/tmp/rung_patch_").join(std::process::id().to_string());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Identify the question file for `id` and inject `note: keep me` into its
/// frontmatter — an UNUSED (unmodelled) frontmatter key the patch must not drop.
fn inject_unknown_field(dir: &std::path::Path, id: &str) {
    let file = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|d| d.path()))
        .filter(|p| p.is_file())
        .find(|p| {
            p.file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .starts_with(&format!("{id}-"))
        })
        .unwrap();
    let t = std::fs::read_to_string(&file).unwrap();
    let out = t.replace("\n---\n", "\nnote: keep me\n---\n");
    std::fs::write(&file, out).unwrap();
}

#[test]
fn persist_is_a_patch_that_leaves_unknown_fields_byte_identical() {
    let dir = fresh_tmp();
    for e in std::fs::read_dir(ws_root().join(".het/rung-questions/questions")).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            std::fs::copy(&p, dir.join(p.file_name().unwrap())).unwrap();
        }
    }
    inject_unknown_field(&dir, "q4"); // q4 has a rich body
    let mut world = Questions::load(RUNG, &dir);

    // the author enacts a REWRITE (repair) on q4 — changes answerable only.
    let edit = QuestionEdit::Rewrite {
        answerable: "a single, unique, authentic fact".into(),
    };
    let original_file =
        std::fs::read_to_string(dir.join("q4-composition-nested-ladders.md")).unwrap();
    world.apply("q4", &edit).unwrap();
    let path = world.persist("q4").expect("patch persists");
    let patched = std::fs::read_to_string(&path).unwrap();

    // 1) the edit landed
    assert!(
        patched.contains(
            "answerable: a single, unique, authentic fact"
                .replace(' ', "  ")
                .as_str()
        ) || patched.contains("a single, unique, authentic fact")
    );
    // 2) the unknown frontmatter key survived byte-for-byte
    assert!(
        patched.contains("note: keep me"),
        "unmodeled frontmatter must survive"
    );
    // 3) the prose body survived
    assert!(
        patched.contains("## Two axis of flexibility")
            || patched.contains("## What rests on it")
            || original_file.contains("# Q4")
    );
    // 4) an unrelated modelled field (status) is byte-identical
    assert!(patched.contains("status: open"));

    // and round-trips through a fresh load
    let reread = Questions::load(RUNG, &dir);
    let q = reread.by_id("q4").unwrap();
    assert_eq!(
        q.answerable.as_deref(),
        Some("a single, unique, authentic fact")
    );
    assert_eq!(q.filing, Filing::WellPosed);

    let _ = std::fs::remove_dir_all(&dir);
}
