use anyhow::Result;
use glam::Mat4;

use crate::{
    asset::Asset,
    conversion::Exporter,
    scene::{Joint, Mesh, Scene},
};

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

/// Converts the joints into position bones and angle bones. Angle bone `i` is joint `i`.
/// Position bone 0 holds the roots, and each other joint has its own position bone.
fn convert_joints(joints: &[Joint]) -> (Vec<PositionBone>, Vec<AngleBone>) {
    let joints = &joints[..joints.len().min(MAX_NUM_BONES)];

    let roots: Vec<_> = (0..joints.len())
        .filter(|&index| joints[index].parent.is_none())
        .collect();
    let mut position_bones = vec![PositionBone {
        // All roots share position bone 0, thus they get the translation of the first root.
        position: roots
            .first()
            .map_or([0.; 3], |&root| joints[root].translation.into()),
        children: roots.iter().map(|&root| root as u8).collect(),
    }];
    let mut position_bone_of = vec![None; joints.len()];
    for (index, joint) in joints.iter().enumerate() {
        if joint.parent.is_some() {
            position_bone_of[index] = Some(position_bones.len() as u8);
            position_bones.push(PositionBone {
                position: joint.translation.into(),
                children: vec![index as u8],
            });
        }
    }

    let angle_bones = joints
        .iter()
        .map(|joint| AngleBone {
            children: joint
                .children
                .iter()
                .filter_map(|&child| position_bone_of.get(child).copied().flatten())
                .collect(),
            ..Default::default()
        })
        .collect();

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
        let joint_transform = match vertex.joint {
            Some(index) => scene.joint_world_transform(index),
            None => Mat4::IDENTITY,
        };

        skin_vertices.push(SkinVertex {
            position: joint_transform
                .inverse()
                .transform_point3a(vertex.position)
                .into(),
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn joints_with_three_roots() {
        // Joints 0, 2 and 4 are roots. Each root has one child.
        let joints: Vec<_> = [
            (None, vec![1]),
            (Some(0), vec![]),
            (None, vec![3]),
            (Some(2), vec![]),
            (None, vec![5]),
            (Some(4), vec![]),
        ]
        .into_iter()
        .map(|(parent, children)| Joint {
            parent,
            children,
            ..Default::default()
        })
        .collect();

        let (position_bones, angle_bones) = convert_joints(&joints);

        // Position bone 0 holds all roots. Each child joint has its own position bone.
        let position_children: Vec<_> = position_bones.iter().map(|b| &b.children).collect();
        assert_eq!(
            vec![&vec![0, 2, 4], &vec![1], &vec![3], &vec![5]],
            position_children
        );
        let angle_children: Vec<_> = angle_bones.iter().map(|b| &b.children).collect();
        assert_eq!(
            vec![&vec![1], &vec![], &vec![2], &vec![], &vec![3], &vec![]],
            angle_children
        );
    }
}
