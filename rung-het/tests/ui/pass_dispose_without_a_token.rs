//! `dispose` is **judgmental**, so it cannot be called without a `Qualified`
//! licence. There is no term for "settle this without consulting anyone."
//!
//! The intended diagnostic is **E0061** — "this function takes 2 arguments but
//! 1 argument was supplied". Anything else (a parse error, E0433, E0601) would
//! mean this file guards nothing. Everything up to the final line is a
//! well-formed run, so that the only error is the missing licence.

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

pub struct Person;
impl Principal for Person {
    fn capable(&self, role_name: &str) -> bool {
        role_name == "editor"
    }
    fn id(&self) -> &str {
        "editor"
    }

    /// `authored` — the history this principal claims. `π(p)` is this
    /// **with `id()` added**, by the blanket `Provenanced` impl in `rung`:
    /// the provenance floor is not a value a principal gets to state.
    fn authored(&self) -> Prov {
        Prov::of(["editor"])
    }

    /// The oracle. The verdict is the outside's, not the caller's.
    fn rule(&self, _matter: &str) -> Verdict {
        Verdict::Conforming
    }
}
impl Steward for Person {
    fn has_standing(&self, over: &str) -> bool {
        over == "folio"
    }
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

    let pool = Pool::new(vec![Person]);
    let pen = pool.authorize::<Editor, _>(&Person, "folio").unwrap();
    let proposed = pass::proposed(proposing, pen);

    let _outcome = pass::step(proposed);
}
