use anyhow::{Context, Result};

use crate::conversion::{Asset, Importer, Joint, Scene};

use super::internal::{AngleBone, P3m, PositionBone};

pub struct P3mImporter {}

impl Importer for P3mImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        let p3m = P3m::from_bytes(&asset.bytes)
            .context("Failed to deserialize the bytes of the .p3m asset")?;

        scene.skeleton = convert_joints(&p3m.position_bones, &p3m.angle_bones);

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["p3m"]
    }
}

fn convert_joints(position_bones: &[PositionBone], angle_bones: &[AngleBone]) -> Vec<Joint> {
    let mut joints: Vec<_> = angle_bones.iter().map(|_| Joint::new()).collect();

    // Apply translation to the joints.
    for p_bone in position_bones {
        for &child in &p_bone.children {
            joints[child as usize].translation = p_bone.position.into();
        }
    }

    // Update joint children by squashing position and angle bones.
    for (joint, a_bone) in joints.iter_mut().zip(angle_bones) {
        let children = a_bone
            .children
            .iter()
            .flat_map(|&x| &position_bones[x as usize].children);
        for &child in children {
            joint.children.push(child as usize);
        }
    }

    // Set the parents of the joints.
    let mut joint_parents: Vec<_> = joints.iter().map(|_| None).collect();
    for (index, joint) in joints.iter().enumerate() {
        for &child in &joint.children {
            joint_parents[child] = Some(index);
        }
    }
    for (child, parent) in joint_parents.into_iter().enumerate() {
        joints[child].parent = parent;
    }

    joints
}

#[cfg(test)]
mod tests {
    use glam::Vec3A;

    use super::*;

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
                parent: None,
                children: vec![2],
            },
            Joint {
                translation: Vec3A::new(1., 1., 1.),
                parent: None,
                children: Vec::new(),
            },
            Joint {
                translation: Vec3A::new(2., 2., 2.),
                parent: Some(0),
                children: vec![3],
            },
            Joint {
                translation: Vec3A::new(3., 3., 3.),
                parent: Some(2),
                children: Vec::new(),
            },
        ];

        assert_eq!(expected, actual);
    }
}
