use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

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

        // The joint index comes from the "bone_XX" name. The names can skip numbers, thus
        // the highest index sets the number of joints.
        let num_joints = joint_map.values().max().map_or(0, |&max| max + 1);

        let joints = convert_joints(&gltf, &buffers, &joint_map, num_joints);
        let mut meshes = convert_meshes(&gltf, &buffers, &joint_map, &skin_map);
        let mut animations =
            convert_animations(&gltf, &buffers, &joint_map, num_joints, skeleton_root_index);

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

fn convert_joints(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_map: &HashMap<usize, usize>,
    num_joints: usize,
) -> Vec<Joint> {
    // Compute a mapping between child nodes and their parents. This is necessary because
    // the intermediary joint representation keeps track of the each joint's parent.
    let mut child_parent_map = HashMap::new();
    for node in gltf.nodes() {
        for child in node.children() {
            child_parent_map.insert(child.index(), node.index());
        }
    }

    // Derive joint world transforms from inverse bind matrices when available.
    // This is more robust than using node TRS, because tools like Blender may
    // change the node TRS decomposition (e.g. adding bone orientation rotations)
    // while keeping the IBMs consistent. Using IBMs ensures we recover the true
    // bind-pose world transform regardless of how the node TRS is decomposed.
    let ibm_world_transforms = read_ibm_world_transforms(gltf, buffers, joint_map);

    let detached_groups: HashSet<_> = gltf
        .nodes()
        .filter(|node| node.name() == Some(super::DETACHED_NODE_NAME))
        .map(|node| node.index())
        .collect();

    // An index without a "bone_XX" node does not exist in the model, thus its joint is
    // detached.
    let mut joints = vec![Joint::detached(); num_joints];
    for node in gltf.nodes() {
        if let Some(&joint_index) = joint_map.get(&node.index()) {
            let parent_node = child_parent_map.get(&node.index());
            if parent_node.is_some_and(|parent| detached_groups.contains(parent)) {
                continue;
            }
            let parent = parent_node.and_then(|index| joint_map.get(index)).copied();

            let (translation, rotation) =
                if let Some(&world) = ibm_world_transforms.get(&joint_index) {
                    let parent_world = parent
                        .and_then(|pi| ibm_world_transforms.get(&pi))
                        .copied()
                        .unwrap_or(Mat4::IDENTITY);
                    let relative = parent_world.inverse() * world;
                    let (_scale, rotation, translation) = relative.to_scale_rotation_translation();
                    (Vec3A::from(translation), rotation)
                } else {
                    // Fallback to node TRS when IBMs are not available.
                    let (translation, rotation, _scale) = node.transform().decomposed();
                    (Vec3A::from(translation), Quat::from_array(rotation))
                };

            *joints.get_mut(joint_index).unwrap() = Joint {
                translation,
                rotation,
                parent,
                children: node
                    .children()
                    .filter_map(|child| joint_map.get(&child.index()).copied())
                    .collect(),
                detached: false,
            };
        }
    }

    joints
}

/// Reads inverse bind matrices from the first skin and returns the corresponding
/// joint world transforms (i.e. `inverse(IBM)` for each joint).
fn read_ibm_world_transforms(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_map: &HashMap<usize, usize>,
) -> HashMap<usize, Mat4> {
    let mut result = HashMap::new();

    if let Some(skin) = gltf.skins().next() {
        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
        if let Some(ibms) = reader.read_inverse_bind_matrices() {
            for (ibm, joint_node) in ibms.zip(skin.joints()) {
                if let Some(&joint_index) = joint_map.get(&joint_node.index()) {
                    let world_transform = Mat4::from_cols_array_2d(&ibm).inverse();
                    result.insert(joint_index, world_transform);
                }
            }
        }
    }

    result
}

