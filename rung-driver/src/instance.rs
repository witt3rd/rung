//! [`Instance`] — an instance's `config.yaml` (Q18), read by the driver.
//!
//! The driver stays domain-blind by reading *which theory governs* and *which
//! carrier it audits* from one small file, instead of constants baked into a
//! binary. A theory is a crate that knows itself; the driver reads the
//! instance to know *what to point at*.
//!
//! ```yaml
//! theory: rung-question   # which theory! governs this carrier
//! carrier:                # a CarrierConfig (see carrier::config)
//!   kind: folder
//!   path: ./questions/open
//! ```

use serde::{Deserialize, Serialize};

use super::carrier::{CarrierConfig, CarrierRef};

/// An instance declaration: the governing theory and its carrier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    /// Which theory governs this carrier (its crate/module name, e.g.
    /// `rung-question`). The driver never knows the theory's content — a
    /// theory crate instantiates the driver with its own logic.
    pub theory: String,
    pub carrier: CarrierConfig,
}

impl Instance {
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(text)
    }

    /// Build the concrete carrier this instance audits.
    ///
    /// Paths are used as declared — relative to the process cwd. For configs
    /// whose `path:` is meant relative to the config file itself, use
    /// [`Instance::build_carrier_at`].
    pub fn build_carrier(&self) -> Result<CarrierRef, String> {
        self.carrier.build()
    }

    /// Build the carrier, resolving relative colocated paths against `base`
    /// (the config file's directory, say) rather than the process cwd. This is
    /// what makes a `config.yaml` portable: `path: ./questions` means "beside
    /// me".
    pub fn build_carrier_at(&self, base: &std::path::Path) -> Result<CarrierRef, String> {
        let mut cfg = self.carrier.clone();
        if let Some(path) = cfg.path.as_mut() {
            let p = std::path::Path::new(path);
            if !p.is_absolute() {
                *path = base.join(p).to_string_lossy().into_owned();
            }
        }
        cfg.build()
    }
}
