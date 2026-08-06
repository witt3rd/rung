//! The carrier error, naming the object it happened on.

use super::ObjectId;

/// An error from a carrier operation, naming the subject it happened on.
#[derive(Debug, Clone)]
pub struct CarrierError {
    pub object: ObjectId,
    pub reason: String,
}

impl CarrierError {
    pub fn new(object: ObjectId, reason: impl Into<String>) -> Self {
        Self {
            object,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for CarrierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "carrier {}: {}", self.object, self.reason)
    }
}

impl std::error::Error for CarrierError {}
