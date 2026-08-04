//! `Proposing` carries **classification only**.
//!
//! Its payload is built inline by `step` — that is, by the judge (rung-props.md
//! G10). If it could carry proposal content, the judge would be authoring, which
//! `disposition-is-a-ruling` and `no-amending-disposition` forbid. `Chain` is a
//! concrete, non-generic record of prose and counts, so there is no accessor to
//! read an edit off it and no type parameter one could be hidden in.
//!
//! The intended diagnostic is **E0599** — no method named `edit`. Anything else
//! would mean this file guards nothing.

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
    let chain: &Chain = &proposing.payload;
    let _edit: &DraftEdit = chain.edit();
}
