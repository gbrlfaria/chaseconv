use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;

use crate::{
    asset::Asset,
    formats::{FrmExporter, FrmImporter, GltfExporter, GltfImporter, P3mExporter, P3mImporter},
    scene::Scene,
};

/// Defines a type that can import asset files into a scene.
#[allow(unused_variables)]
pub trait Importer {
    /// Imports an asset file into a scene.
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()>;
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
}

/// The converter for certain asset format.
/// A converter will convert any available input format to a specific set of output formats.
pub struct Converter {
    /// The display name of the output asset format.
    pub name: &'static str,
    exporters: Vec<Box<dyn Exporter>>,
}

impl Converter {
    pub fn convert(&self, files: &[String], out_path: &str) {
        let importers = importers();
        let importers: HashMap<_, _> = importers
            .iter()
            .flat_map(|importer| importer.extensions().iter().map(move |ext| (ext, importer)))
            .collect();

        let scenes = files
            .iter()
            // Read asset bytes.
            .map(|file| Asset::from_path(file))
            // Skip invalid assets.
            .filter_map(|result| match result {
                Ok(asset) => Some(asset),
                Err(err) => {
                    eprintln!("{}", err);
                    None
                }
            })
            // Import supported formats.
            .filter_map(
                |asset| match importers.get(&asset.extension().to_lowercase().as_str()) {
                    Some(importer) => {
                        let mut scene = Scene::default();

                        eprint!("Importing \"{}.{}\"... ", asset.name(), asset.extension());
                        match importer.import(&asset, &mut scene) {
                            Ok(_) => {
                                eprintln!("Success!",);
                                Some(scene)
                            }
                            Err(err) => {
                                eprintln!("Failure: {}", err);
                                None
                            }
                        }
                    }
                    None => {
                        eprintln!(
                            "Skipped \"{}.{}: unsupported extension\"",
                            asset.name(),
                            asset.extension()
                        );
                        None
                    }
                },
            );

        // Merge imported scenes.
        match scenes.into_iter().reduce(|a, b| a.merge(b)) {
            Some(scene) => {
                fs::create_dir_all(out_path).unwrap_or_else(|err| {
                    eprintln!("Failed to create the output directory: {}", err)
                });

                for exporter in &self.exporters {
                    // Export assets.
                    match exporter.export(&scene) {
                        Ok(assets) => {
                            for asset in assets {
                                let mut path = PathBuf::from(out_path).join(asset.path());
                                if path.exists() {
                                    let uid = &uuid::Uuid::new_v4().to_simple().to_string();
                                    path = PathBuf::from(out_path).join(format!(
                                        "{}_{}.{}",
                                        asset.name(),
                                        &uid[..uid.len() / 2],
                                        asset.extension()
                                    ));
                                }

                                match fs::write(&path, &asset.bytes) {
                                    Ok(_) => {
                                        eprintln!(
                                            "Exported \"{}\" successfully!",
                                            path.file_name()
                                                .unwrap_or_default()
                                                .to_str()
                                                .unwrap_or("<INVALID NAME>"),
                                        );
                                    }
                                    Err(err) => {
                                        eprintln!(
                                            "Failed to export \"{}\": {}",
                                            path.file_name()
                                                .unwrap_or_default()
                                                .to_str()
                                                .unwrap_or("<INVALID NAME>"),
                                            err
                                        )
                                    }
                                };
                            }
                        }
                        Err(err) => {
                            eprintln!("Failed to export the scene: {}", err);
                        }
                    }
                }
            }
            None => {
                eprintln!("No assets were exported")
            }
        }
    }
}

// Returns all importers available.
fn importers() -> Vec<Box<dyn Importer>> {
    vec![
        Box::new(FrmImporter::default()),
        Box::new(P3mImporter::default()),
        Box::new(GltfImporter::default()),
    ]
}

/// Returns all converters available.
pub fn converters() -> Vec<Converter> {
    vec![
        Converter {
            name: ".GLB (glTF)",
            exporters: vec![Box::new(GltfExporter::default())],
        },
        Converter {
            name: ".P3M/FRM (Grand Chase)",
            exporters: vec![
                Box::new(P3mExporter::default()),
                Box::new(FrmExporter::default()),
            ],
        },
    ]
}
