// WARNING: GLTF importing does not work properly yet.

use std::{collections::HashMap, path::Path};

use anyhow::Result;
use glam::{Mat4, Quat, Vec2, Vec3, Vec3A};
use gltf::animation::{util::ReadOutputs, Property};

use crate::conversion::{Animation, Asset, Importer, Joint, Keyframe, Mesh, Scene, Vertex};

#[derive(Default)]
pub struct GltfImporter {}

impl Importer for GltfImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        let gltf = gltf::Gltf::from_slice(&asset.bytes)?;
        let buffers = load_buffers(&gltf, asset.path())?;

        let mut joint_map = HashMap::new();
        if let Some(skin) = gltf.skins().next() {
            for node in skin.joints() {
                joint_map.insert(node.index(), joint_map.len());
            }
        }
        let joints = convert_joints(&gltf, &mut joint_map);
        let skeleton_index = get_skeleton_index(&gltf);

        let mut meshes = convert_meshes(&gltf, &buffers, &joint_map);
        let mut animations = convert_animations(&gltf, &buffers, &joint_map, skeleton_index);

        scene.skeleton = joints;
        scene.meshes.append(&mut meshes);
        scene.animations.append(&mut animations);

        *scene = super::transform(scene);

        // println!("");
        // for (i, j) in scene.skeleton.iter().enumerate() {
        //     let global_translation = scene.joint_world_translation(i);
        //     println!("{}\t{:?}\t{:?}", i, j.translation, global_translation);
        // }

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["gltf", "glb"]
    }
}

fn convert_joints(gltf: &gltf::Gltf, joint_map: &mut HashMap<usize, usize>) -> Vec<Joint> {
    const PREFIX: &str = "bone_";

    let nodes = gltf
        .nodes()
        .map(|x| (x.index(), x))
        .collect::<HashMap<_, _>>();

    let mut child_parent_map = HashMap::new();
    for node in gltf.nodes() {
        let node_name = node.name().unwrap_or_default();
        if true {
            // If the name of the node has the format "bone_X", set the index of the joint to X.
            // This is done to maintain the compatibility with the bones of the original format.
            // TODO: move this code away
            if let Some(stripped) = node_name.strip_prefix(PREFIX) {
                if let Ok(joint_index) = stripped.parse() {
                    joint_map.insert(node.index(), joint_index);
                }
            }
            for child in node.children() {
                child_parent_map.insert(child.index(), node.index());
            }
        }
    }

    // Compute absolute and relative positions. This is necessary because the intermediary joint
    // representation only supports translations as joint transoforms.
    let mut absolute_positions = HashMap::new();
    for node in gltf.nodes() {
        let mut transform = Mat4::from_cols_array_2d(&node.transform().matrix());

        let mut current_node = &node;
        while let Some(parent) = child_parent_map
            .get(&current_node.index())
            .and_then(|index| nodes.get(index))
        {
            let parent_transform = Mat4::from_cols_array_2d(&parent.transform().matrix());
            transform = parent_transform.mul_mat4(&transform);
            current_node = parent;
        }

        let position = transform.transform_point3a(Vec3A::ZERO);
        absolute_positions.insert(node.index(), position);
    }

    let max_index = joint_map.values().max().copied().unwrap_or_default();
    let mut joints = vec![Joint::default(); max_index + 1];
    for node in gltf.nodes() {
        if let Some(&joint_index) = joint_map.get(&node.index()) {
            let translation = absolute_positions.get(&node.index()).unwrap();
            let parent_translation = child_parent_map
                .get(&node.index())
                .and_then(|index| absolute_positions.get(index))
                .copied()
                .unwrap_or(Vec3A::ZERO);

            *joints.get_mut(joint_index).unwrap() = Joint {
                translation: *translation - parent_translation,
                parent: child_parent_map
                    .get(&node.index())
                    .and_then(|index| joint_map.get(index))
                    .copied(),
                children: node
                    .children()
                    .filter_map(|child| joint_map.get(&child.index()).copied())
                    .collect(),
            };
        }
    }

    joints
}

