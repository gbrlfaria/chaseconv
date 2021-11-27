use chaseconv::{
    conversion::{Asset, Exporter, Importer, Scene},
    format::{gltf::GltfExporter, p3m::P3mImporter},
};

fn main() {
    let mut scene = Scene::default();

    let path = "./model_darkmage.p3m";
    let bytes = std::fs::read(path).unwrap();
    P3mImporter {}
        .import(&Asset::new(bytes, path), &mut scene)
        .unwrap();
    let assets = GltfExporter {}.export(&scene).unwrap();
    for asset in &assets {
        std::fs::write(&asset.path(), &asset.bytes).unwrap();
    }
}
