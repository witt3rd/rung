//! JSONL carriers — row-wise `M(S)`, the **flatmap** over lines.
//!
//! A JSONL row is a subject. [`JsonlFileCarrier`] reads a single `.jsonl`
//! file, one subject per non-blank line; [`JsonlFolderCarrier`] flattens a
//! directory of them. Rows are **opaque** — the carrier splits lines, nothing
//! more. Parsing a row into a subject of the sort is the theory's job.
//!
//! ## Id encoding
//!
//! A row's id is `<file-path>/row/<n>` (0-based; blank lines do not count).
//! Sharing the encoding means a population can narrow from folder to file
//! without a subject changing identity.
//!
//! ## Why both file and folder
//!
//! A `JsonlFile` says *which* file holds the population; a `JsonlFolder` walks
//! whatever is there. The narrow form is right when a file is population
//! *beside* files that are not — an operational log, a state sidecar — and a
//! folder walk cannot tell them apart.

use std::path::{Path, PathBuf};

use super::{Carrier, CarrierError, ObjectId};

fn rows_in(path: &Path, base: &ObjectId) -> Result<Vec<ObjectId>, CarrierError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CarrierError::new(base.clone(), format!("{}: {e}", path.display())))?;
    let ids = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(n, _)| ObjectId::new(format!("{}/row/{n}", base.as_str())))
        .collect();
    Ok(ids)
}

/// A carrier backed by one JSONL file — one subject per non-blank line.
#[derive(Debug, Clone)]
pub struct JsonlFileCarrier {
    path: PathBuf,
}

impl JsonlFileCarrier {
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Carrier for JsonlFileCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(self.path.to_string_lossy().into_owned())
    }

    fn exists(&self) -> bool {
        self.path.is_file()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        let id = self.id();
        let ids = match rows_in(&self.path, &id) {
            Ok(v) => v,
            Err(e) => return Box::new(std::iter::once(Err(e))),
        };
        Box::new(ids.into_iter().map(Ok))
    }

    fn read(&self, item: &ObjectId) -> Result<String, CarrierError> {
        let out = self.path.to_string_lossy().into_owned();
        let all = std::fs::read_to_string(&self.path)
            .map_err(|e| CarrierError::new(item.clone(), e.to_string()))?;
        // id is "<path>/row/<n>": index into the non-blank lines
        let row = item
            .as_str()
            .strip_prefix(&out)
            .and_then(|rest| rest.strip_prefix("/row/"))
            .and_then(|n| n.parse::<usize>().ok());
        let n =
            row.ok_or_else(|| CarrierError::new(item.clone(), "id is not a row of this file"))?;
        all.lines()
            .filter(|l| !l.trim().is_empty())
            .nth(n)
            .map(str::to_string)
            .ok_or_else(|| CarrierError::new(item.clone(), "row index out of bounds"))
    }
}

/// A carrier backed by a directory of JSONL files, flattened row-wise.
#[derive(Debug, Clone)]
pub struct JsonlFolderCarrier {
    dir: PathBuf,
}

impl JsonlFolderCarrier {
    pub fn new(dir: impl AsRef<std::path::Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    fn sorted_jsonl_files(&self) -> Result<Vec<PathBuf>, CarrierError> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map_err(|e| CarrierError::new(self.id(), e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|t| t.is_file()).unwrap_or(false)
                    && e.path().extension().map(|x| x == "jsonl").unwrap_or(false)
            })
            .map(|e| e.path())
            .collect();
        entries.sort();
        Ok(entries)
    }
}

impl Carrier for JsonlFolderCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(self.dir.to_string_lossy().into_owned())
    }

    fn exists(&self) -> bool {
        self.dir.is_dir()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        let files = match self.sorted_jsonl_files() {
            Ok(f) => f,
            Err(e) => return Box::new(std::iter::once(Err(e))),
        };
        // the flatmap: rows across all files, in sorted order
        let merge = files.into_iter().flat_map(|path| {
            let base = ObjectId::new(path.to_string_lossy().into_owned());
            match rows_in(&path, &base) {
                Ok(ids) => ids.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            }
        });
        Box::new(merge)
    }

    fn read(&self, item: &ObjectId) -> Result<String, CarrierError> {
        // find which file the id's path prefix points at, then read the row
        let file = item
            .as_str()
            .split("/row/")
            .next()
            .ok_or_else(|| CarrierError::new(item.clone(), "not a row id"))?;
        let path = PathBuf::from(file);
        let all = std::fs::read_to_string(&path)
            .map_err(|e| CarrierError::new(item.clone(), e.to_string()))?;
        let n: usize = item
            .as_str()
            .rsplit_once("/row/")
            .and_then(|(_, n)| n.parse().ok())
            .ok_or_else(|| CarrierError::new(item.clone(), "not a row id"))?;
        all.lines()
            .filter(|l| !l.trim().is_empty())
            .nth(n)
            .map(str::to_string)
            .ok_or_else(|| CarrierError::new(item.clone(), "row index out of bounds"))
    }
}
