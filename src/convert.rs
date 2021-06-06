use crate::{asset::Asset, scene::Scene};

/// Defines a type that can import asset files into a scene.
#[allow(unused_variables)]
pub trait Importer {
    /// Imports an asset file into a scene.
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<(), ConversionError>;
    /// Postprocesses a scene after all its assets are imported. It's usually used to
    /// transform the scene geometry into the appropriate coordinate system.
    fn postprocess(&self, scene: &mut Scene) {}
    /// Returns the file extensions supported by the importer. These extensions are used to
    /// select the appropriate importer given an asset file.
    fn extensions(&self) -> &[&str];
}

/// Defines a type that can export a scene into asset files.
#[allow(unused_variables)]
pub trait Exporter {
    /// Exports a scene into one or more asset files.
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>, ConversionError>;
    /// Preprocesses a scene before it is exported. It's usually used to
    /// transform the scene geometry into the appropriate coordinate system.
    fn preprocess(&self, scene: &mut Scene) {}
}

/// The error type for conversion operations of the [`Importer`] and [`Exporter`] traits.
pub enum ConversionError {}

/// The converter for certain asset format.
pub struct Converter {
    /// The display name of the asset format.
    pub format: &'static str,
    pub importer: Box<dyn Importer>,
    pub exporter: Box<dyn Exporter>,
}

impl Converter {
    /// Generic helper function that creates a new converter.
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
