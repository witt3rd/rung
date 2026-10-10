//! The gateway: one HTTP listener in front of any number of rung hosts.
//!
//! It serves the UI's static app, lists the instances it knows
//! (`GET /api/instances`), and passes `/api/i/{id}/v1/...` through to that
//! instance's own `/v1` doors, adding the instance's key and streaming the
//! answer back untouched. The keys live only here: no page and no response
//! carries one. The tailnet is the boundary, so there is no login; a request
//! that presents a read-only token may only read. Door shapes:
//! `docs/rung-host-api.md`.

pub mod config;
pub mod doors;
pub mod registry;
pub mod role;
mod server;

pub use config::{Config, InstanceConfig, Settings};
pub use doors::{Access, Door};
pub use registry::{Instance, Registry, StaticRegistry};
pub use role::Role;
pub use server::{Running, start, start_with_registry};
