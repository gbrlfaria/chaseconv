use std::{fs, path::PathBuf};

use anyhow::Result;

pub struct Asset {
    pub bytes: Vec<u8>,
    path: PathBuf,
}

impl Asset {
    pub fn new(bytes: Vec<u8>, path: &str) -> Self {
        Self {
            bytes,
            path: path.into(),
        }
    }

    pub fn from_path(path: &str) -> Result<Self> {
        let bytes = fs::read(path)?;
        Ok(Self::new(bytes, path))
    }

    /// Get a reference to the asset's path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn name(&self) -> &str {
        self.path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .expect("The name of the asset file is not a valid unicode string")
    }

    pub fn extension(&self) -> &str {
        self.path
            .extension()
            .unwrap_or_default()
            .to_str()
            .expect("The extension of the asset file is not a valid unicode string")
    }

    pub fn parent_dir(&self) -> &str {
        self.path
            .parent()
            .unwrap()
            .to_str()
            .expect("The path to directory of the asset is not a valid unicode string")
    }
}
