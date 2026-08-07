//! Configure a pool of principals and run a ladder over a subject.
//!
//! The application layer. `rung` declares arrows and sentences, `rung-std`
//! supplies canonical blocks, and this turns a **declaration of who is
//! available** into a `Pool` that Het's two filters can be run against.
//!
//! ## Deliberately domain-blind
//!
//! Nothing here knows what a proposition, a question or a doctrine is. It knows
//! principals, capabilities, standing, provenance and parks. A domain supplies
//! its own theory, its own ladder and its own population file; this drives them.
//!
//! Encoding rung's own doctrine is the first thing pointed at it, and it must
//! not be the only thing it can carry — a driver that grew a special case for
//! its first caller would be an app wearing a library's name.
//!
//! ## What it does not decide
//!
//! **Which qualifying principal is used.** `Pool::qualify_for` returns *any*
//! survivor of the two filters, and that is Het-correct rather than a placeholder
//! (`no-preference-among-judges`). Preferring one over another is a worth
//! judgment and Het declares no worth law.
//!
//! **Which models are worth having.** Procurement — shopping for a model by
//! price, context length or benchmark — happens *before* a population is
//! declared. By the time a principal reaches the pool the buying is done, so
//! cost never touches the filter and there is nothing here to keep honest.
//!
//! **When to stop waiting.** Parking is [`rung_std::driver::Park`]'s, and it has
//! no timeout for the same reason.

pub mod carrier;
pub mod commission;
pub mod instance;
pub mod judgment;
pub mod oracle_llm;
pub mod pass;
pub mod principal;
pub mod system;

pub use carrier::{
    Carrier, CarrierConfig, CarrierError, CarrierKind, CarrierRef, CsvFileCarrier,
    CsvFolderCarrier, FileCarrier, FolderCarrier, GitHubIssuesCarrier, JsonlFileCarrier,
    JsonlFolderCarrier, ObjectId,
};
pub use commission::CommissionLog;
// The principals convergence: the driver's own Population/PrincipalSpec were
// collapsed into the theory's single Roster/PrincipalDecl in rung_std. The
// unified model (with providers + backing + family) is re-exported here so a
// driver has one import.
pub use instance::Instance;
pub use judgment::{DispatchedJudge, DispatchedRecord};
pub use oracle_llm::{Adjudicate, ModelOracle, Prompt, Unreachable, resolve};
pub use pass::{Audit, CycleOutcome, Finding, audit_run, run_cycle};
pub use principal::{
    Answer, Configured, Oracle, Unwired, population_pool, population_pool_with_log,
};
pub use rung_std::principals::{
    Backing, ConfigError, Kind, PrincipalDecl, Provider, RoleSpec, Roster, RosterFault,
};
pub use system::SystemConfig;

/// Re-exported so a driver has one import for holding suspended runs.
pub use rung_std::driver::Park;
