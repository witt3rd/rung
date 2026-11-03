//! `rung-fleet`: a tiered fleet of rung hosts (`docs/rung-fleet-props.md`).
//!
//! This crate is a product crate (J2). It adds the delegation the host
//! reserves and changes nothing in the kernel or in `rung-host`.

pub mod brief;
pub mod lifecycle;

pub use brief::{Autonomy, Brief, BriefError};
