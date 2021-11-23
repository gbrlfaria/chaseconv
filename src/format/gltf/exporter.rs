use anyhow::Result;
use gltf::json;

use crate::conversion::{Asset, Exporter, Joint, Mesh, Scene};

pub struct GltfExporter {}

impl Exporter for GltfExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        // TODO: transform to correct coordinate system.
        Ok(Vec::new())
    }
}

fn convert_scene(root: &mut json::Root, skeleton: &[Joint], meshes: &[Mesh]) {
    let mut nodes = Vec::new();

    let node = push_skeleton_nodes(&mut root.nodes, skeleton);
    nodes.push(node);
    for (index, mesh) in meshes.iter().enumerate() {
        let node = push_mesh_node(&mut root.nodes, mesh, index as u32);
        nodes.push(node);
    }

    root.scene = Some(json::Index::new(0));
    root.scenes.push(json::Scene {
        nodes: nodes.iter().map(|&node| json::Index::new(node)).collect(),
        name: None,
        extensions: None,
        extras: Default::default(),
    });
}

fn push_skeleton_nodes(nodes: &mut Vec<json::Node>, skeleton: &[Joint]) -> u32 {
    let mut roots = Vec::new();

    let offset = nodes.len() as u32;
    for (index, joint) in skeleton.iter().enumerate() {
        if joint.parent.is_none() {
            roots.push(offset + index as u32)
        }

        nodes.push(json::Node {
            name: Some(format!("joint_{}", index)),
            children: if joint.children.len() > 0 {
                Some(
                    joint
                        .children
                        .iter()
                        .map(|&child| json::Index::new(offset + child as u32))
                        .collect(),
                )
            } else {
                None
            },
            translation: Some(joint.translation.into()),
            camera: None,
            extensions: None,
            matrix: None,
            mesh: None,
            rotation: None,
            scale: None,
            skin: None,
            weights: None,
            extras: Default::default(),
        });
    }

    nodes.push(json::Node {
        name: Some(String::from("skeleton")),
        children: Some(roots.iter().map(|&root| json::Index::new(root)).collect()),
        translation: None,
        camera: None,
        extensions: None,
        matrix: None,
        mesh: None,
        rotation: None,
        scale: None,
        skin: None,
        weights: None,
        extras: Default::default(),
    });

    (nodes.len() - 1) as u32
}

fn push_mesh_node(nodes: &mut Vec<json::Node>, mesh: &Mesh, index: u32) -> u32 {
    nodes.push(json::Node {
        name: Some(format!("mesh_{}", mesh.name)),
        mesh: Some(json::Index::new(index)),
        skin: Some(json::Index::new(0)),
        children: None,
        translation: None,
        camera: None,
        extensions: None,
        matrix: None,
        rotation: None,
        scale: None,
        weights: None,
        extras: Default::default(),
    });

    (nodes.len() - 1) as u32
}
