use anyhow::Result;

use crate::format::{FrmImporter, GltfExporter, P3mImporter};

pub use self::{
    asset::Asset,
    scene::{Animation, Joint, Keyframe, Mesh, Scene, Vertex},
};

mod asset;
mod scene;

/// Defines a type that can import asset files into a scene.
#[allow(unused_variables)]
pub trait Importer {
    /// Imports an asset file into a scene.
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()>;
    /// Postprocesses a scene after all its assets are imported. It's usually used to
    /// transform the scene geometry into the coordinate system of the intermediary
    /// scene format.
    fn transform(&self, scene: &mut Scene) {}
    /// Returns the file extensions supported by the importer. These extensions are used to
    /// select the appropriate importer given an asset file.
    ///
    /// The extension should not include the period (e.g "zip", not ".zip").
    fn extensions(&self) -> &[&str];
}

/// Defines a type that can export a scene into asset files.
#[allow(unused_variables)]
pub trait Exporter {
    /// Exports a scene into one or more asset files.
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>>;
    /// Preprocesses a scene before it's exported. It's usually used to
    /// transform the scene geometry into coordinate system of the output format.
    fn transform(&self, scene: &mut Scene) {}
}

/// The converter for certain asset format.
pub struct Converter {
    /// The display name of the output asset format.
    pub format: &'static str,
    exporters: Vec<Box<dyn Exporter>>,
}

impl Converter {}

// Returns all importers available.
fn importers() -> Vec<Box<dyn Importer>> {
    vec![
        Box::new(P3mImporter::default()),
        Box::new(FrmImporter::default()),
    ]
}

/// Returns all converters available.
pub fn converters() -> Vec<Converter> {
    vec![
        Converter {
            format: ".P3M/FRM (Grand Chase)",
            exporters: vec![],
        },
        Converter {
            format: ".GLB (glTF)",
            exporters: vec![Box::new(GltfExporter::default())],
        },
    ]
}
