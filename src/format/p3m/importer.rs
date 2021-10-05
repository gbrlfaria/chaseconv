use crate::conversion::{Asset, ConversionError, Importer, Scene};

pub struct GrandChaseImporter {}

impl Importer for GrandChaseImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<(), ConversionError> {
        todo!()
    }

    fn extensions(&self) -> &[&str] {
        &["p3m", "frm"]
    }
}