/// Returns the index of the skeleton root node. The skeleton of the root
/// node is the first node whose name starts with "root". This node is used to
/// apply translations to the whole skeleton in animations.
fn get_skeleton_index(gltf: &gltf::Gltf) -> Option<usize> {
    gltf.nodes().find_map(|node| {
        if node.name().unwrap_or_default().starts_with("root") {
            Some(node.index())
        } else {
            None
        }
    })
}

/// The animation input time should already be sampled at 55 FPS. All channels should be
/// the same length.
fn convert_animations(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_map: &HashMap<usize, usize>,
    skeleton_index: Option<usize>,
) -> Vec<Animation> {
    let mut result = Vec::new();
    for animation in gltf.animations() {
        let mut root_translations: Vec<Vec3> = Vec::new();
        // Dimensions: [joint, frame, value]
        let mut translations: Vec<Vec<Vec3>> = vec![Vec::new(); joint_map.len()];
        let mut rotations: Vec<Vec<Quat>> = vec![Vec::new(); joint_map.len()];
        let mut scales: Vec<Vec<Vec3>> = vec![Vec::new(); joint_map.len()];

        let mut num_frames = 0;
        for channel in animation.channels() {
            let index = channel.target().node().index();
            if Some(index) == skeleton_index && channel.target().property() == Property::Translation
            {
                // ROOT TRANSLATIONS
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                root_translations = reader
                    .read_outputs()
                    .map(|v| match v {
                        ReadOutputs::Translations(v) => v.map(|x| x.into()).collect(),
                        _ => Vec::new(),
                    })
                    .unwrap_or_default();
                num_frames = num_frames.max(root_translations.len());
            } else if joint_map.contains_key(&index) {
                // BONE TRANSFORMS
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                match channel.target().property() {
                    Property::Translation => {
                        translations[*joint_map.get(&index).unwrap()] = reader
                            .read_outputs()
                            .map(|v| match v {
                                ReadOutputs::Translations(v) => v.map(|x| x.into()).collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();
                        num_frames =
                            num_frames.max(translations[*joint_map.get(&index).unwrap()].len());
                    }
                    Property::Rotation => {
                        rotations[*joint_map.get(&index).unwrap()] = reader
                            .read_outputs()
                            .map(|v| match v {
                                ReadOutputs::Rotations(v) => {
                                    v.into_f32().map(Quat::from_array).collect()
                                }
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();
                        num_frames =
                            num_frames.max(rotations[*joint_map.get(&index).unwrap()].len());
                    }
                    Property::Scale => {
                        scales[*joint_map.get(&index).unwrap()] = reader
                            .read_outputs()
                            .map(|v| match v {
                                ReadOutputs::Scales(v) => v.map(|x| x.into()).collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();
                        num_frames = num_frames.max(scales[*joint_map.get(&index).unwrap()].len());
                    }
                    _ => {}
                }
            }
        }

        let frames = (0..num_frames)
            .map(|i| {
                let root_translation = root_translations.get(i).copied().unwrap_or_default();
                let num_transforms = joint_map.len();
                let transforms: Vec<Mat4> = (0..num_transforms)
                    .map(|j| {
                        let translation = translations
                            .get(j)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or_default();
                        let rotation = rotations
                            .get(j)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or(Quat::IDENTITY);
                        let scale = scales
                            .get(j)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or_else(|| Vec3::new(1., 1., 1.));
                        Mat4::from_scale_rotation_translation(scale, rotation, translation)
                    })
                    .collect();
                Keyframe {
                    translation: root_translation.into(),
                    transforms,
                }
            })
            .collect();

        result.push(Animation {
            name: animation.name().unwrap_or_default().to_string(),
            frames,
        })
    }
    result
}

fn convert_meshes(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_map: &HashMap<usize, usize>,
) -> Vec<Mesh> {
    let mut meshes = Vec::new();
    for mesh in gltf.meshes() {
        let name = mesh.name().unwrap_or_default();
        for primitive in mesh.primitives() {
            let mut mesh = Mesh {
                name: name.into(),
                ..Default::default()
            };

            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions: Vec<Vec3A> = reader
                .read_positions()
                .map(|v| v.map(|x| x.into()).collect())
                .unwrap_or_default();
            let normals: Vec<Vec3A> = reader
                .read_normals()
                .map(|v| v.map(|x| x.into()).collect())
                .unwrap_or_default();
            let tex_coords: Vec<Vec2> = reader
                .read_tex_coords(0)
                .map(|v| v.into_f32().map(|x| x.into()).collect())
                .unwrap_or_default();
            let joints: Vec<_> = reader
                .read_joints(0)
                .map(|v| v.into_u16().collect())
                .unwrap_or_default();
            let weights: Vec<_> = reader
                .read_weights(0)
                .map(|v| v.into_f32().collect())
                .unwrap_or_default();
            let indices: Vec<_> = reader
                .read_indices()
                .map(|v| v.into_u32().map(|x| x as usize).collect())
                .unwrap_or_default();

            mesh.vertices = (0..positions.len())
                .map(|index| {
                    let position = positions[index];
                    let normal = normals.get(index).cloned().unwrap_or_default();
                    let uv = tex_coords.get(index).cloned().unwrap_or_default();
                    let joints = joints.get(index).cloned().unwrap_or_default();
                    let weights = weights.get(index).cloned().unwrap_or_default();

                    // Chooses the joint with maximum influence over the vertex.
                    let (joint, weight) = joints
                        .iter()
                        .zip(weights)
                        .max_by(|(_, w_a), (_, w_b)| {
                            w_a.partial_cmp(w_b).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap();
                    let joint = if weight > 0.0 {
                        joint_map.get(&(*joint as usize)).copied()
                    } else {
                        None
                    };

                    Vertex {
                        position,
                        normal,
                        uv,
                        joint,
                    }
                })
                .collect();
            mesh.indices = indices;

            meshes.push(mesh);
        }
    }
    meshes
}

// Adapted from https://github.com/bevyengine/bevy/blob/c6fec1f0c256597af9746050dd1a4dcd3b80fe24/crates/bevy_gltf/src/loader.rs#L643
fn load_buffers(gltf: &gltf::Gltf, asset_path: &Path) -> Result<Vec<Vec<u8>>> {
    const VALID_MIME_TYPES: &[&str] = &["application/octet-stream", "application/gltf-buffer"];

    let mut buffer_data = Vec::new();
    for buffer in gltf.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Uri(uri) => {
                let buffer_bytes = match DataUri::parse(uri) {
                    Ok(data_uri) if VALID_MIME_TYPES.contains(&data_uri.mime_type) => {
                        data_uri.decode()?
                    }
                    Ok(_) => return Err(anyhow::anyhow!("Buffer format unsupported")),
                    Err(()) => {
                        let buffer_path = asset_path.parent().unwrap().join(uri);
                        std::fs::read(buffer_path)?
                    }
                };
                buffer_data.push(buffer_bytes);
            }
            gltf::buffer::Source::Bin => {
                if let Some(blob) = gltf.blob.as_deref() {
                    buffer_data.push(blob.into());
                } else {
                    return Err(anyhow::anyhow!("The GLB binary chunk is missing"));
                }
            }
        }
    }

    Ok(buffer_data)
}

// Taken from https://github.com/bevyengine/bevy/blob/c6fec1f0c256597af9746050dd1a4dcd3b80fe24/crates/bevy_gltf/src/loader.rs#L742
struct DataUri<'a> {
    mime_type: &'a str,
    base64: bool,
    data: &'a str,
}

impl<'a> DataUri<'a> {
    fn parse(uri: &'a str) -> Result<DataUri<'a>, ()> {
        let uri = uri.strip_prefix("data:").ok_or(())?;
        let (mime_type, data) = split_once(uri, ',').ok_or(())?;

        let (mime_type, base64) = match mime_type.strip_suffix(";base64") {
            Some(mime_type) => (mime_type, true),
            None => (mime_type, false),
        };

        Ok(DataUri {
            mime_type,
            base64,
            data,
        })
    }

    fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
        if self.base64 {
            base64::decode(self.data)
        } else {
            Ok(self.data.as_bytes().to_owned())
        }
    }
}

fn split_once(input: &str, delimiter: char) -> Option<(&str, &str)> {
    let mut iter = input.splitn(2, delimiter);
    Some((iter.next()?, iter.next()?))
}
