use anyhow::Result;
use glam::Vec3A;

use crate::conversion::{Asset, Exporter, Joint, Mesh, Scene};

use super::internal::{
    AngleBone, MeshVertex, P3m, PositionBone, SkinVertex, INVALID_BONE_INDEX, MAX_NUM_BONES,
};

#[derive(Default)]
pub struct P3mExporter {}

impl Exporter for P3mExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        let mut result = Vec::new();
        for mesh in &scene.meshes {
            let (position_bones, angle_bones) = convert_joints(&scene.skeleton);
            let (skin_vertices, mesh_vertices) =
                convert_vertices(mesh, position_bones.len(), scene);
            let faces = convert_faces(mesh);

            let p3m = P3m {
                position_bones,
                angle_bones,
                skin_vertices,
                mesh_vertices,
                faces,
                ..Default::default()
            };

            let name = if !mesh.name.is_empty() {
                &mesh.name
            } else {
                "mesh"
            };
            let asset = Asset::new(p3m.to_bytes()?, &format!("{}.p3m", name));

            result.push(asset);
        }
        Ok(result)
    }
}

// The conversion is slightly different from the official one. Here, each angle
// bone corresponds to exactly one position bone, whereas official character models
// usually have one position bone for the two 'root' angle bones. Nonetheless,
// the number and the indexes of angle bones stay the same, so it should work fine
// as they are the ones actually used in the game.
fn convert_joints(joints: &[Joint]) -> (Vec<PositionBone>, Vec<AngleBone>) {
    let mut position_bones = Vec::new();
    let mut angle_bones = Vec::new();

    for (index, joint) in joints.iter().take(MAX_NUM_BONES).enumerate() {
        position_bones.push(PositionBone {
            position: joint.translation.into(),
            children: vec![index as u8],
        });

        angle_bones.push(AngleBone {
            children: joint
                .children
                .iter()
                .filter_map(|&index| {
                    if index < u8::MAX as usize {
                        Some(index as u8)
                    } else {
                        None
                    }
                })
                .collect(),
            ..Default::default()
        });
    }

    (position_bones, angle_bones)
}

fn convert_vertices(
    mesh: &Mesh,
    num_position_bones: usize,
    scene: &Scene,
) -> (Vec<SkinVertex>, Vec<MeshVertex>) {
    let mut skin_vertices = Vec::new();
    let mut mesh_vertices = Vec::new();

    for vertex in &mesh.vertices {
        let joint_translation = match vertex.joint {
            Some(index) => scene.joint_world_translation(index),
            None => Vec3A::new(0., 0., 0.),
        };

        skin_vertices.push(SkinVertex {
            position: (vertex.position - joint_translation).into(),
            bone_index: match vertex.joint {
                Some(index) => (index + num_position_bones) as u8,
                None => INVALID_BONE_INDEX,
            },
            normal: vertex.normal.into(),
            uv: vertex.uv.into(),
            ..Default::default()
        });

        mesh_vertices.push(MeshVertex {
            position: vertex.position.into(),
            normal: vertex.normal.into(),
            uv: vertex.uv.into(),
        });
    }

    (skin_vertices, mesh_vertices)
}

fn convert_faces(mesh: &Mesh) -> Vec<[u16; 3]> {
    mesh.indices
        .chunks(3)
        .map(|face| [face[0] as u16, face[1] as u16, face[2] as u16])
        .collect()
}
