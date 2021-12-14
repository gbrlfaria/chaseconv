use std::{collections::HashMap, mem};

use anyhow::Result;
use byteorder::{WriteBytesExt, LE};
use glam::{Mat4, Vec4};
use gltf::{
    json::{
        self,
        mesh::{Primitive, Semantic},
        validation::Checked,
    },
    Glb,
};

use crate::conversion::{Animation, Asset, Exporter, Joint, Mesh, Scene};

#[derive(Default)]
pub struct GltfExporter {}

// https://www.khronos.org/registry/glTF/specs/2.0/glTF-2.0.html
impl Exporter for GltfExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        let mut root = json::Root::default();
        let mut buffer = Vec::new();

        let scene = transform(scene);

        let skeleton_index = insert_scene(&mut root, &scene.skeleton, &scene.meshes);
        insert_meshes(&mut root, &mut buffer, &scene.meshes)?;
        insert_skins(&mut root, &mut buffer, &scene, skeleton_index)?;
        insert_animations(
            &mut root,
            &mut buffer,
            &scene.animations,
            scene.skeleton.len(),
            skeleton_index,
        )?;
        insert_buffers(&mut root, &buffer);

        root.asset = json::Asset {
            generator: Some(format!(
                "{} {}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            )),
            ..Default::default()
        };

        let json_string = json::serialize::to_string(&root)?;
        let bytes = Glb {
            header: gltf::binary::Header {
                magic: *b"glTF",
                version: 2,
                length: calculate_length(&json_string, &buffer) as u32,
            },
            json: json_string.into_bytes().into(),
            bin: Some(buffer.into()),
        }
        .to_vec()?;

        let name = if let Some(mesh) = scene.meshes.first() {
            &mesh.name
        } else if let Some(animation) = scene.animations.first() {
            &animation.name
        } else {
            "model"
        };

        Ok(vec![Asset::new(bytes, &format!("{}.glb", name))])
    }
}

fn calculate_length(json: &str, bin: &[u8]) -> usize {
    const HEADER_SIZE: usize = 12;
    const CHUNK_HEADER_SIZE: usize = 8;

    let mut length = HEADER_SIZE + CHUNK_HEADER_SIZE + json.len();
    length += length % 4;
    length += CHUNK_HEADER_SIZE + bin.len();
    length += length % 4;

    length
}

fn transform(scene: &Scene) -> Scene {
    let mut scene = scene.clone();

    let mut matrix = Mat4::IDENTITY;
    matrix.z_axis = Vec4::new(0., 0., -1., 0.);

    for mesh in &mut scene.meshes {
        for vertex in &mut mesh.vertices {
            vertex.position = matrix.transform_point3a(vertex.position);
            vertex.normal = matrix.transform_point3a(vertex.normal);
        }

        for i in 0..mesh.indices.len() / 3 {
            mesh.indices.swap(i * 3 + 1, i * 3 + 2);
        }
    }

    for joint in &mut scene.skeleton {
        joint.translation = matrix.transform_point3a(joint.translation);
    }

    for animation in &mut scene.animations {
        for frame in &mut animation.frames {
            frame.translation.z *= -1.;
            for transform in &mut frame.transforms {
                *transform = matrix.mul_mat4(transform).mul_mat4(&matrix.inverse());
            }
        }
    }

    scene
}

/// Converts and inserts the scene and its nodes into the json.
/// Returns the index of the root node of the skeleton in the node hierarchy.
fn insert_scene(root: &mut json::Root, skeleton: &[Joint], meshes: &[Mesh]) -> usize {
    let mut nodes = Vec::new();

    let skeleton_node = push_skeleton_nodes(&mut root.nodes, skeleton);
    nodes.push(skeleton_node);
    for (index, mesh) in meshes.iter().enumerate() {
        let mesh_node = push_mesh_node(&mut root.nodes, mesh, index as u32);
        nodes.push(mesh_node);
    }

    root.scene = Some(json::Index::new(0));
    root.scenes.push(json::Scene {
        nodes: nodes
            .iter()
            .map(|&node| json::Index::new(node as u32))
            .collect(),
        name: None,
        extensions: None,
        extras: Default::default(),
    });

    skeleton_node
}

