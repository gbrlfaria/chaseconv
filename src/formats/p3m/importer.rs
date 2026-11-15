use std::collections::VecDeque;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3A};

use crate::{
    asset::Asset,
    conversion::Importer,
    scene::{Joint, Mesh, Scene, Vertex},
};

use super::internal::{AngleBone, P3m, PositionBone, SkinVertex};

#[derive(Default)]
pub struct P3mImporter {}

impl Importer for P3mImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        let p3m = P3m::from_bytes(&asset.bytes)
            .context("Failed to deserialize the bytes of the P3M asset")?;

        scene.skeleton = convert_joints(&p3m.position_bones, &p3m.angle_bones);
        scene
            .meshes
            .push(convert_mesh(&p3m, asset.name().to_string(), scene));

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["p3m"]
    }
}

/// Walks the skeleton from position bone 0. A position bone gives its translation to
/// its child angle bones, and an angle bone continues to its child position bones. The
/// angle bones that the walk does not reach are not part of the skeleton, thus they
/// become detached joints.
fn convert_joints(position_bones: &[PositionBone], angle_bones: &[AngleBone]) -> Vec<Joint> {
    let mut joints = vec![Joint::detached(); angle_bones.len()];
    // Each item is a position bone and the joint of its parent angle bone.
    let mut queue = VecDeque::from([(0, None)]);
    while let Some((position_bone, parent)) = queue.pop_front() {
        let Some(position_bone) = position_bones.get(position_bone) else {
            continue;
        };
        for &child in &position_bone.children {
            let child = child as usize;
            // Only the first visit counts. This also stops cycles.
            if !joints.get(child).is_some_and(|joint| joint.detached) {
                continue;
            }
            joints[child] = Joint {
                translation: position_bone.position.into(),
                parent,
                ..Default::default()
            };
            if let Some(parent) = parent {
                joints[parent].children.push(child);
            }
            let next = angle_bones[child].children.iter();
            queue.extend(next.map(|&position_bone| (position_bone as usize, Some(child))));
        }
    }

    joints
}

fn convert_mesh(p3m: &P3m, name: String, scene: &Scene) -> Mesh {
    Mesh {
        name,
        vertices: convert_vertices(&p3m.skin_vertices, p3m.position_bones.len(), scene),
        indices: p3m
            .faces
            .iter()
            .flat_map(|face| face.iter().map(|&index| index as usize))
            .collect(),
    }
}

