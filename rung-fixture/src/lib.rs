//! A ladder declared here, consumed from `tests/` — which Rust compiles as a
//! **separate crate** linking this one. That is the boundary
//! `cross-crate-provenance` is about.
//!
//! ## What survives the crossing
//!
//! `G2` seals rung construction to the emitted module, and that half crosses
//! intact: a consumer crate cannot build a mid-ladder rung. `tests/ui/` pins it.
//!
//! ## What does not
//!
//! **Whether the token it receives was produced from anything real.** The seal
//! is a module boundary *inside this crate*, so this crate can mint an entry
//! rung from an argument no caller supplied and drive it forward legitimately.
//! What comes out the other side is a well-formed `Active` carrying a payload,
//! and the payload says nothing about where it came from.
//!
//! [`Order`] is the demonstration: it carries a private `invented` flag that
//! only this crate can set, and the flag **is not carried into the receipt**.
//! By the time a consumer sees anything, the distinction is gone — not hidden,
//! *absent*.
//!
//! Closing this needs the sealed types emitted into a sub-crate the macro alone
//! controls, so even this crate could not mint the entry rung. One crate per
//! ladder, which is why it is parked as Q2 rather than done.

use rung::ladder;

/// What a caller asks for.
///
/// `invented` is private, so only this crate can produce an order nobody
/// placed. A downstream crate has [`Order::placed`] and nothing else.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub units: u32,
    invented: bool,
}

impl Order {
    /// A real order, from a caller who asked for it.
    pub fn placed(units: u32) -> Self {
        Self {
            units,
            invented: false,
        }
    }

    /// Whether this order was invented. Readable **here** and nowhere else,
    /// which is the whole point: the consumer has no such accessor.
    fn is_invented(&self) -> bool {
        self.invented
    }
}

/// What a run claims to have produced.
///
/// It carries units and no provenance. That is not an oversight — it is what
/// the proposition says: a token crossing a crate boundary is trusted.
#[derive(Clone, Debug, PartialEq)]
pub struct Receipt {
    pub processed: u32,
}

ladder!(Work {
    Placed(Order) => Active(Receipt) => { Settled(Receipt) }
} impl {
    // An honest body. It derives the receipt from the order it consumed, and
    // still cannot carry across what it knows: `is_invented` is a fact about
    // the order, and the receipt has nowhere to put it.
    active = |placed| {
        let order = placed.payload;
        let _ = order.is_invented();
        Active::new(Receipt { processed: order.units })
    },
    step = |active| { Ok(StepOutcome::Settled(Settled::new(active.payload))) },
});

/// A token derived from an order a caller actually placed.
pub fn traversed(units: u32) -> work::Active {
    work::active(work::Placed::new(Order::placed(units)))
}

/// A token derived from an order **this crate invented**.
///
/// Every arrow ran. Nothing was fabricated in `G2`'s sense. The lie is upstream
/// of the ladder, in an entry rung minted from an argument no caller supplied —
/// and only this crate can supply it, because `invented` is private.
pub fn from_an_invented_order(units: u32) -> work::Active {
    work::active(work::Placed::new(Order {
        units,
        invented: true,
    }))
}