fn push_skeleton_nodes(nodes: &mut Vec<json::Node>, skeleton: &[Joint]) -> usize {
    let mut roots = Vec::new();

    let offset = nodes.len() as u32;
    for (index, joint) in skeleton.iter().enumerate() {
        if joint.parent.is_none() {
            roots.push(offset + index as u32)
        }

        nodes.push(json::Node {
            name: Some(format!("bone_{}", index)),
            children: if !joint.children.is_empty() {
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

    nodes.len() - 1
}

fn push_mesh_node(nodes: &mut Vec<json::Node>, mesh: &Mesh, index: u32) -> usize {
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

    nodes.len() - 1
}

fn insert_skins(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    scene: &Scene,
    skeleton_index: usize,
) -> Result<()> {
    if scene.skeleton.is_empty() {
        return Ok(());
    }
    let inverse_bind_accessor = insert_inverse_bind_bytes(root, buffer, scene)?;
    root.skins = vec![json::Skin {
        inverse_bind_matrices: Some(json::Index::new(inverse_bind_accessor as u32)),
        joints: (0..scene.skeleton.len())
            .map(|index| json::Index::new(index as u32))
            .collect(),
        skeleton: Some(json::Index::new(skeleton_index as u32)),
        name: None,
        extensions: None,
        extras: Default::default(),
    }];

    Ok(())
}

fn insert_meshes(root: &mut json::Root, buffer: &mut Vec<u8>, meshes: &[Mesh]) -> Result<()> {
    for mesh in meshes {
        let positions_accessor = insert_positions_bytes(root, buffer, mesh)?;
        let normals_accessor = insert_normals_bytes(root, buffer, mesh)?;
        let uv_accessor = insert_uv_bytes(root, buffer, mesh)?;
        let joints_accessor = insert_joints_bytes(root, buffer, mesh)?;
        let weights_accessor = insert_weights_bytes(root, buffer, mesh)?;
        let indices_accessor = insert_indices_bytes(root, buffer, mesh)?;

        let mut attributes = HashMap::new();
        attributes.insert(
            Checked::Valid(Semantic::Positions),
            json::Index::new(positions_accessor as u32),
        );
        attributes.insert(
            Checked::Valid(Semantic::Normals),
            json::Index::new(normals_accessor as u32),
        );
        attributes.insert(
            Checked::Valid(Semantic::TexCoords(0)),
            json::Index::new(uv_accessor as u32),
        );
        attributes.insert(
            Checked::Valid(Semantic::Joints(0)),
            json::Index::new(joints_accessor as u32),
        );
        attributes.insert(
            Checked::Valid(Semantic::Weights(0)),
            json::Index::new(weights_accessor as u32),
        );

        root.meshes.push(json::Mesh {
            name: Some(format!("mesh_{}", mesh.name)),
            primitives: vec![Primitive {
                attributes,
                extensions: None,
                indices: Some(json::Index::new(indices_accessor as u32)),
                material: None,
                targets: None,
                mode: Default::default(),
                extras: Default::default(),
            }],
            extensions: None,
            weights: None,
            extras: Default::default(),
        });
    }

    Ok(())
}

fn insert_buffers(root: &mut json::Root, buffer: &[u8]) {
    root.buffers.push(json::Buffer {
        byte_length: buffer.len() as u32,
        uri: None,
        name: None,
        extensions: None,
        extras: Default::default(),
    });
}

fn insert_animations(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    animations: &[Animation],
    skeleton_len: usize,
    skeleton_index: usize,
) -> Result<()> {
    for animation in animations {
        let mut gltf_animation = json::Animation {
            name: Some(animation.name.clone()),
            samplers: Vec::new(),
            channels: Vec::new(),
            extensions: None,
            extras: Default::default(),
        };

        let time_accessor = insert_time_bytes(root, buffer, animation)?;

        let translations_accessor = insert_translations_bytes(root, buffer, animation)?;
        gltf_animation.samplers.push(json::animation::Sampler {
            input: json::Index::new(time_accessor as u32),
            output: json::Index::new(translations_accessor as u32),
            // For the sake of simplicity, we use linear interpolation. In reality,
            // Grand Chase uses bezier curves with unknown in-tangent and out-tangent values.
            interpolation: Checked::Valid(gltf::animation::Interpolation::Linear),
            extensions: None,
            extras: Default::default(),
        });
        gltf_animation.channels.push(json::animation::Channel {
            sampler: json::Index::new(gltf_animation.channels.len() as u32),
            target: json::animation::Target {
                node: json::Index::new(skeleton_index as u32),
                path: Checked::Valid(gltf::animation::Property::Translation),
                extensions: None,
                extras: Default::default(),
            },
            extensions: None,
            extras: Default::default(),
        });

        for (index, transforms) in animation.joints().iter().enumerate().take(skeleton_len) {
            let rotations_accessor = insert_rotations_bytes(root, buffer, transforms)?;
            gltf_animation.samplers.push(json::animation::Sampler {
                input: json::Index::new(time_accessor as u32),
                output: json::Index::new(rotations_accessor as u32),
                interpolation: Checked::Valid(gltf::animation::Interpolation::Linear),
                extensions: None,
                extras: Default::default(),
            });
            gltf_animation.channels.push(json::animation::Channel {
                sampler: json::Index::new(gltf_animation.channels.len() as u32),
                target: json::animation::Target {
                    // The index of the iteration corresponds to the index of the node hierarchy
                    // because joints are the first things to be inserted, and they are insserted
                    // in order.
                    node: json::Index::new(index as u32),
                    path: Checked::Valid(gltf::animation::Property::Rotation),
                    extensions: None,
                    extras: Default::default(),
                },
                extensions: None,
                extras: Default::default(),
            });

            // TODO: translations of individual joints are not currently supported for exporting.
        }

        root.animations.push(gltf_animation);
    }

    Ok(())
}

fn insert_positions_bytes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    mesh: &Mesh,
) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec3),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: Some(
            vec![
                mesh.vertices
                    .iter()
                    .map(|v| v.position.x)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.y)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.z)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default(),
            ]
            .into(),
        ),
        max: Some(
            vec![
                mesh.vertices
                    .iter()
                    .map(|v| v.position.x)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.y)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default(),
                mesh.vertices
                    .iter()
                    .map(|v| v.position.z)
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default(),
            ]
            .into(),
        ),
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * mem::size_of::<[f32; 3]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        for &coordinate in vertex.position.as_ref() {
            buffer.write_f32::<LE>(coordinate)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_normals_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec3),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * mem::size_of::<[f32; 3]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        for &coordinate in vertex.normal.normalize_or_zero().as_ref() {
            buffer.write_f32::<LE>(coordinate)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_uv_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec2),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * mem::size_of::<[f32; 2]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        for &coordinate in vertex.uv.as_ref() {
            buffer.write_f32::<LE>(coordinate)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_joints_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec4),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::U8,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<u8>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * mem::size_of::<[u8; 4]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        buffer.extend_from_slice(&[vertex.joint.unwrap_or_default() as u8, 0, 0, 0]);
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_weights_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.vertices.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec4),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.vertices.len() * mem::size_of::<[f32; 4]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for vertex in &mesh.vertices {
        let weight = match vertex.joint {
            Some(_) => 1.,
            None => 0.,
        };
        for coordinate in [weight, 0., 0., 0.] {
            buffer.write_f32::<LE>(coordinate)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_indices_bytes(root: &mut json::Root, buffer: &mut Vec<u8>, mesh: &Mesh) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: mesh.indices.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Scalar),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::U16,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<u16>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (mesh.indices.len() * mem::size_of::<u16>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for &index in &mesh.indices {
        buffer.write_u16::<LE>(index as u16)?;
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_inverse_bind_bytes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    scene: &Scene,
) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: scene.skeleton.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Mat4),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (scene.skeleton.len() * mem::size_of::<[f32; 4 * 4]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for (index, _) in scene.skeleton.iter().enumerate() {
        let translation = Vec4::from((-scene.joint_world_translation(index), 1.));

        let mut matrix = Mat4::IDENTITY;
        matrix.w_axis = translation;
        for value in matrix.to_cols_array() {
            buffer.write_f32::<LE>(value)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_time_bytes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    animation: &Animation,
) -> Result<usize> {
    let times: Vec<_> = animation
        .frames
        .iter()
        .enumerate()
        .map(|(i, _)| i as f32 * (1. / animation.sampling_rate() as f32))
        .collect();

    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: animation.frames.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Scalar),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: Some(
            [times
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or_default()]
            .as_ref()
            .into(),
        ),
        max: Some(
            [times
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or_default()]
            .as_ref()
            .into(),
        ),
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (animation.frames.len() * mem::size_of::<f32>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for &time in &times {
        buffer.write_f32::<LE>(time)?;
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_translations_bytes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    animation: &Animation,
) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: animation.frames.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec3),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (animation.frames.len() * mem::size_of::<[f32; 3]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for frame in &animation.frames {
        for &coordinate in frame.translation.as_ref() {
            buffer.write_f32::<LE>(coordinate)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

fn insert_rotations_bytes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    rotations: &[&Mat4],
) -> Result<usize> {
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: rotations.len() as u32,
        type_: Checked::Valid(json::accessor::Type::Vec4),
        component_type: Checked::Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    };

    align_to(buffer, mem::size_of::<f32>());
    let view = json::buffer::View {
        buffer: json::Index::new(root.buffers.len() as u32),
        byte_offset: Some(buffer.len() as u32),
        byte_length: (rotations.len() * mem::size_of::<[f32; 4]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for matrix in rotations {
        let (_, rotation, _) = matrix.to_scale_rotation_translation();
        for &value in rotation.as_ref() {
            buffer.write_f32::<LE>(value)?;
        }
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

/// Adds zeros to the buffer until it is n-byte aligned.
fn align_to(buffer: &mut Vec<u8>, n: usize) {
    buffer.append(&mut vec![0; buffer.len() % n]);
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
            indices: Vec::new(),
        }];
        let skeleton_node = insert_scene(&mut root, &skeleton, &meshes);

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
    }
}
