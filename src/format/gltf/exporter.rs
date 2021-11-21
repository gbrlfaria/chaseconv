use anyhow::Result;

use crate::conversion::{Asset, Exporter, Scene};

pub struct GltfExporter {}

impl Exporter for GltfExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        Ok(Vec::new())
    }
}
