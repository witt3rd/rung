//! `rung-fleet`: a tiered fleet of rung hosts (`docs/rung-fleet-props.md`).
//!
//! This crate is a product crate (J2). It adds the delegation the host
//! reserves and changes nothing in the kernel or in `rung-host`.

#![deny(unused_must_use)]

pub mod authority;
pub mod brief;
pub mod delivery;
pub mod landing;
pub mod lifecycle;
pub mod records;
pub mod spawn;
pub mod steer;
pub mod tree;
pub mod verbs;

pub use brief::{Autonomy, Brief, BriefError};
