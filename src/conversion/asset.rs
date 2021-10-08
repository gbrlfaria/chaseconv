use std::path::PathBuf;

pub struct Asset {
    pub bytes: Vec<u8>,
    path: PathBuf,
}

impl Asset {
    pub fn new() {
        // TODO: panic on paths that are not files.
    }

    pub fn name(&self) -> &str {
        self.path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .expect("The name of the asset file isn't a valid unicode string")
    }

    pub fn parent_dir(&self) -> &str {
        self.path
            .parent()
            .unwrap()
            .to_str()
            .expect("The path to directory of the asset isn't a valid unicode string")
    }
}
