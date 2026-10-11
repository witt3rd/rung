//! The context door (S4): stub.

use std::io::{self, Write};
use std::path::Path;

use crate::read::{Door, DoorError};
use crate::redact::Redactor;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContextQuery {
    pub call: Option<u64>,
}

impl ContextQuery {
    pub fn parse(_pairs: &[(String, String)]) -> Door<Self> {
        Ok(Self::default())
    }
}

pub struct ContextPage;

pub fn open_dir(
    _state_dir: &Path,
    _turn: u64,
    _q: &ContextQuery,
    _red: &Redactor,
    _env_unset: Vec<String>,
) -> Door<ContextPage> {
    Err(DoorError::new(501, "not_built", "not built"))
}

impl ContextPage {
    pub fn write_json(self, _out: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }
}
