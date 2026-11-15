use std::{collections::HashMap, mem};

use anyhow::Result;
use byteorder::{WriteBytesExt, LE};
use glam::{Mat3, Mat4, Quat, Vec3};
use gltf::{
    animation::Property,
    json::{
        self,
        mesh::{Primitive, Semantic},
        validation::Checked,
    },
    Glb,
};

use crate::{
    asset::Asset,
    conversion::Exporter,
    scene::{Animation, Joint, Mesh, Scene},
};

#[derive(Default)]
pub struct GltfExporter {}

// https://www.khronos.org/registry/glTF/specs/2.0/glTF-2.0.html
impl Exporter for GltfExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        let mut root = json::Root::default();
        let mut buffer = Vec::new();

        let scene = super::transform(scene);

        let skeleton_index = insert_scene(&mut root, &scene.skeleton, &scene.meshes);
        insert_meshes(&mut root, &mut buffer, &scene.meshes, &scene.skeleton)?;
        insert_skins(&mut root, &mut buffer, &scene, skeleton_index)?;
        insert_animations(
            &mut root,
            &mut buffer,
            &scene.animations,
            &scene.skeleton,
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

/// Converts and inserts the scene and its nodes into the json.
/// Returns the index of the root node of the skeleton in the node hierarchy.
fn insert_scene(root: &mut json::Root, skeleton: &[Joint], meshes: &[Mesh]) -> usize {
    let mut nodes = Vec::new();

    let skeleton_node = push_skeleton_nodes(&mut root.nodes, skeleton);
    nodes.push(skeleton_node);
    // `insert_skins` writes a skin only if the skeleton has joints.
    let skinned = !skeleton.is_empty();
    for (index, mesh) in meshes.iter().enumerate() {
        let mesh_node = push_mesh_node(&mut root.nodes, mesh, index as u32, skinned);
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
    let mut detached = Vec::new();

    let offset = nodes.len() as u32;
    for (index, joint) in skeleton.iter().enumerate() {
        if joint.detached {
            detached.push(offset + index as u32);
        } else if joint.parent.is_none() {
            roots.push(offset + index as u32);
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
            rotation: Some(json::scene::UnitQuaternion(joint.rotation.to_array())),
            scale: None,
            skin: None,
            weights: None,
            extras: Default::default(),
        });
    }

    // The skin needs all joints below one root, thus the detached joints are below it too.
    if !detached.is_empty() {
        roots.push(nodes.len() as u32);
        nodes.push(group_node(super::DETACHED_NODE_NAME, &detached));
    }
    nodes.push(group_node("root", &roots));

    nodes.len() - 1
}

fn group_node(name: &str, children: &[u32]) -> json::Node {
    json::Node {
        name: Some(String::from(name)),
        // glTF does not permit an empty `children` array.
        children: (!children.is_empty()).then(|| {
            children
                .iter()
                .map(|&child| json::Index::new(child))
                .collect()
        }),
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
    }
}

fn push_mesh_node(nodes: &mut Vec<json::Node>, mesh: &Mesh, index: u32, skinned: bool) -> usize {
    nodes.push(json::Node {
        name: Some(format!("{}{}", super::MESH_NAME_PREFIX, mesh.name)),
        mesh: Some(json::Index::new(index)),
        skin: skinned.then(|| json::Index::new(0)),
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
            // We include the root of the skeleton as a part of the bone hierarchy,
            // mostly for compatibility purposes with tools such as Blender. The
            // inverse bind matrix of the root is added accordingly.
            .chain(std::iter::once(skeleton_index))
            .map(|index| json::Index::new(index as u32))
            .collect(),
        skeleton: Some(json::Index::new(skeleton_index as u32)),
        name: None,
        extensions: None,
        extras: Default::default(),
    }];

    Ok(())
}

fn insert_meshes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    meshes: &[Mesh],
    skeleton: &[Joint],
) -> Result<()> {
    for mesh in meshes {
        let positions_accessor = insert_positions_bytes(root, buffer, mesh)?;
        let normals_accessor = insert_normals_bytes(root, buffer, mesh)?;
        let uv_accessor = insert_uv_bytes(root, buffer, mesh)?;
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
        if !skeleton.is_empty() {
            let joints_accessor = insert_joints_bytes(root, buffer, mesh, skeleton)?;
            let weights_accessor = insert_weights_bytes(root, buffer, mesh)?;
            attributes.insert(
                Checked::Valid(Semantic::Joints(0)),
                json::Index::new(joints_accessor as u32),
            );
            attributes.insert(
                Checked::Valid(Semantic::Weights(0)),
                json::Index::new(weights_accessor as u32),
            );
        }

        root.meshes.push(json::Mesh {
            name: Some(format!("{}{}", super::MESH_NAME_PREFIX, mesh.name)),
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
    skeleton: &[Joint],
    skeleton_index: usize,
) -> Result<()> {
    // Values closer than this to the identity transform carry no data.
    const EPSILON: f32 = 1e-5;

    for animation in animations {
        let mut gltf_animation = json::Animation {
            name: Some(animation.name.clone()),
            samplers: Vec::new(),
            channels: Vec::new(),
            extensions: None,
            extras: Default::default(),
        };

        let time_accessor = insert_time_bytes(root, buffer, animation)?;
        let mut push_channel = |output, node, path| {
            gltf_animation.samplers.push(json::animation::Sampler {
                input: json::Index::new(time_accessor as u32),
                output: json::Index::new(output as u32),
                // For the sake of simplicity, we use linear interpolation. In reality,
                // Grand Chase uses bezier curves with unknown in-tangent and out-tangent values.
                interpolation: Checked::Valid(gltf::animation::Interpolation::Linear),
                extensions: None,
                extras: Default::default(),
            });
            gltf_animation.channels.push(json::animation::Channel {
                sampler: json::Index::new(gltf_animation.channels.len() as u32),
                target: json::animation::Target {
                    node: json::Index::new(node as u32),
                    path: Checked::Valid(path),
                    extensions: None,
                    extras: Default::default(),
                },
                extensions: None,
                extras: Default::default(),
            });
        };

        let translations: Vec<[f32; 3]> = animation
            .frames
            .iter()
            .map(|frame| frame.translation.into())
            .collect();
        let output = insert_vectors_bytes(root, buffer, &translations)?;
        push_channel(output, skeleton_index, Property::Translation);

        for (index, transforms) in animation.joints().iter().enumerate().take(skeleton.len()) {
            let parts: Vec<_> = transforms
                .iter()
                .map(|&transform| decompose(transform))
                .collect();

            // The index of the loop is also the index of the joint node, because joints
            // are the first nodes in the scene hierarchy.
            let rotations: Vec<[f32; 4]> = parts.iter().map(|(_, r, _)| r.to_array()).collect();
            let output = insert_vectors_bytes(root, buffer, &rotations)?;
            push_channel(output, index, Property::Rotation);

            if parts
                .iter()
                .any(|(_, _, t)| t.abs().max_element() > EPSILON)
            {
                // The channel replaces the translation of the node, thus it adds the node
                // translation to the bone translation.
                let rest = Vec3::from(skeleton[index].translation);
                let translations: Vec<[f32; 3]> =
                    parts.iter().map(|&(_, _, t)| (rest + t).into()).collect();
                let output = insert_vectors_bytes(root, buffer, &translations)?;
                push_channel(output, index, Property::Translation);
            }

            if parts
                .iter()
                .any(|(s, _, _)| !s.abs_diff_eq(Vec3::ONE, EPSILON))
            {
                let scales: Vec<[f32; 3]> = parts.iter().map(|&(s, _, _)| s.into()).collect();
                let output = insert_vectors_bytes(root, buffer, &scales)?;
                push_channel(output, index, Property::Scale);
            }
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

fn insert_joints_bytes(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    mesh: &Mesh,
    skeleton: &[Joint],
) -> Result<usize> {
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
        // A vertex without a joint uses the skeleton root. The root is the last joint
        // of the skin and has an identity inverse bind matrix. Bone animations do not
        // move a vertex of a detached joint, thus it also uses the root.
        let joint = vertex
            .joint
            .filter(|&joint| !skeleton[joint].detached)
            .unwrap_or(skeleton.len());
        buffer.extend_from_slice(&[joint as u8, 0, 0, 0]);
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

    // Each vertex has one joint with full influence (see `insert_joints_bytes`).
    for _ in &mesh.vertices {
        for coordinate in [1., 0., 0., 0.] {
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
        count: (scene.skeleton.len() + 1) as u32,
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
        byte_length: ((scene.skeleton.len() + 1) * mem::size_of::<[f32; 4 * 4]>()) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for (index, _) in scene.skeleton.iter().enumerate() {
        let matrix = scene.joint_world_transform(index).inverse();
        for value in matrix.to_cols_array() {
            buffer.write_f32::<LE>(value)?;
        }
    }
    // Write root inverse bind matrix.
    let matrix = Mat4::IDENTITY;
    for value in matrix.to_cols_array() {
        buffer.write_f32::<LE>(value)?;
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
                .max_by(|a, b| a.partial_cmp(b).unwrap())
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

/// Splits a transform into scale, rotation and translation. A mirror (a negative
/// determinant) gets a negative scale on all three axes. A single negative axis depends
/// on the orientation of the bone axes, and tools that change those axes on import do
/// not keep it. A uniform scale does not depend on them.
fn decompose(transform: &Mat4) -> (Vec3, Quat, Vec3) {
    let sign = if Mat3::from_mat4(*transform).determinant() < 0. {
        -1.
    } else {
        1.
    };
    let axes =
        [transform.x_axis, transform.y_axis, transform.z_axis].map(|axis| sign * axis.truncate());
    let lengths = Vec3::new(axes[0].length(), axes[1].length(), axes[2].length());
    let rotation = Quat::from_mat3(&rotation_axes(axes));
    (sign * lengths, rotation, transform.w_axis.truncate())
}

/// Returns the directions of the axes as the columns of a rotation matrix. Animations
/// can scale an axis to zero, for example to hide vertices. Such an axis has no
/// direction, thus it gets a direction that completes the other axes.
fn rotation_axes(axes: [Vec3; 3]) -> Mat3 {
    let mut units = axes.map(Vec3::try_normalize);
    match units.iter().filter(|unit| unit.is_some()).count() {
        0 => return Mat3::IDENTITY,
        1 => {
            let index = units.iter().position(Option::is_some).unwrap();
            let unit = units[index].unwrap();
            units[(index + 1) % 3] = Some(unit.any_orthonormal_vector());
        }
        _ => {}
    }
    // The cross product of the next two axes gives a right-handed set.
    for index in 0..3 {
        if units[index].is_none() {
            let (next, last) = (units[(index + 1) % 3], units[(index + 2) % 3]);
            units[index] = Some(next.unwrap().cross(last.unwrap()));
        }
    }
    Mat3::from_cols(units[0].unwrap(), units[1].unwrap(), units[2].unwrap())
}

/// Inserts VEC3 or VEC4 float values and returns the index of their accessor.
fn insert_vectors_bytes<const N: usize>(
    root: &mut json::Root,
    buffer: &mut Vec<u8>,
    values: &[[f32; N]],
) -> Result<usize> {
    let type_ = match N {
        3 => json::accessor::Type::Vec3,
        4 => json::accessor::Type::Vec4,
        _ => unreachable!("only VEC3 and VEC4 values are used"),
    };
    let accessor = json::Accessor {
        buffer_view: Some(json::Index::new(root.buffer_views.len() as u32)),
        byte_offset: 0,
        count: values.len() as u32,
        type_: Checked::Valid(type_),
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
        byte_length: mem::size_of_val(values) as u32,
        byte_stride: None,
        name: None,
        target: None,
        extensions: None,
        extras: Default::default(),
    };

    for &value in values.iter().flatten() {
        buffer.write_f32::<LE>(value)?;
    }

    root.accessors.push(accessor);
    root.buffer_views.push(view);

    Ok(root.accessors.len() - 1)
}

/// Adds zeros to the buffer until it is n-byte aligned.
fn align_to(buffer: &mut Vec<u8>, n: usize) {
    let remainder = buffer.len() % n;
    if remainder != 0 {
        buffer.append(&mut vec![0; n - remainder]);
    }
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec2, Vec3A};
    use pretty_assertions::assert_eq;

    use crate::scene::Vertex;

    use super::*;

    #[test]
    fn scene_nodes() {
        let mut root = json::Root::default();
        let skeleton = [
            Joint {
                translation: Vec3A::new(1., 1., 1.),
                rotation: Quat::default(),
                parent: None,
                children: vec![1],
                ..Default::default()
            },
            Joint {
                translation: Vec3A::new(2., 2., 2.),
                rotation: Quat::default(),
                parent: Some(0),
                children: Vec::new(),
                ..Default::default()
            },
            Joint {
                translation: Vec3A::new(0., 0., 0.),
                rotation: Quat::default(),
                parent: None,
                children: Vec::new(),
                ..Default::default()
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

    fn triangle(joint: Option<usize>) -> Mesh {
        let vertex = |x| Vertex {
            position: Vec3A::new(x, 0., 0.),
            normal: Vec3A::Y,
            uv: Vec2::ZERO,
            joint,
        };
        Mesh {
            name: String::from("triangle"),
            vertices: vec![vertex(0.), vertex(1.), vertex(2.)],
            indices: vec![0, 1, 2],
        }
    }

    /// Exports the scene and parses the result, which also validates it.
    fn export(scene: &Scene) -> gltf::Gltf {
        let bytes = GltfExporter::default()
            .export(scene)
            .unwrap()
            .remove(0)
            .bytes;
        gltf::Gltf::from_slice(&bytes).unwrap()
    }

    #[test]
    fn mesh_without_skeleton() {
        let scene = Scene {
            meshes: vec![triangle(None)],
            ..Default::default()
        };

        let gltf = export(&scene);

        assert_eq!(0, gltf.skins().count());
        let mesh_node = gltf.nodes().find(|node| node.mesh().is_some()).unwrap();
        assert!(mesh_node.skin().is_none());
        let primitive = mesh_node.mesh().unwrap().primitives().next().unwrap();
        assert!(primitive.get(&gltf::Semantic::Joints(0)).is_none());
        let json = gltf.document.clone().into_json();
        let root_node = json
            .nodes
            .iter()
            .find(|node| node.name.as_deref() == Some("root"));
        assert!(root_node.unwrap().children.is_none());
    }

    #[test]
    fn vertex_without_joint() {
        let mut mesh = triangle(Some(0));
        mesh.vertices[1].joint = None;
        mesh.vertices[2].joint = Some(1);
        let scene = Scene {
            meshes: vec![mesh],
            skeleton: vec![Joint::default(), Joint::detached()],
            ..Default::default()
        };

        let gltf = export(&scene);

        let blob = gltf.blob.as_deref().unwrap();
        let primitive = gltf.meshes().next().unwrap().primitives().next().unwrap();
        let reader = primitive.reader(|_| Some(blob));
        let joints: Vec<_> = reader.read_joints(0).unwrap().into_u16().collect();
        let weights: Vec<_> = reader.read_weights(0).unwrap().into_f32().collect();
        // Joint 2 is the skeleton root, the last joint of the skin. The vertex of the
        // detached joint 1 also uses it.
        assert_eq!(vec![[0, 0, 0, 0], [2, 0, 0, 0], [2, 0, 0, 0]], joints);
        assert_eq!(vec![[1., 0., 0., 0.]; 3], weights);
    }

    #[test]
    fn decompose_transforms() {
        let rotation = Quat::from_rotation_x(0.3);
        let mirror = Vec3::new(-1., 1., 1.);
        let scales = [
            Vec3::new(2., 1., 0.5),
            mirror,
            // Zero scales, which hide vertices.
            Vec3::new(0., 1., 2.),
            Vec3::new(0., 0., 2.),
            Vec3::ZERO,
        ];
        for scale in scales {
            let transform = Mat4::from_scale_rotation_translation(scale, rotation, Vec3::X);

            let (scale, rotation, translation) = decompose(&transform);

            assert!(rotation.is_finite());
            let actual = Mat4::from_scale_rotation_translation(scale, rotation, translation);
            assert!(actual.abs_diff_eq(transform, 1e-6));
        }

        // A mirror gets a negative scale on all three axes.
        let (scale, _, _) = decompose(&Mat4::from_scale_rotation_translation(
            mirror,
            rotation,
            Vec3::ZERO,
        ));
        assert!(scale.abs_diff_eq(Vec3::NEG_ONE, 1e-6));

        // A projective part does not change the sign of the scale.
        let mut projective = Mat4::from_quat(rotation);
        projective.w_axis.w = -1.;
        let (scale, _, _) = decompose(&projective);
        assert!(scale.abs_diff_eq(Vec3::ONE, 1e-6));
    }
}
