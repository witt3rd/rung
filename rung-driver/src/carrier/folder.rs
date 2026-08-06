//! [`FolderCarrier`] — a carrier backed by a directory of files.
//!
//! `iter()` yields one subject per regular file in the directory, sorted by
//! file name, so the walk is deterministic. Subject ids are the file paths.

use std::path::PathBuf;

use super::{Carrier, CarrierError, ObjectId};

/// A carrier backed by a directory, one subject per regular file.
#[derive(Debug, Clone)]
pub struct FolderCarrier {
    dir: PathBuf,
}

impl FolderCarrier {
    pub fn new(dir: impl AsRef<std::path::Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    fn sorted_files(&self) -> Result<Vec<PathBuf>, CarrierError> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map_err(|e| CarrierError::new(self.id(), e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        entries.sort();
        Ok(entries)
    }
}

impl Carrier for FolderCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(self.dir.to_string_lossy().into_owned())
    }

    fn exists(&self) -> bool {
        self.dir.is_dir()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        let files = match self.sorted_files() {
            Ok(f) => f,
            Err(e) => return Box::new(std::iter::once(Err(e))),
        };
        Box::new(
            files
                .into_iter()
                .map(|p| Ok(ObjectId::new(p.to_string_lossy().into_owned()))),
        )
    }

    fn read(&self, item: &ObjectId) -> Result<String, CarrierError> {
        std::fs::read_to_string(item.as_str())
            .map_err(|e| CarrierError::new(item.clone(), e.to_string()))
    }
}
