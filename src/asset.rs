use std::{fs, path::PathBuf};

use anyhow::Result;

#[derive(Debug)]
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

    pub fn extension(&self) -> String {
        self.path
            .extension()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .to_string_lossy()
            .to_string()
    }
}
