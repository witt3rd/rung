//! **HetOpt** — the worth-law extension of Het (rung-het-props.md §8).
//!
//! Het settles belonging. HetOpt orders what belongs (het-settles-hetopt-orders):
//!
//! ```text
//! Het    = judgmental institution + gate-marked ⊨ + metric verdict space
//! HetOpt = Het + V
//! ```
//!
//! The cut is drawn at **valuation itself** (cut-at-valuation), and it lands
//! exactly here in the code:
//!
//! - **no earlier** (cut-at-valuation): the kernel `Principal` interface keeps
//!   exactly four predicates (nothing-further-required). Cost tier and ε live
//!   on the [`Priced`] extension trait, which Het-generic code does not see —
//!   a `fn f<P: Principal>` cannot read a cost, and the compile-fail doctests
//!   below pin that.
//! - **no later** (non-identity-not-deferrable): every method here runs Het's
//!   filters *first* and optimizes only among the survivors
//!   (filter-then-optimize). Nothing in this module can mint a [`Qualified`]
//!   token; [`Pool`] remains the only mint, so no worth-ordering can admit a
//!   principal the filters refused.
//!
//! HetOpt is a theory extension in the ordinary sense (hetopt-is-a-theory-extension):
//! [`ValuedPool`] **re-indexes** an existing [`Pool`] rather than forking the
//! machinery, so there is one pool and two filters still (one-pool-two-filters)
//! — plus, now, one order.
//!
//! ## The cut, as compile errors
//!
//! ### Het-generic code cannot read a cost
//!
//! cut-at-valuation, no-later direction: the worth side is not in the
//! `Principal` interface, so a function written against Het has no term for
//! preferring among qualifying judges (no-preference-among-judges). If this
//! ever compiles, the cut has drifted.
//!
//! ```compile_fail
//! use rung::{Pool, Principal, Prov, Response, Verdict};
//! pub struct P;
//! impl Principal for P {
//!     fn capable(&self, _: &str) -> bool { true }
//!     fn id(&self) -> &str { "p" }
//!     fn authored(&self) -> Prov { Prov::of(["p"]) }
//!     fn rule(&self, _: &str) -> Response { Response::Rendered(Verdict::Conforming) }
//! }
//! // `Principal` has no `cost_tier` — E0599, and nothing else.
//! fn cost_invisible<P: Principal>(p: &P) -> u64 {
//!     p.cost_tier()
//! }
//! # fn main() {}
//! ```
//!
//! ### A principal without a worth declaration cannot be re-indexed
//!
//! Sign_HetOpt extends Sign_Het with the declaration of V
//! (hetopt-is-a-theory-extension): declaring worth is the extension's content,
//! so a pool of principals that never declared it has no image in the HetOpt
//! fiber. If this ever compiles, worth is being manufactured.
//!
//! ```compile_fail
//! use rung::{Pool, Principal, Prov, Response, Verdict};
//! use rung_het::hetopt::ValuedPool;
//! pub struct Undeclared;
//! impl Principal for Undeclared {
//!     fn capable(&self, _: &str) -> bool { true }
//!     fn id(&self) -> &str { "u" }
//!     fn authored(&self) -> Prov { Prov::of(["u"]) }
//!     fn rule(&self, _: &str) -> Response { Response::Rendered(Verdict::Conforming) }
//! }
//! // `Undeclared: Priced` does not hold — E0277.
//! fn no_image(p: Pool<Undeclared>) {
//!     let _ = ValuedPool::reindexed(p);
//! }
//! # fn main() {}
//! ```
//!
//! ## The three valuations
//!
//! V applies wherever Het has produced a conforming set (v-applies-to-conforming-sets).
//! Het produces three, so this module instantiates valuation three times —
//! one piece of machinery, three uses:
//!
//! | Het produces | ordered by | method |
//! |---|---|---|
//! | qualifying judges for a sentence | cost tier, then ε | [`ValuedPool::minimal_judge`] / [`ValuedPool::judges_by_worth`] |
//! | qualifying authors for an operation | cost tier | [`ValuedPool::authors_by_cost`] |
//! | the conforming algebras of a theory | the declared worth-law | [`rank`] |
//!
//! ## Scope, stated honestly
//!
//! Worth here is a **total order** (`Ord`), not a quantale. A quantale's
//! monoidal composition (`⊗`) has no earned use in this corpus yet — a ranking
//! is what every named consumer needs. The general enrichment
//! (enrichment-base-is-the-metric) is the general form; this is the fragment
//! the current theories exercise, and no more.
//!
//! ```rust
//! use rung_het::hetopt::{Priced, ValuedPool};
//! use rung_het::{Pool, Principal, Prov, Provenanced, Response, Role, Verdict};
//!
//! pub struct SoulDoc { chars: usize, author: &'static str }
//! impl Provenanced for SoulDoc {
//!     fn provenance(&self) -> Prov { Prov::of([self.author]) }
//! }
//!
//! // A principal that declares its worth — the Sign_HetOpt extension (8.4).
//! #[derive(Clone, Copy)]
//! pub struct Judge { name: &'static str, tier: u64, eps: f64 }
//! impl Principal for Judge {
//!     fn capable(&self, _: &str) -> bool { true }
//!     fn id(&self) -> &str { self.name }
//!     fn authored(&self) -> Prov { Prov::of([self.name]) }
//!     fn rule(&self, _: &str) -> Response { Response::Rendered(Verdict::Conforming) }
//! }
//! impl Priced for Judge {
//!     fn cost_tier(&self) -> u64 { self.tier }
//!     fn epsilon(&self) -> f64 { self.eps }
//! }
//!
//! # fn main() {
//! let pool = Pool::new(vec![
//!     Judge { name: "cheap", tier: 1, eps: 0.2 },
//!     Judge { name: "fine", tier: 0, eps: 0.5 },
//! ]);
//! let valued = ValuedPool::reindexed(pool);
//!
//! let doc = SoulDoc { chars: 10, author: "somebody-else" };
//! // The minimal-judge rule: cheapest qualifying judge, ties by lowest ε.
//! let judge = valued.minimal_judge::<ChordReader>(&doc).unwrap();
//! assert_eq!(judge.principal_id(), "fine");
//! # }
//!
//! #[derive(Clone, Copy)]
//! pub struct ChordReader;
//! impl Role for ChordReader { const NAME: &'static str = "chord-reader"; }
//! ```

