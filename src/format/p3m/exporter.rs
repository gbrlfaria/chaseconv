use anyhow::Result;

use crate::conversion::{Asset, Exporter, Joint, Mesh, Scene};

use super::internal::{AngleBone, MeshVertex, P3m, PositionBone, SkinVertex, MAX_NUM_BONES};

#[derive(Default)]
pub struct P3mExporter {}

impl Exporter for P3mExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        let mut result = Vec::new();
        for mesh in &scene.meshes {
            let (position_bones, angle_bones) = convert_joints(&scene.skeleton);
            let (skin_vertices, mesh_vertices) = convert_vertices(mesh);
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

// TODO: check character biped hierarchy to make it compatible...
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

fn convert_vertices(mesh: &Mesh) -> (Vec<SkinVertex>, Vec<MeshVertex>) {
    let mut skin_vertices = Vec::new();
    let mut mesh_vertices = Vec::new();

    // skin_vertices
    // .iter()
    // .map(|vertex| {
    //     let joint = vertex.bone_index as usize - num_position_bones;
    //     Vertex {
    //         position: Vec3A::from(vertex.position) + scene.joint_world_translation(joint),
    //         normal: Vec3A::from(vertex.normal),
    //         uv: vertex.uv.into(),
    //         joint: if joint != 0xff { Some(joint) } else { None },
    //     }
    // })
    // .collect()

    (skin_vertices, mesh_vertices)
}

fn convert_faces(mesh: &Mesh) -> Vec<[u16; 3]> {
    mesh.indices
        .chunks(3)
        .map(|face| [face[0] as u16, face[1] as u16, face[2] as u16])
        .collect()
}
