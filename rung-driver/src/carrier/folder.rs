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

impl super::ObjectCarrier for FolderCarrier {
    /// Write the subject as a new file in the directory, named by its id
    /// (`.md` appended if absent) — each subject is one file here. Returns the
    /// file's path as the carrier-assigned id, mirroring what `iter()` yields.
    fn add(
        &self,
        id: &super::ObjectId,
        content: &str,
    ) -> Result<super::ObjectId, super::CarrierError> {
        let mut name = id.as_str().to_string();
        if !name.ends_with(".md") {
            name.push_str(".md");
        }
        let path = self.dir.join(&name);
        std::fs::write(&path, content).map_err(|e| {
            super::CarrierError::new(self.id(), format!("write {}: {e}", path.display()))
        })?;
        Ok(super::ObjectId::new(path.to_string_lossy().into_owned()))
    }

    /// Delete the subject's file. The id is the file path, as `iter()` yields.
    fn remove(&self, id: &super::ObjectId) -> Result<(), super::CarrierError> {
        std::fs::remove_file(id.as_str())
            .map_err(|e| super::CarrierError::new(id.clone(), e.to_string()))
    }
}