/// The animation input time should already be sampled at 55 FPS. All channels should be
/// the same length.
fn convert_animations(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    joint_map: &HashMap<usize, usize>,
    num_joints: usize,
    skeleton_root_index: Option<usize>,
) -> Vec<Animation> {
    // A translation channel replaces the translation of its node, thus the bone
    // translation is the difference between them.
    let mut rest_translations = vec![Vec3::ZERO; num_joints];
    for node in gltf.nodes() {
        if let Some(&joint) = joint_map.get(&node.index()) {
            rest_translations[joint] = node.transform().decomposed().0.into();
        }
    }

    let mut result = Vec::new();
    for animation in gltf.animations() {
        let mut root_translations: Vec<Vec3> = Vec::new();
        // Dimensions: [joint, frame, value]
        let mut translations: Vec<Vec<Vec3>> = vec![Vec::new(); num_joints];
        let mut rotations: Vec<Vec<Quat>> = vec![Vec::new(); num_joints];
        let mut scales: Vec<Vec<Vec3>> = vec![Vec::new(); num_joints];

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
                let transforms: Vec<Mat4> = (0..num_joints)
                    .map(|j| {
                        let translation = translations[j]
                            .get(i)
                            .map_or(Vec3::ZERO, |&t| t - rest_translations[j]);
                        let rotation = rotations[j].get(i).copied().unwrap_or(Quat::IDENTITY);
                        let scale = scales[j].get(i).copied().unwrap_or(Vec3::ONE);
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::{conversion::Exporter, formats::GltfExporter};

    use super::*;

    #[test]
    fn detached_joints() {
        let scene = Scene {
            skeleton: vec![Joint::default(), Joint::detached()],
            animations: vec![Animation {
                name: String::from("wait"),
                frames: vec![
                    Keyframe {
                        translation: Vec3A::ZERO,
                        transforms: vec![Mat4::from_rotation_x(1.); 2],
                    };
                    2
                ],
            }],
            ..Default::default()
        };
        let bytes = GltfExporter::default()
            .export(&scene)
            .unwrap()
            .remove(0)
            .bytes;

        let gltf = gltf::Gltf::from_slice(&bytes).unwrap();
        let group = gltf.nodes().find(|node| node.name() == Some("detached"));
        let children: Vec<_> = group.unwrap().children().map(|node| node.name()).collect();
        assert_eq!(vec![Some("bone_1")], children);

        let mut actual = Scene::default();
        GltfImporter::default()
            .import(&Asset::new(bytes, "model.glb"), &mut actual)
            .unwrap();
        assert_eq!(scene.skeleton, actual.skeleton);
        // The animation data of a detached joint stays, because other models can use it.
        let transform = actual.animations[0].frames[1].transforms[1];
        assert!(transform.abs_diff_eq(Mat4::from_rotation_x(1.), 1e-6));
    }

    #[test]
    fn bone_scale_and_translation() {
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(2., 1., 0.5),
            Quat::from_rotation_y(0.5),
            Vec3::X,
        );
        // The joint translation is not zero, thus the importer must subtract it.
        let scene = Scene {
            skeleton: vec![Joint {
                translation: Vec3A::new(0., 3., 0.),
                ..Default::default()
            }],
            animations: vec![Animation {
                name: String::from("wait"),
                frames: vec![Keyframe {
                    translation: Vec3A::ZERO,
                    transforms: vec![transform],
                }],
            }],
            ..Default::default()
        };
        let bytes = GltfExporter::default()
            .export(&scene)
            .unwrap()
            .remove(0)
            .bytes;

        let mut actual = Scene::default();
        GltfImporter::default()
            .import(&Asset::new(bytes, "model.glb"), &mut actual)
            .unwrap();

        assert!(actual.animations[0].frames[0].transforms[0].abs_diff_eq(transform, 1e-5));
    }

    #[test]
    fn bone_names_with_gap() {
        let chain = |parent, children| Joint {
            parent,
            children,
            ..Default::default()
        };
        let scene = Scene {
            skeleton: vec![
                chain(None, vec![1]),
                chain(Some(0), vec![2]),
                chain(Some(1), vec![]),
            ],
            animations: vec![Animation {
                name: String::from("wait"),
                frames: vec![
                    Keyframe {
                        translation: Vec3A::ZERO,
                        transforms: vec![Mat4::IDENTITY; 3],
                    };
                    2
                ],
            }],
            ..Default::default()
        };
        let bytes = GltfExporter::default()
            .export(&scene)
            .unwrap()
            .remove(0)
            .bytes;

        // Rename "bone_1". The names that stay are "bone_0" and "bone_2".
        let glb = gltf::Glb::from_slice(&bytes).unwrap();
        let mut root: gltf::json::Root = gltf::json::deserialize::from_slice(&glb.json).unwrap();
        root.nodes[1].name = Some(String::from("Bone.001"));
        let json = gltf::json::serialize::to_string(&root).unwrap();
        let bytes = gltf::Glb {
            header: glb.header,
            json: json.into_bytes().into(),
            bin: glb.bin.clone(),
        }
        .to_vec()
        .unwrap();

        let mut actual = Scene::default();
        GltfImporter::default()
            .import(&Asset::new(bytes, "model.glb"), &mut actual)
            .unwrap();

        assert_eq!(3, actual.skeleton.len());
        for frame in &actual.animations[0].frames {
            assert_eq!(3, frame.transforms.len());
        }
    }
}
