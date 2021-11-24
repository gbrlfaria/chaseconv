use anyhow::Result;
use byteorder::{WriteBytesExt, LE};
use gltf::json;
use json::validation::Checked::Valid;

use crate::conversion::{Asset, Exporter, Joint, Mesh, Scene};

pub struct GltfExporter {}

// https://github.com/gltf-rs/gltf/blob/master/examples/export/main.rs
// https://www.khronos.org/registry/glTF/specs/2.0/glTF-2.0.html
// TODO: (final) return Results instead of unwraps
// TODO: u32 returns to usize
impl Exporter for GltfExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        // TODO: transform to correct coordinate system. And normalize normals.
        Ok(Vec::new())
    }
}

fn insert_meshes(root: &mut json::Root, buffer: &mut Vec<u8>, meshes: &[Mesh]) {
    for mesh in meshes {
        let position_accessor = insert_position_bytes(root, buffer, mesh);
        let normal_accessor = insert_normal_bytes(root, buffer, mesh);
        let uv_accessor = insert_uv_bytes(root, buffer, mesh);
    }
}

fn insert_position_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> u32 {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Valid(json::accessor::Type::Vec3),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: Some(
            vec![
                mesh.vertices
                    .iter()
                    .map(|v| v.position.x)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.y)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.z)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
            ]
            .into(),
        ),
        max: Some(
            vec![
                mesh.vertices
                    .iter()
                    .map(|v| v.position.x)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.y)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.z)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap(),
            ]
            .into(),
        ),
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to_multiple_of_four(buffer);
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        for &coordinate in vertex.position.as_ref() {
            buffer.write_f32::<LE>(coordinate).unwrap();
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    (root.accessors.len() - 1) as u32
}

fn insert_normal_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> u32 {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Valid(json::accessor::Type::Vec3),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: true,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to_multiple_of_four(buffer);
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        for &coordinate in vertex.normal.as_ref() {
            buffer.write_f32::<LE>(coordinate).unwrap();
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    (root.accessors.len() - 1) as u32
}

fn insert_uv_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> u32 {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Valid(json::accessor::Type::Vec2),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: true,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to_multiple_of_four(buffer);
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * std::mem::size_of::<[f32; 2]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        for &coordinate in vertex.uv.as_ref() {
            buffer.write_f32::<LE>(coordinate).unwrap();
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    (root.accessors.len() - 1) as u32
}

fn insert_joint_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> u32 {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Valid(json::accessor::Type::Vec4),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::U8,
        )),
        min: None,
        max: None,
        name: None,
        normalized: true,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to_multiple_of_four(buffer);
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * std::mem::size_of::<[u8; 4]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        buffer.extend_from_slice(&[vertex.joint as u8; 4]);
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    (root.accessors.len() - 1) as u32
}

/// Adds zeros to the buffer until it is 4-byte aligned.
fn align_to_multiple_of_four(buffer: &mut Vec<u8>) {
    buffer.append(&mut vec![0; buffer.len() % 4]);
}

/// Converts and inserts the scene and its nodes. Returns the id of the root node of the skeleton.
fn insert_scene_nodes(root: &mut json::Root, skeleton: &[Joint], meshes: &[Mesh]) -> u32 {
    let mut nodes = Vec::new();

    let skeleton_node = push_skeleton_nodes(&mut root.nodes, skeleton);
    nodes.push(skeleton_node);
    for (index, mesh) in meshes.iter().enumerate() {
        let mesh_node = push_mesh_node(&mut root.nodes, mesh, index as u32);
        nodes.push(mesh_node);
    }

    root.scene = Some(json::Index::new(0));
    root.scenes.push(json::Scene {
        nodes: nodes.iter().map(|&node| json::Index::new(node)).collect(),
        name: None,
        extensions: None,
        extras: Default::default(),
    });

    skeleton_node
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

#[cfg(test)]
mod tests {
    use glam::Vec3A;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn scene_nodes() {
        let mut root = json::Root::default();
        let skeleton = [
            Joint {
                translation: Vec3A::new(1., 1., 1.),
                parent: None,
                children: vec![1],
            },
            Joint {
                translation: Vec3A::new(2., 2., 2.),
                parent: Some(0),
                children: Vec::new(),
            },
            Joint {
                translation: Vec3A::new(0., 0., 0.),
                parent: None,
                children: Vec::new(),
            },
        ];
        let meshes = [Mesh {
            name: String::from("goblin"),
            vertices: Vec::new(),
            indexes: Vec::new(),
        }];
        let skeleton_node = insert_scene_nodes(&mut root, &skeleton, &meshes);

        assert_eq!(0, root.scene.unwrap().value());
        assert_eq!(
            vec![3, 4],
            root.scenes[0]
                .nodes
                .iter()
                .map(|x| x.value())
                .collect::<Vec<usize>>()
        );
        assert_eq!(3, skeleton_node);
        assert_eq!(Some(String::from("mesh_goblin")), root.nodes[4].name);
        assert_eq!(Some([2., 2., 2.]), root.nodes[1].translation);

        std::fs::write("output.json", root.to_string_pretty().unwrap()).unwrap();
    }
}