fn convert_vertices(
    skin_vertices: &[SkinVertex],
    num_position_bones: usize,
    scene: &Scene,
) -> Vec<Vertex> {
    skin_vertices
        .iter()
        .map(|vertex| {
            // The slot uses 8-bit arithmetic, thus the subtraction wraps. A slot outside
            // the skeleton does not attach the vertex to a joint.
            let slot = vertex.bone_index.wrapping_sub(num_position_bones as u8) as usize;
            let joint = (slot < scene.skeleton.len()).then_some(slot);
            let transform = joint
                .map(|joint| scene.joint_world_transform(joint))
                .unwrap_or(Mat4::IDENTITY);
            Vertex {
                position: transform.transform_point3a(Vec3A::from(vertex.position)),
                normal: Vec3A::from(vertex.normal),
                uv: vertex.uv.into(),
                joint,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use glam::{Quat, Vec2, Vec3A};

    use super::*;

    #[test]
    fn vertices_outside_skeleton() {
        // With two position bones, bone index 2 is slot 0, index 3 is slot 1 (no joint),
        // and index 1 wraps to slot 255 (no joint).
        let skin_vertices = [2, 3, 1].map(|bone_index| SkinVertex {
            position: [1., 0., 0.],
            bone_index,
            ..Default::default()
        });
        let scene = Scene {
            skeleton: vec![Joint {
                translation: Vec3A::new(0., 1., 0.),
                ..Default::default()
            }],
            ..Default::default()
        };

        let actual = convert_vertices(&skin_vertices, 2, &scene);

        let joints: Vec<_> = actual.iter().map(|vertex| vertex.joint).collect();
        assert_eq!(vec![Some(0), None, None], joints);
        assert_eq!(Vec3A::new(1., 1., 0.), actual[0].position);
        assert_eq!(Vec3A::new(1., 0., 0.), actual[1].position);
    }

    #[test]
    fn mesh() {
        let name = String::from("model");
        let p3m = P3m {
            version_header: String::new(),
            position_bones: vec![PositionBone::new(); 1],
            angle_bones: Vec::new(),
            texture_name: String::new(),
            faces: vec![[0, 1, 2]],
            skin_vertices: vec![
                SkinVertex {
                    position: [1., 0., 0.],
                    weight: 1.,
                    bone_index: 1,
                    normal: [1., 0., 0.],
                    uv: [0., 0.],
                },
                SkinVertex {
                    position: [0., 1., 0.],
                    weight: 1.,
                    bone_index: 1,
                    normal: [0., 1., 0.],
                    uv: [0.5, 0.5],
                },
                SkinVertex {
                    position: [0., 0., 1.],
                    weight: 1.,
                    bone_index: 1,
                    normal: [0., 0., 1.],
                    uv: [1., 1.],
                },
            ],
            mesh_vertices: Vec::new(),
        };
        let scene = Scene {
            meshes: Vec::new(),
            skeleton: vec![Joint {
                translation: Vec3A::new(1., 1., 1.),
                rotation: Quat::default(),
                parent: None,
                children: Vec::new(),
                ..Default::default()
            }],
            animations: Vec::new(),
        };

        let actual = convert_mesh(&p3m, name, &scene);
        let expected = Mesh {
            name: String::from("model"),
            vertices: vec![
                Vertex {
                    position: Vec3A::new(2., 1., 1.),
                    normal: Vec3A::new(1., 0., 0.),
                    uv: Vec2::new(0., 0.),
                    joint: Some(0),
                },
                Vertex {
                    position: Vec3A::new(1., 2., 1.),
                    normal: Vec3A::new(0., 1., 0.),
                    uv: Vec2::new(0.5, 0.5),
                    joint: Some(0),
                },
                Vertex {
                    position: Vec3A::new(1., 1., 2.),
                    normal: Vec3A::new(0., 0., 1.),
                    uv: Vec2::new(1., 1.),
                    joint: Some(0),
                },
            ],
            indices: vec![0, 1, 2],
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn unreachable_angle_bones() {
        // No angle bone refers to position bone 1, thus the walk from position bone 0
        // does not reach angle bone 1 and its child, angle bone 2.
        let position_bone = |children| PositionBone {
            position: [1.; 3],
            children,
        };
        let angle_bone = |children| AngleBone {
            children,
            ..Default::default()
        };
        let position_bones = [
            position_bone(vec![0]),
            position_bone(vec![1]),
            position_bone(vec![2]),
        ];
        let angle_bones = [angle_bone(vec![]), angle_bone(vec![2]), angle_bone(vec![])];

        let actual = convert_joints(&position_bones, &angle_bones);

        let detached: Vec<_> = actual.iter().map(|joint| joint.detached).collect();
        assert_eq!(vec![false, true, true], detached);
        assert_eq!(Joint::detached(), actual[1]);
    }

    #[test]
    fn joints() {
        let position_bones = vec![
            PositionBone {
                position: [1.; 3],
                children: vec![0, 1],
            },
            PositionBone {
                position: [2.; 3],
                children: vec![2],
            },
            PositionBone {
                position: [3.; 3],
                children: vec![3],
            },
        ];
        let angle_bones = vec![
            AngleBone {
                position: [0.; 3],
                scale: 0.,
                children: vec![1],
            },
            AngleBone {
                position: [0.; 3],
                scale: 0.,
                children: Vec::new(),
            },
            AngleBone {
                position: [0.; 3],
                scale: 0.,
                children: vec![2],
            },
            AngleBone {
                position: [0.; 3],
                scale: 0.,
                children: Vec::new(),
            },
        ];

        let actual = super::convert_joints(&position_bones, &angle_bones);
        let expected = vec![
            Joint {
                translation: Vec3A::new(1., 1., 1.),
                rotation: Quat::default(),
                parent: None,
                children: vec![2],
                ..Default::default()
            },
            Joint {
                translation: Vec3A::new(1., 1., 1.),
                rotation: Quat::default(),
                parent: None,
                children: Vec::new(),
                ..Default::default()
            },
            Joint {
                translation: Vec3A::new(2., 2., 2.),
                rotation: Quat::default(),
                parent: Some(0),
                children: vec![3],
                ..Default::default()
            },
            Joint {
                translation: Vec3A::new(3., 3., 3.),
                rotation: Quat::default(),
                parent: Some(2),
                children: Vec::new(),
                ..Default::default()
            },
        ];

        assert_eq!(expected, actual);
    }
}