use rung::{
    AuthorizeError, Authorized, Pool, Principal, Provenanced, Qualified, QualifyError, Role,
    Steward,
};

/// The worth side of a principal — **the Sign extension** (hetopt-is-a-theory-extension).
///
/// Het's supplier interface is exactly four predicates and nothing further
/// (nothing-further-required); worth is declared here, one axis over, so that
/// Het-generic code has no term for it (het-declares-no-worth-law) while a
/// HetOpt theory must declare it before a principal can be pooled at all.
///
/// Both fields support ordering among those that qualify (ordering-is-hetopts):
///
/// - `cost_tier` orders principals by resource consumption. Lower is cheaper;
///   the minimal-judge and minimal-author rules take the cheapest survivor.
/// - `epsilon` is the error bar Het requires be declared (3.32) and never
///   reads as a preference. HetOpt reads it — but only as the **tie-break**,
///   after cost tier.
pub trait Priced: Principal {
    /// Ordering on principals by resource consumption. Lower is cheaper.
    fn cost_tier(&self) -> u64;

    /// The declared error bar on this principal's verdicts.
    fn epsilon(&self) -> f64;
}

/// The declared worth-law `V` over a theory's conforming candidates
/// (v-applies-to-conforming-sets, third row).
///
/// The theory implements this to say what it prefers; [`rank`] applies it.
/// The order is the preference: greater worth is more preferred. That is the
/// fragment of a quantale the corpus exercises — the order part. The monoidal
/// composition stays out until a theory needs it (see the module scope note).
pub trait WorthLaw<C> {
    /// The worth of a candidate. `Ord` is the ranking.
    type Worth: Ord;
    fn worth(&self, candidate: &C) -> Self::Worth;
}

