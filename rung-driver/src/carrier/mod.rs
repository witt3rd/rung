//! Carriers — the **model side** of the institution, read by the driver.
//!
//! A theory declares a sort; a carrier supplies the set that sort ranges over
//! (`M(S)`). This module is how the driver reads that set: enumerate the
//! inhabitants ([`Carrier::iter`]), fetch one by id ([`Carrier::read`]).
//!
//! ```text
//!   theory!  ->  sorts, edits, sentences, roles      (what is governed)
//!   carrier  ->  M(S): the concrete body of subjects  (what is audited)
//! ```
//!
//! ## Strategy, not storage
//!
//! A carrier is a *strategy* — how a pile of bytes becomes a set of subjects —
//! selected by an instance's `config.yaml` (see Q18). The backends are the
//! colocated ones: one file, a folder of files, a JSONL stream (row-wise and
//! flat). An external carrier (GitHub issues) is the same strategy with a
//! different resolution, added the same way.
//!
//! Row content is **opaque**: the carrier walks and reads bytes, and nothing
//! else. Parsing is the theory's job, and the engine never interprets a
//! subject's content. That is what keeps a carrier domain-blind and lets the
//! same `Carrier` back a questions folder and a portfolio JSONL.
//!
//! ## What is deliberately absent
//!
//! Ported with skepticism from the archived het-rs prototype: no schema
//! validation (`jsonschema`), no ids (`uuid`), no csv engine, no capacity
//! ordering (`measure_cap` — worth-lived; rung keeps ordering out of
//! mechanism). A carrier walks and reads; it does not judge, measure, or parse.

pub mod config;
pub mod csv;
pub mod error;
pub mod file;
pub mod folder;
pub mod github;
pub mod id;
pub mod jsonl;
pub mod source;

pub use config::{CarrierConfig, CarrierKind};
pub use csv::{CsvFileCarrier, CsvFolderCarrier};
pub use error::CarrierError;
pub use file::FileCarrier;
pub use folder::FolderCarrier;
pub use github::GitHubIssuesCarrier;
pub use id::ObjectId;
pub use jsonl::{JsonlFileCarrier, JsonlFolderCarrier};
pub use source::{Carrier, CarrierRef, ObjectCarrier};
