//! Resolve a carrier from an instance's `config.yaml` (Q18).
//!
//! The driver stays domain-blind by reading a [`CarrierConfig`] that names the
//! *strategy* and its location — colocated (a folder, a file, a jsonl/csv
//! stream) or external (GitHub issues via `gh`). [`CarrierConfig::build`]
//! converts one into a concrete [`Carrier`].
//!
//! ```yaml
//! carrier:
//!   kind: folder          # folder | file | jsonl | jsonl-folder | csv | csv-folder | github
//!   path: ./questions     # colocated location (ignored for github)
//!   repos: []             # github only: owner/repo list
//! ```

use serde::{Deserialize, Serialize};

use super::{
    CarrierRef, CsvFileCarrier, CsvFolderCarrier, FileCarrier, FolderCarrier, GitHubIssuesCarrier,
    JsonlFileCarrier, JsonlFolderCarrier,
};

/// The carrier strategy an instance's config selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CarrierKind {
    Folder,
    File,
    Jsonl,
    JsonlFolder,
    Csv,
    CsvFolder,
    /// External, via the ambient `gh` CLI. Discriminated `github` (not kebab
    /// `git-hub`).
    #[serde(rename = "github")]
    GitHub,
}

/// A carrier declaration from config — strategy + location, nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierConfig {
    pub kind: CarrierKind,
    /// Colocated location (folder/file/jsonl/csv paths).
    #[serde(default)]
    pub path: Option<String>,
    /// External: repo list (`owner/repo`) for `github`.
    #[serde(default)]
    pub repos: Vec<String>,
}

impl CarrierConfig {
    /// Turn this declaration into a concrete [`Carrier`].
    pub fn build(&self) -> Result<CarrierRef, String> {
        let path = |kind: &str| -> Result<&str, String> {
            self.path
                .as_deref()
                .ok_or_else(|| format!("a `{kind}` carrier needs a `path` in config"))
        };
        let c: CarrierRef = match self.kind {
            CarrierKind::Folder => Arc::new(FolderCarrier::new(path("folder")?)),
            CarrierKind::File => Arc::new(FileCarrier::new(path("file")?)),
            CarrierKind::Jsonl => Arc::new(JsonlFileCarrier::new(path("jsonl")?)),
            CarrierKind::JsonlFolder => Arc::new(JsonlFolderCarrier::new(path("jsonl-folder")?)),
            CarrierKind::Csv => Arc::new(CsvFileCarrier::new(path("csv")?)),
            CarrierKind::CsvFolder => Arc::new(CsvFolderCarrier::new(path("csv-folder")?)),
            CarrierKind::GitHub => Arc::new(GitHubIssuesCarrier::new(self.repos.clone())?),
        };
        Ok(c)
    }
}

use std::sync::Arc;
