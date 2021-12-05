use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;

use crate::format::{GltfExporter, GrandChaseImporter};

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
/// A converter will convert any available input format to a specific set of output formats.
pub struct Converter {
    /// The display name of the output asset format.
    pub name: &'static str,
    exporter: Box<dyn Exporter>,
}

impl Converter {
    pub fn convert(&self, files: &[String], out_path: &str) {
        let importers = importers();
        let importers: HashMap<_, _> = importers
            .iter()
            .flat_map(|importer| importer.extensions().iter().map(move |ext| (ext, importer)))
            .collect();

        let scenes: Vec<_> = files
            .iter()
            // Read asset bytes.
            .map(|file| Asset::from_path(file))
            // Skip invalid assets.
            .filter_map(|result| match result {
                Ok(asset) => Some(asset),
                Err(err) => {
                    eprintln!("{}", err.to_string());
                    None
                }
            })
            // Import supported formats.
            .filter_map(
                |asset| match importers.get(&asset.extension().to_lowercase().as_str()) {
                    Some(importer) => {
                        let mut scene = Scene::default();
                        match importer.import(&asset, &mut scene) {
                            Ok(_) => {
                                println!(
                                    "Imported \"{}.{}\" successfully!",
                                    asset.name(),
                                    asset.extension()
                                );
                                importer.transform(&mut scene);
                                Some(scene)
                            }
                            Err(err) => {
                                eprintln!(
                                    "Failed to import \"{}.{}\"! {}",
                                    asset.name(),
                                    asset.extension(),
                                    err.to_string()
                                );
                                None
                            }
                        }
                    }
                    None => None,
                },
            )
            .collect();

        // Merge imported scenes.
        match scenes.into_iter().reduce(|a, b| a.merge(b)) {
            Some(mut scene) => {
                fs::create_dir_all(&out_path).unwrap_or_else(|err| {
                    eprintln!("Failed to create the output directory: {}", err)
                });

                // Export assets.
                self.exporter.transform(&mut scene);
                match self.exporter.export(&scene) {
                    Ok(assets) => {
                        for asset in assets {
                            let path = PathBuf::from(out_path).join(asset.path());
                            fs::write(&path, &asset.bytes).unwrap_or_else(|err| {
                                eprintln!(
                                    "Failed to export the asset \"{}.{}\": {}",
                                    asset.name(),
                                    asset.extension(),
                                    err
                                )
                            });
                        }
                    }
                    Err(err) => {
                        eprintln!("Failed to export the scene: {}", err.to_string());
                    }
                }
            }
            None => {
                println!("No assets were exported")
            }
        }
    }
}

// Returns all importers available.
fn importers() -> Vec<Box<dyn Importer>> {
    vec![Box::new(GrandChaseImporter::default())]
}

/// Returns all converters available.
pub fn converters() -> Vec<Converter> {
    vec![
        Converter {
            name: ".P3M/FRM (Grand Chase)",
            exporter: Box::new(GltfExporter::default()),
        },
        Converter {
            name: ".GLB (glTF)",
            exporter: Box::new(GltfExporter::default()),
        },
    ]
}
