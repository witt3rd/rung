//! Stable, serializable identity for a subject in a carrier.
//!
//! Opaque to the engine — meaningful only to the deployment. Used wherever only
//! *which* object matters, not its content. A plain string wrapper; no id
//! scheme is imposed, because the theory's id rule is the theory's.

/// A subject's identity within a carrier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ObjectId(pub String);

impl ObjectId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
