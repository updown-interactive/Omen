//! Local profile used by Studio's file templates.
use crate::services;
use serde::{Deserialize, Serialize};
use std::{io, path::Path};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Account {
    pub name: String,
    pub email: String,
}

impl Account {
    pub(crate) fn load(directory: &Path) -> io::Result<Self> {
        services::read_json(&directory.join("account.json"))
    }

    pub(crate) fn save(&self, directory: &Path) -> io::Result<()> {
        services::write_json(&directory.join("account.json"), self)
    }
}
