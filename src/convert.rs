use crate::{asset::Asset, scene::Scene};

pub trait Importer {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<(), ()>;
    fn extensions(&self) -> &[&str];
}

pub trait Exporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>, ()>;
    fn extensions(&self) -> &[&str];
}
