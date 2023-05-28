use std::{collections::HashMap, path::Path};

use anyhow::Result;
use glam::{Mat4, Quat, Vec2, Vec3, Vec3A};
use gltf::animation::{util::ReadOutputs, Property};

use crate::{
    asset::Asset,
    conversion::Importer,
    scene::{Animation, Joint, Keyframe, Mesh, Scene, Vertex},
};

#[derive(Default)]
pub struct GltfImporter {}

impl Importer for GltfImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        let gltf = gltf::Gltf::from_slice(&asset.bytes)?;
        let buffers = load_buffers(&gltf, asset.path())?;

        let skin_map = make_skin_map(&gltf);
        let joint_map = make_joint_map(&gltf);
        let skeleton_root_index = get_skeleton_root_index(&gltf);

        let joints = convert_joints(&gltf, &joint_map);
        let mut meshes = convert_meshes(&gltf, &buffers, &joint_map, &skin_map);
        let mut animations = convert_animations(&gltf, &buffers, &joint_map, skeleton_root_index);

        scene.skeleton = joints;
        scene.meshes.append(&mut meshes);
        scene.animations.append(&mut animations);

        *scene = super::transform(scene);

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["gltf", "glb"]
    }
}

/// Returns a mapping between joint indices (referenced by skinned vertex data)
/// and node indices.
fn make_skin_map(gltf: &gltf::Gltf) -> HashMap<usize, usize> {
    gltf.skins()
        .next()
        .map(|skin| {
            skin.joints()
                .enumerate()
                .map(|(index, node)| (index, node.index()))
                .collect()
        })
        .unwrap_or_default()
}

/// Returns a mapping between GLTF node indices and joint indices from the
/// internal scene representation. Only joints named "bone_XX" are considered.
fn make_joint_map(gltf: &gltf::Gltf) -> HashMap<usize, usize> {
    gltf.nodes()
        .filter_map(|node| {
            let node_name = node.name().unwrap_or_default();
            if let Some(stripped) = node_name.strip_prefix("bone_") {
                if let Ok(joint_index) = stripped.parse() {
                    return Some((node.index(), joint_index));
                }
            }
            None
        })
        .collect()
}

/// Returns the index of the skeleton root node. The skeleton of the root
/// node is the first node whose name is "root". This node is used to
/// apply translations to the whole skeleton in animations.
fn get_skeleton_root_index(gltf: &gltf::Gltf) -> Option<usize> {
    gltf.nodes().find_map(|node| {
        if node.name().unwrap_or_default() == "root" {
            Some(node.index())
        } else {
            None
        }
    })
}

fn convert_joints(gltf: &gltf::Gltf, joint_map: &HashMap<usize, usize>) -> Vec<Joint> {
    let nodes = gltf
        .nodes()
        .map(|x| (x.index(), x))
        .collect::<HashMap<_, _>>();

    // Compute a mapping between child nodes and their parents. This is necessary because
    // the intermediary joint representation keeps track of the each joint's parent.
    let mut child_parent_map = HashMap::new();
    for node in gltf.nodes() {
        for child in node.children() {
            child_parent_map.insert(child.index(), node.index());
        }
    }

    // Compute absolute joint positions.
    let mut joint_absolute_positions = HashMap::new();
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
        joint_absolute_positions.insert(node.index(), position);
    }

    // Calculate joint positions relative to their parents in order to do away with
    // the rotation transforms of each joint. This is done because the intermediary
    // representation doesn't allow joints to have default rotations.
    let joint_relative_positions = joint_absolute_positions
        .iter()
        .map(|(index, &position)| {
            let parent_position = child_parent_map
                .get(index)
                .and_then(|parent_index| joint_absolute_positions.get(parent_index))
                .copied()
                .unwrap_or(Vec3A::ZERO);
            (index, position - parent_position)
        })
        .collect::<HashMap<_, _>>();

    let max_index = joint_map.values().max().copied().unwrap_or_default();
    let mut joints = vec![Joint::default(); max_index + 1];
    for node in gltf.nodes() {
        if let Some(&joint_index) = joint_map.get(&node.index()) {
            *joints.get_mut(joint_index).unwrap() = Joint {
                translation: joint_relative_positions
                    .get(&node.index())
                    .copied()
                    .unwrap(),
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

/// The animation input time should already be sampled at 55 FPS. All channels should be
/// the same length. Joint translations and scales are ignored.
fn convert_animations(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_map: &HashMap<usize, usize>,
    skeleton_root_index: Option<usize>,
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
            if Some(index) == skeleton_root_index
                && channel.target().property() == Property::Translation
            {
                // Root translations
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
                // Joint transforms
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
                        let _ = translations
                            .get(j)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or_default();
                        let rotation = rotations
                            .get(j)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or(Quat::IDENTITY);
                        let _ = scales
                            .get(j)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or_else(|| Vec3::new(1., 1., 1.));

                        // Currently, translation and scale are ignored. Only rotation
                        // is taken into account. In order to use the entire transform,
                        // it would be necessary to apply the inverse joint transforms
                        // from the default pose.
                        Mat4::from_rotation_translation(rotation, Vec3::ZERO)
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
    skin_map: &HashMap<usize, usize>,
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
                        let joint = skin_map.get(&(*joint as usize)).unwrap();
                        joint_map.get(joint).copied()
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
