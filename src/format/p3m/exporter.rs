use anyhow::Result;

use crate::conversion::{Asset, Exporter, Scene};

#[derive(Default)]
pub struct P3mExporter {}

impl Exporter for P3mExporter {
    fn export(&self, _scene: &Scene) -> Result<Vec<Asset>> {
        todo!()
    }
}
