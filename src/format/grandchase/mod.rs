use anyhow::Result;

use crate::conversion::{Asset, Importer, Scene};

mod frm;
mod p3m;

#[derive(Default)]
pub struct GrandChaseImporter {}

impl Importer for GrandChaseImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        if asset.extension().to_lowercase() == "p3m" {
            p3m::importer::import(asset, scene)
        } else if asset.extension().to_lowercase() == "frm" {
            frm::importer::import(asset, scene)
        } else {
            panic!(
                "`GrandChaseImporter` does not support the extension {}",
                asset.extension()
            );
        }
    }

    fn extensions(&self) -> &[&str] {
        &["p3m", "frm"]
    }
}
