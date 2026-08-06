//! [`FileCarrier`] — a carrier backed by a single UTF-8 text file.
//!
//! `iter()` yields exactly one subject — the file itself. `read()` returns the
//! full content regardless of id. The reference implementation for a
//! text-document carrier, and the simplest possible carrier.

use std::path::PathBuf;

use super::{Carrier, CarrierError, ObjectId};

/// A carrier backed by one text file.
#[derive(Debug, Clone)]
pub struct FileCarrier {
    path: PathBuf,
}

impl FileCarrier {
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Carrier for FileCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(self.path.to_string_lossy().into_owned())
    }

    fn exists(&self) -> bool {
        self.path.is_file()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        // walk the whole population or fault trying: a missing file is a
        // carrier fault, not an empty sweep
        if self.exists() {
            Box::new(std::iter::once(Ok(self.id())))
        } else {
            Box::new(std::iter::once(Err(CarrierError::new(
                self.id(),
                "file does not exist",
            ))))
        }
    }

    fn read(&self, _item: &ObjectId) -> Result<String, CarrierError> {
        std::fs::read_to_string(&self.path)
            .map_err(|e| CarrierError::new(self.id(), format!("{}: {e}", self.path.display())))
    }
}
