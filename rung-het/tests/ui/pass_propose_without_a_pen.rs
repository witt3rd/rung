//! `propose` is **authorial** (`propose-is-authorial`), so it cannot be called
//! without an `Authorized` pen.
//!
//! The intended diagnostic is **E0061** — "this function takes 2 arguments but
//! 1 argument was supplied". Anything else (a parse error, E0433, E0601) would
//! mean this file guards nothing.

use rung_het::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub complete: bool,
}

impl Provenanced for Draft {
    fn provenance(&self) -> Prov {
        Prov::of(["drafter"])
    }
}

#[derive(Clone, Copy)]
pub struct Editor;
impl Role for Editor {
    const NAME: &'static str = "editor";
}

#[derive(Clone, Copy)]
pub struct Reader;
impl Role for Reader {
    const NAME: &'static str = "reader";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftEdit {
    Finish,
}

het_pass!(Pass {
    subject = Draft,
    edit = DraftEdit,
    author = Editor,
    judge = Reader,
} impl {
    audit = |d: &Draft| Verdict::conforming(d.complete, "unfinished"),
    propose = |_chain: &Chain, _author: &str| Answer::Remedy(DraftEdit::Finish),
    rule = |_p: &Proposal<DraftEdit>, _judge: &str| Disposition::Accept,
});

fn main() {
    let entry = pass::Governed::new(
        Draft { complete: false },
        pass::Carry {
            subject_id: "d1".to_string(),
            container: "folio".to_string(),
        },
    );
    let proposing = pass::proposing(pass::audited(entry));
    let _proposed = pass::proposed(proposing);
}