/// Order a conforming set by the declared worth-law — most preferred first.
///
/// Filter first, then optimize (filter-then-optimize): this function touches
/// no filter at all, because its input is already a conforming set — the thing
/// Het produced (v-applies-to-conforming-sets). Ranking it is exactly and only
/// HetOpt's move.
///
/// The sort is **stable**: candidates of equal worth keep their order, which
/// makes the result order-stable in the content of the set rather than in the
/// list the caller happened to type.
pub fn rank<C, V: WorthLaw<C>>(worth: &V, candidates: Vec<C>) -> Vec<C> {
    let mut ranked = candidates;
    ranked.sort_by_key(|a| std::cmp::Reverse(worth.worth(a)));
    ranked
}

/// A [`Pool`] re-indexed into the HetOpt fiber (hetopt-is-a-theory-extension).
///
/// The extension carries Het-algebras into the HetOpt fiber **by re-indexing**:
/// this wraps the pool, it does not rebuild or fork it. The filters are Het's,
/// run through the same [`Pool`] methods the judgmental surface uses, so a
/// worth-ordering cannot admit anyone the filters refused and non-identity
/// cannot drift to this layer (non-identity-not-deferrable).
pub struct ValuedPool<P: Priced> {
    pool: Pool<P>,
}

impl<P: Priced> ValuedPool<P> {
    /// Re-index a Het pool into HetOpt. No machinery is copied.
    pub fn reindexed(pool: Pool<P>) -> Self {
        Self { pool }
    }

    /// Back to Het: the same pool, worth forgotten. The inclusion runs both
    /// ways because the cut is a declaration, not a copy (cut-at-valuation) —
    /// forgetting the order loses nothing Het ever had.
    pub fn pool(&self) -> &Pool<P> {
        &self.pool
    }

    /// The **minimal-judge rule** (v-applies-to-conforming-sets, first row).
    ///
    /// ```text
    /// qualifying = { p : capable(p, role) ∧ π(p) ∩ π(a) = ∅ }   ← Het
    /// argmin over qualifying by (cost tier, then ε)             ← HetOpt
    /// ```
    ///
    /// The substitution of *argmin* for *any* is precisely the seam where
    /// HetOpt lands (10.23): Het's [`Pool::qualify_for`] returns any survivor
    /// because no-preference-among-judges forbids it a preference; HetOpt is
    /// the preference, declared. The filter still runs whole and first —
    /// non-identity is not deferrable (non-identity-not-deferrable), so a
    /// cheap judge who authored the argument is refused here exactly as
    /// anywhere else.
    pub fn minimal_judge<R: Role>(
        &self,
        argument: &dyn Provenanced,
    ) -> Result<Qualified<R>, QualifyError> {
        let panel = self.pool.qualifying::<R>(argument)?;
        self.argmin(panel)
    }

    /// The qualifying set, **in worth order** — cheapest first.
    ///
    /// This is the seam Het never opens and HetOpt does
    /// (no-preference-after-a-deferral): walking on to the next qualifying
    /// judge after one raised a matter is a preference among qualifying
    /// judges, which Het is forbidden to have and a theory holding a
    /// [`ValuedPool`] is licensed to exercise. The order is the theory's to
    /// walk; the pool still mints every token, and every token still carries
    /// its own argument.
    pub fn judges_by_worth<R: Role>(
        &self,
        argument: &dyn Provenanced,
    ) -> Result<Vec<Qualified<R>>, QualifyError> {
        let mut panel = self.pool.qualifying::<R>(argument)?;
        self.order(&mut panel);
        Ok(panel)
    }

