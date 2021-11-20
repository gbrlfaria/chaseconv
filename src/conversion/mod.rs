use anyhow::Result;

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
    fn postprocess(&self, scene: &mut Scene) {}
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
    fn preprocess(&self, scene: &mut Scene) {}
}

/// The converter for certain asset format.
pub struct Converter {
    /// The display name of the asset format.
    pub format: &'static str,
    pub importer: Box<dyn Importer>,
    pub exporter: Box<dyn Exporter>,
}

// TODO: extend from Box to Vec<Box<T>>
impl Converter {
    fn new(
        format: &'static str,
        importer: impl Importer + 'static,
        exporter: impl Exporter + 'static,
    ) -> Self {
        Self {
            format,
            importer: Box::new(importer),
            exporter: Box::new(exporter),
        }
    }
}

/// Returns all converters available.
pub fn converters() -> Vec<Converter> {
    Vec::new()
}
