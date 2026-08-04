//! The provenance floor is held by **coherence**, not by a check.
//!
//! `Principal` no longer has `Provenanced` as a supertrait, and the only route
//! from a principal to a provenance is the blanket impl in `rung`:
//!
//! ```text
//! impl<P: Principal + ?Sized> Provenanced for P {
//!     fn provenance(&self) -> Prov { self.authored().with(self.id()) }
//! }
//! ```
//!
//! So a principal cannot supply its own `π`. The intended diagnostic is
//! **E0119** — conflicting implementations — which is the trait solver refusing
//! the impl, not a runtime guard refusing a value. Anything else (E0277,
//! E0407, a parse error) would mean this file guards nothing.

use rung::{Principal, Prov, Provenanced, Response, Verdict};

struct Universal;

impl Principal for Universal {
    fn capable(&self, _role_name: &str) -> bool {
        true
    }
    fn id(&self) -> &str {
        "universal"
    }
    fn authored(&self) -> Prov {
        Prov::empty()
    }
    fn rule(&self, _matter: &str) -> Response {
        Response::Rendered(Verdict::Conforming)
    }
}

// The universal judge, attempted: empty provenance is disjoint from every
// argument in the workspace, so this principal would qualify to judge anything
// — including its own work. It is not a value the language will produce.
impl Provenanced for Universal {
    fn provenance(&self) -> Prov {
        Prov::empty()
    }
}

fn main() {}