    /// The **minimal-author rule** (v-applies-to-conforming-sets, second row)
    /// — qualifying authors in cost order, cheapest first.
    ///
    /// Standing-escalation happens **before** any valuation
    /// (standing-escalation-precedes-valuation): the standing + competence
    /// filter runs whole and first, and this function returns the survivors,
    /// not a verdict on who closes. The caller walks the order and escalates
    /// when the minimal author cannot close (two-escalation-triggers, second
    /// row) — a worth-ordering says escalate; it does not close for anyone.
    ///
    /// ε is deliberately unread here: the minimal-author rule orders by cost
    /// tier alone (v-applies-to-conforming-sets). An author transforms rather
    /// than classifies, so an error bar on its verdicts is not a thing this
    /// rule has a use for.
    pub fn authors_by_cost<'a, R: Role, S: Priced + Steward>(
        &self,
        stewards: &'a [S],
        over: &'a str,
    ) -> Result<Vec<Authorized<'a, R>>, AuthorizeError> {
        // Filter first (standing-escalation-precedes-valuation): capable ∧
        // standing, through the same [`Pool::authorize`] the authorial gate
        // uses — one pool, two filters, no third conjunct invented here.
        let mut pens = Vec::new();
        for s in stewards {
            if let Ok(pen) = self.pool.authorize::<R, S>(s, over) {
                pens.push((s.cost_tier(), pen));
            }
        }
        // Order-stable on cost tier only; equal-cost authors keep pool order.
        pens.sort_by_key(|(tier, _)| *tier);
        Ok(pens.into_iter().map(|(_, pen)| pen).collect())
    }

    /// argmin by (cost tier, then ε), stable.
    fn argmin<R: Role>(&self, panel: Vec<Qualified<R>>) -> Result<Qualified<R>, QualifyError> {
        let mut best: Option<(u64, f64, Qualified<R>)> = None;
        for token in panel {
            let p = self
                .pool
                .principal(token.principal_id())
                .ok_or(QualifyError::PoolExhausted { considered: 0 })?;
            let key = (p.cost_tier(), p.epsilon());
            let take = match &best {
                None => true,
                Some((c, e, _)) => key.0 < *c || (key.0 == *c && lt(key.1, *e)),
            };
            if take {
                best = Some((key.0, key.1, token));
            }
        }
        best.map(|(_, _, t)| t)
            .ok_or(QualifyError::PoolExhausted { considered: 0 })
    }

    /// In-place worth order on a panel: cost tier, then ε; stable.
    fn order<R: Role>(&self, panel: &mut [Qualified<R>]) {
        panel.sort_by(|a, b| {
            let (pa, pb) = (
                self.pool.principal(a.principal_id()),
                self.pool.principal(b.principal_id()),
            );
            match (pa, pb) {
                (Some(pa), Some(pb)) => pa.cost_tier().cmp(&pb.cost_tier()).then(
                    pa.epsilon().partial_cmp(&pb.epsilon()).unwrap_or(
                        // NaN sorts last: an undeclared error bar never wins.
                        if pa.epsilon().is_nan() {
                            std::cmp::Ordering::Greater
                        } else {
                            std::cmp::Ordering::Less
                        },
                    ),
                ),
                // A token whose principal is not in the pool cannot arise
                // (the pool minted it); keep pool order on the impossible arm.
                _ => std::cmp::Ordering::Equal,
            }
        });
    }
}

