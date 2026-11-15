use anyhow::Result;
use glam::Mat4;

use crate::{
    asset::Asset,
    conversion::Exporter,
    scene::{Joint, Mesh, Scene},
};

use super::internal::{AngleBone, MeshVertex, P3m, PositionBone, SkinVertex, MAX_NUM_BONES};

#[derive(Default)]
pub struct P3mExporter {}

impl Exporter for P3mExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        let mut result = Vec::new();
        for mesh in &scene.meshes {
            let unbound = mesh.vertices.iter().any(|vertex| vertex.joint.is_none());
            let (position_bones, angle_bones, detached) = convert_joints(&scene.skeleton, unbound);
            let (skin_vertices, mesh_vertices) =
                convert_vertices(mesh, position_bones.len(), detached, scene);
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
/// Position bone 0 holds the roots, and each other attached joint has its own position
/// bone. A detached joint has no position bone, thus the walk from position bone 0
/// does not reach it.
///
/// Also returns a detached angle bone for vertices without a joint. If `unbound` is true
/// and no joint is detached, the result gets one more angle bone for this purpose.
fn convert_joints(
    joints: &[Joint],
    unbound: bool,
) -> (Vec<PositionBone>, Vec<AngleBone>, Option<usize>) {
    let joints = &joints[..joints.len().min(MAX_NUM_BONES)];

    let roots: Vec<_> = (0..joints.len())
        .filter(|&index| joints[index].parent.is_none() && !joints[index].detached)
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
        if joint.parent.is_some() && !joint.detached {
            position_bone_of[index] = Some(position_bones.len() as u8);
            position_bones.push(PositionBone {
                position: joint.translation.into(),
                children: vec![index as u8],
            });
        }
    }

    let mut angle_bones: Vec<_> = joints
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
    let mut detached = joints.iter().position(|joint| joint.detached);
    if unbound && detached.is_none() {
        detached = Some(angle_bones.len());
        angle_bones.push(AngleBone::default());
    }

    (position_bones, angle_bones, detached)
}

fn convert_vertices(
    mesh: &Mesh,
    num_position_bones: usize,
    detached: Option<usize>,
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
            // A detached angle bone does not move, thus the vertex stays in place.
            bone_index: (vertex
                .joint
                .or(detached)
                .expect("convert_joints gives a detached angle bone to vertices without a joint")
                + num_position_bones) as u8,
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
    fn joints() {
        // Joints 0, 2 and 4 are roots. Each root has one child. Joint 6 is detached.
        let mut joints: Vec<_> = [
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
        joints.push(Joint::detached());

        let (position_bones, angle_bones, detached) = convert_joints(&joints, true);

        // Position bone 0 holds all roots. Each child joint has its own position bone, and
        // no position bone refers to the detached joint.
        let position_children: Vec<_> = position_bones.iter().map(|b| &b.children).collect();
        assert_eq!(
            vec![&vec![0, 2, 4], &vec![1], &vec![3], &vec![5]],
            position_children
        );
        let angle_children: Vec<_> = angle_bones.iter().map(|b| &b.children).collect();
        assert_eq!(
            vec![
                &vec![1],
                &vec![],
                &vec![2],
                &vec![],
                &vec![3],
                &vec![],
                &vec![]
            ],
            angle_children
        );
        assert_eq!(Some(6), detached);

        // Without a detached joint, vertices without a joint get a new angle bone.
        let (_, angle_bones, detached) = convert_joints(&joints[..6], true);
        assert_eq!((7, Some(6)), (angle_bones.len(), detached));
    }
}
