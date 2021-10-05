use crate::conversion::{Asset, ConversionError, Importer, Mesh, Scene};

use super::internal::P3m;

pub struct P3mImporter {}

impl Importer for P3mImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<(), ConversionError> {
        let p3m = match P3m::from_bytes(&asset.bytes) {
            Ok(p3m) => p3m,
            Err(_) => return Err(ConversionError::FailedDeserialization),
        };

        // This is just a demo
        if p3m.mesh_vertices.len() > 1 {
            scene.meshes.push(Mesh {
                vertices: Vec::new(),
                indexes: Vec::new(),
            })
        }

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["p3m"]
    }
}