fn lt(a: f64, b: f64) -> bool {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Greater) == std::cmp::Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;
    use rung::{Prov, Response, Verdict};

    #[derive(Clone, Copy)]
    struct Reader;
    impl Role for Reader {
        const NAME: &'static str = "reader";
    }

    #[derive(Clone, Copy)]
    struct Keeper;
    impl Role for Keeper {
        const NAME: &'static str = "keeper";
    }

    struct Doc {
        author: &'static str,
    }
    impl Provenanced for Doc {
        fn provenance(&self) -> Prov {
            Prov::of([self.author])
        }
    }

    #[derive(Clone, Copy)]
    struct Judge {
        name: &'static str,
        tier: u64,
        eps: f64,
    }
    impl Principal for Judge {
        fn capable(&self, _: &str) -> bool {
            true
        }
        fn id(&self) -> &str {
            self.name
        }
        fn authored(&self) -> Prov {
            Prov::of([self.name])
        }
        fn rule(&self, _: &str) -> Response {
            Response::Rendered(Verdict::Conforming)
        }
    }
    impl Priced for Judge {
        fn cost_tier(&self) -> u64 {
            self.tier
        }
        fn epsilon(&self) -> f64 {
            self.eps
        }
    }

    fn pool() -> Pool<Judge> {
        Pool::new(vec![
            Judge {
                name: "cheap",
                tier: 1,
                eps: 0.2,
            },
            Judge {
                name: "fine",
                tier: 0,
                eps: 0.5,
            },
            Judge {
                name: "sloppy",
                tier: 1,
                eps: 0.9,
            },
        ])
    }

    #[test]
    fn minimal_judge_is_cheapest_then_lowest_epsilon() {
        let valued = ValuedPool::reindexed(pool());
        let doc = Doc { author: "author" };
        let j = valued.minimal_judge::<Reader>(&doc).unwrap();
        assert_eq!(j.principal_id(), "fine");
    }

    #[test]
    fn epsilon_breaks_cost_ties_only() {
        let valued = ValuedPool::reindexed(pool());
        let doc = Doc { author: "author" };
        let ordered: Vec<String> = valued
            .judges_by_worth::<Reader>(&doc)
            .unwrap()
            .iter()
            .map(|t| t.principal_id().to_string())
            .collect();
        assert_eq!(ordered, vec!["fine", "cheap", "sloppy"]);
    }

    #[test]
    fn a_cheap_judge_who_authored_the_argument_is_still_refused() {
        let valued = ValuedPool::reindexed(pool());
        let doc = Doc { author: "fine" };
        // "fine" is tier 0 — the cheapest in the pool — and still out:
        // the filter runs whole and first (non-identity-not-deferrable).
        // The argmin lands on the cheapest *survivor*, never on "fine".
        let j = valued.minimal_judge::<Reader>(&doc).unwrap();
        assert_eq!(j.principal_id(), "cheap");
        let ordered: Vec<String> = valued
            .judges_by_worth::<Reader>(&doc)
            .unwrap()
            .iter()
            .map(|t| t.principal_id().to_string())
            .collect();
        assert!(!ordered.contains(&"fine".to_string()));
    }

    #[test]
    fn standing_precedes_valuation() {
        // Authorial: standing + competence filter first (3.67), then cost.
        #[derive(Clone, Copy)]
        struct StewardJudge {
            name: &'static str,
            tier: u64,
            containers: &'static [&'static str],
        }
        impl Principal for StewardJudge {
            fn capable(&self, _: &str) -> bool {
                true
            }
            fn id(&self) -> &str {
                self.name
            }
            fn authored(&self) -> Prov {
                Prov::of([self.name])
            }
            fn rule(&self, _: &str) -> Response {
                Response::Rendered(Verdict::Conforming)
            }
        }
        impl Priced for StewardJudge {
            fn cost_tier(&self) -> u64 {
                self.tier
            }
            fn epsilon(&self) -> f64 {
                0.1
            }
        }
        impl Steward for StewardJudge {
            fn has_standing(&self, over: &str) -> bool {
                self.containers.contains(&over)
            }
        }

        let stewards = vec![
            StewardJudge {
                name: "cheap-outsider",
                tier: 0,
                containers: &["elsewhere"],
            },
            StewardJudge {
                name: "costly-steward",
                tier: 5,
                containers: &["repo"],
            },
        ];
        let valued = ValuedPool::reindexed(pool());
        let pens = valued
            .authors_by_cost::<Keeper, StewardJudge>(&stewards, "repo")
            .unwrap();
        // The tier-0 principal has no standing over "repo" — the cheapest
        // survivor is the tier-5 steward, not the cheapest principal.
        assert_eq!(pens.len(), 1);
        assert_eq!(pens[0].principal_id(), "costly-steward");
    }

    #[test]
    fn rank_is_most_preferred_first_and_stable() {
        struct Sum;
        impl WorthLaw<u64> for Sum {
            type Worth = u64;
            fn worth(&self, c: &u64) -> u64 {
                *c
            }
        }
        let ranked = rank(&Sum, vec![1, 9, 9, 4]);
        assert_eq!(ranked, vec![9, 9, 4, 1]);
    }

    #[test]
    fn reindexing_round_trips_without_copying_the_pool() {
        let pool = pool();
        let valued = ValuedPool::reindexed(pool);
        assert_eq!(valued.pool().len(), 3);
    }
}
