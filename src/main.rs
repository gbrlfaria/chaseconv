use chaseconv::{conversion::{Asset, Exporter, Importer, Scene, converters}, format::{FrmImporter, GltfExporter, P3mImporter}};

fn main() {
    let importers: Vec<Box<dyn Importer>> = vec![
        Box::new(P3mImporter::default()),
        Box::new(FrmImporter::default()),
    ];
    let exporters: Vec<Box<dyn Exporter>> = vec![Box::new(GltfExporter::default())];

    // let inputs: Vec<_> = std::env::args().into_iter().skip(1).collect();

    let items: Vec<_> = converters().iter().map(|x| x.format).collect();
    let x = dialoguer::Select::new().default(0).items(&items).interact().unwrap();

    // let mut scene = Scene::default();

    // let path = "./model_darkmage.p3m";
    // let bytes = std::fs::read(path).unwrap();
    // P3mImporter {}
    //     .import(&Asset::new(bytes, path), &mut scene)
    //     .unwrap();

    // let exporter = GltfExporter {};
    // exporter.transform(&mut scene);
    // let assets = exporter.export(&scene).unwrap();
    // for asset in &assets {
    //     std::fs::write(&asset.path(), &asset.bytes).unwrap();
    // }
}
