use std::path::Path;

use anyhow::Result;
use glam::{Vec2, Vec3A};

use crate::conversion::{Asset, Importer, Mesh, Scene, Vertex};

#[derive(Default)]
pub struct GltfImporter {}

impl Importer for GltfImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        let gltf = gltf::Gltf::from_slice(&asset.bytes)?;
        let buffers = load_buffers(&gltf, asset.path())?;

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
                            Some(*joint as usize)
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

                meshes.push(mesh);
            }
        }

        todo!()
    }

    fn extensions(&self) -> &[&str] {
        &["gltf", "glb"]
    }
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
