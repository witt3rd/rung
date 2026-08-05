//! Write each encoded doctrine to the document it renders.
//!
//! The whole of this binary is: call `render`, save the string. The generation
//! itself lives in the library and is exercised by the round-trip test — this
//! only makes it a command rather than something a test does in memory.
//!
//! ```text
//! cargo run -p rung-doctrine --bin render          # write
//! cargo run -p rung-doctrine --bin render -- --check   # fail if stale
//! ```
//!
//! `--check` is what CI runs: the document on disk must be what the encoding
//! says it is. A repository where the two can differ has two sources of truth
//! and a habit of picking whichever was edited last.

use rung_doctrine::{Doctrine, Resolver, rung, rung_ct, rung_het};
use std::path::{Path, PathBuf};

fn docs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rung-doctrine sits in the workspace")
        .join("docs")
}

/// Slugs of documents not yet encoded, read from their prose.
///
/// Migration scaffolding: it reads the number a document *displays* rather than
/// deriving it. It shrinks as documents are encoded and disappears with the
/// last one.
fn externals(r: &mut Resolver, encoded: &[&str]) {
    for entry in std::fs::read_dir(docs()).expect("docs/ is readable") {
        let path = entry.expect("a directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with("-props.md") || encoded.contains(&name) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a governing document");
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(rest) = line.strip_prefix("<a id=\"") else {
                continue;
            };
            let Some((slug, _)) = rest.split_once('"') else {
                continue;
            };
            let Some(num) = lines
                .get(i + 1)
                .and_then(|l| l.strip_prefix("**"))
                .and_then(|l| l.split_once("**"))
                .map(|(n, _)| n)
            else {
                continue;
            };
            r.with_external(slug, name, num);
        }
    }
}

fn main() {
    let check = std::env::args().any(|a| a == "--check");
    // Declaration order fixes nothing here — numbering is per document.
    let doctrines: Vec<Doctrine> =
        vec![rung::doctrine(), rung_het::doctrine(), rung_ct::doctrine()];

    let encoded: Vec<&str> = doctrines.iter().map(|d| d.file.as_str()).collect();
    let mut resolver = Resolver::new();
    for d in &doctrines {
        resolver = resolver.with_doctrine(d);
    }
    externals(&mut resolver, &encoded);

    let mut stale = Vec::new();
    for d in &doctrines {
        let rendered = match d.render(&resolver) {
            Ok(text) => text,
            Err(errs) => {
                for e in errs {
                    eprintln!("  {e}");
                }
                std::process::exit(1);
            }
        };
        let path = docs().join(&d.file);
        write_or_report(&path, &rendered, check, &mut stale);
    }

    // The conformance record is a VIEW of the same doctrines — kind and proof
    // come from the encoding, so the join lives in one place.
    let root = docs().parent().expect("the workspace root").to_path_buf();
    let tests = rung_doctrine::workspace_tests(&root);
    let record = rung_doctrine::conformance::render(&doctrines, &resolver, &tests);
    write_or_report(&docs().join("conformance.md"), &record, check, &mut stale);

    if check && !stale.is_empty() {
        eprintln!(
            "\n{} document(s) differ from their encoding:\n{}\n\n\
             The encoding is the source. Run `cargo run -p rung-doctrine --bin render`.",
            stale.len(),
            stale.join("\n")
        );
        std::process::exit(1);
    }
}

fn write_or_report(path: &Path, rendered: &str, check: bool, stale: &mut Vec<String>) {
    let current = std::fs::read_to_string(path).unwrap_or_default();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

    if current == rendered {
        println!("  {name}: up to date");
        return;
    }
    if check {
        stale.push(format!("  {name}"));
        return;
    }
    std::fs::write(path, rendered).expect("the document is writable");
    println!("  {name}: written");
}
