//! CSV carriers — a flat table, one subject per **row** (header excluded).
//!
//! [`CsvFolderCarrier`] walks a directory of `.csv` files row-wise and flat;
//! [`CsvFileCarrier`] reads one. The `csv` crate is used only to get **record
//! boundaries** right (quoted fields, embedded commas, CRLF) — the records
//! themselves are opaque text. Parsing fields is the theory's job, never the
//! carrier's.
//!
//! Ids are `<file-path>/row/<n>` (0-based, header excluded), so a population
//! can narrow from folder to file without any row changing identity.

use std::path::{Path, PathBuf};

use super::{Carrier, CarrierError, ObjectId};

/// Rows of one CSV file as opaque subject ids (`<path>/row/<n>`).
fn rows_in(path: &Path, base: &ObjectId) -> Result<Vec<(ObjectId, String)>, CarrierError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|e| CarrierError::new(base.clone(), e.to_string()))?;
    let mut rows = Vec::new();
    for (n, rec) in reader.records().enumerate() {
        let rec = rec.map_err(|e| CarrierError::new(base.clone(), e.to_string()))?;
        rows.push((
            ObjectId::new(format!("{}/row/{n}", base.as_str())),
            rec.iter().collect::<Vec<&str>>().join(","),
        ));
    }
    Ok(rows)
}

/// A carrier backed by one CSV file — one subject per data row.
#[derive(Debug, Clone)]
pub struct CsvFileCarrier {
    path: PathBuf,
}

impl CsvFileCarrier {
    pub fn new(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Carrier for CsvFileCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(self.path.to_string_lossy().into_owned())
    }
    fn exists(&self) -> bool {
        self.path.is_file()
    }
    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        let base = self.id();
        let rows = match rows_in(&self.path, &base) {
            Ok(r) => r,
            Err(e) => return Box::new(std::iter::once(Err(e))),
        };
        Box::new(rows.into_iter().map(|(id, _)| Ok(id)))
    }
    fn read(&self, item: &ObjectId) -> Result<String, CarrierError> {
        let out = self.path.to_string_lossy().into_owned();
        let n: usize = item
            .as_str()
            .strip_prefix(&out)
            .and_then(|r| r.strip_prefix("/row/"))
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| CarrierError::new(item.clone(), "not a row of this file"))?;
        let rows = rows_in(&self.path, &self.id())?;
        rows.get(n)
            .map(|(_, text)| text.clone())
            .ok_or_else(|| CarrierError::new(item.clone(), "row index out of bounds"))
    }
}

/// A carrier backed by a directory of CSV files, flattened row-wise.
#[derive(Debug, Clone)]
pub struct CsvFolderCarrier {
    dir: PathBuf,
}

impl CsvFolderCarrier {
    pub fn new(dir: impl AsRef<std::path::Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }
    fn sorted_csv_files(&self) -> Result<Vec<PathBuf>, CarrierError> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map_err(|e| CarrierError::new(self.id(), e.to_string()))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|t| t.is_file()).unwrap_or(false)
                    && e.path().extension().and_then(|x| x.to_str()) == Some("csv")
            })
            .map(|e| e.path())
            .collect();
        entries.sort();
        Ok(entries)
    }
}

impl Carrier for CsvFolderCarrier {
    fn id(&self) -> ObjectId {
        ObjectId::new(self.dir.to_string_lossy().into_owned())
    }
    fn exists(&self) -> bool {
        self.dir.is_dir()
    }
    fn iter(&self) -> Box<dyn Iterator<Item = Result<ObjectId, CarrierError>> + '_> {
        let files = match self.sorted_csv_files() {
            Ok(f) => f,
            Err(e) => return Box::new(std::iter::once(Err(e))),
        };
        let merge = files.into_iter().flat_map(|path| {
            let base = ObjectId::new(path.to_string_lossy().into_owned());
            match rows_in(&path, &base) {
                Ok(rows) => rows.into_iter().map(|(id, _)| Ok(id)).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            }
        });
        Box::new(merge)
    }
    fn read(&self, item: &ObjectId) -> Result<String, CarrierError> {
        let file = item
            .as_str()
            .split("/row/")
            .next()
            .ok_or_else(|| CarrierError::new(item.clone(), "not a row id"))?;
        let rows = rows_in(Path::new(file), &self.id())?;
        let n: usize = item
            .as_str()
            .rsplit_once("/row/")
            .and_then(|(_, n)| n.parse().ok())
            .ok_or_else(|| CarrierError::new(item.clone(), "not a row id"))?;
        rows.get(n)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| CarrierError::new(item.clone(), "row index out of bounds"))
    }
}
