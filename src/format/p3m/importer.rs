use crate::conversion::{Asset, ConversionError, Importer, Scene};

pub struct P3mImporter {}

impl Importer for P3mImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<(), ConversionError> {        
        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["p3m"]
    }
}
